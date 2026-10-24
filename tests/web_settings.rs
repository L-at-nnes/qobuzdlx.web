mod common;

use common::Reply;
use common::app::TestApp;
use serde_json::{Value, json};

async fn current(app: &TestApp) -> Value {
    app.browser().get("/api/settings").await.json()
}

#[tokio::test]
async fn the_settings_are_read_as_they_are_stored() {
    let app = TestApp::start().await;

    let settings = current(&app).await;

    assert_eq!(settings["download"]["quality"], "flac16");
    assert_eq!(settings["naming"]["album"], "%AlbumTitle%");
    assert_eq!(settings["tags"]["genre"], true);
    assert_eq!(settings["advanced"]["duplicates"], "skip");
    assert_eq!(settings["artwork"]["embedded_size"], 600);
}

#[tokio::test]
async fn changed_settings_are_saved_and_come_back() {
    let app = TestApp::start().await;
    let mut settings = current(&app).await;
    settings["naming"]["album"] = json!("%Year% - %AlbumTitle%");
    settings["advanced"]["duplicates"] = json!("rename");
    settings["artwork"]["saved_size"] = json!("original");
    settings["language"] = json!("fr");

    let answer = app.browser().put("/api/settings", settings.clone()).await;

    assert_eq!(answer.status, 200);
    assert_eq!(answer.json(), settings);
    let reread = current(&app).await;
    assert_eq!(reread["naming"]["album"], "%Year% - %AlbumTitle%");
    assert_eq!(reread["artwork"]["saved_size"], "original");
    assert_eq!(reread["language"], "fr");
    let on_disk = std::fs::read_to_string(app.config.path().join("settings.json")).unwrap();
    assert!(on_disk.contains("%Year% - %AlbumTitle%"));
}

#[tokio::test]
async fn the_settings_never_hold_a_secret() {
    let app = TestApp::start_with(Some("correct horse battery")).await;
    let browser = app.browser();
    browser
        .post(
            "/api/access/unlock",
            json!({ "password": "correct horse battery" }),
        )
        .await;

    let text = browser.get("/api/settings").await.text;
    let file = std::fs::read_to_string(app.config.path().join("settings.json")).unwrap_or_default();

    for forbidden in ["correct horse battery", "argon2", "the-token", "the-secret"] {
        assert!(!text.contains(forbidden), "{forbidden} in the answer");
        assert!(!file.contains(forbidden), "{forbidden} in the file");
    }
}

#[tokio::test]
async fn a_cover_size_out_of_range_is_refused() {
    let app = TestApp::start().await;
    let mut settings = current(&app).await;
    settings["artwork"]["embedded_size"] = json!(5000);

    let answer = app.browser().put("/api/settings", settings).await;

    assert_eq!(answer.status, 400);
    assert_eq!(answer.json()["error"], "invalid_artwork_size");
}

#[tokio::test]
async fn the_waits_of_the_rate_limit_are_checked_and_kept() {
    let app = TestApp::start().await;
    let mut settings = current(&app).await;

    settings["rate_limit"]["between_albums"] = json!({ "min_ms": 9000, "max_ms": 2000 });
    let reversed = app.browser().put("/api/settings", settings.clone()).await;
    settings["rate_limit"]["between_albums"] = json!({ "min_ms": 0, "max_ms": 7_200_000 });
    let too_long = app.browser().put("/api/settings", settings.clone()).await;
    settings["rate_limit"]["between_albums"] = json!({ "min_ms": 4000, "max_ms": 9000 });
    settings["rate_limit"]["break_after_albums"] = json!(100);
    settings["rate_limit"]["break_after_megabytes"] = json!(500);
    let fine = app.browser().put("/api/settings", settings).await;

    assert_eq!(reversed.status, 400);
    assert_eq!(reversed.json()["error"], "invalid_rate_limit");
    assert_eq!(too_long.status, 400);
    assert_eq!(fine.status, 200);
    let reread = current(&app).await;
    assert_eq!(reread["rate_limit"]["between_albums"]["max_ms"], 9000);
    assert_eq!(reread["rate_limit"]["break_after_albums"], 100);
    assert_eq!(reread["rate_limit"]["break_after_megabytes"], 500);
}

#[tokio::test]
async fn the_number_of_tracks_at_once_stays_within_its_range() {
    let app = TestApp::start().await;
    let mut settings = current(&app).await;

    settings["advanced"]["parallel_tracks"] = json!(0);
    let none = app.browser().put("/api/settings", settings.clone()).await;
    settings["advanced"]["parallel_tracks"] = json!(100);
    let many = app.browser().put("/api/settings", settings.clone()).await;
    settings["advanced"]["parallel_tracks"] = json!(4);
    let fine = app.browser().put("/api/settings", settings).await;

    assert_eq!(none.status, 400);
    assert_eq!(none.json()["error"], "invalid_parallel_tracks");
    assert_eq!(many.status, 400);
    assert_eq!(fine.status, 200);
    assert_eq!(current(&app).await["advanced"]["parallel_tracks"], 4);
}

#[tokio::test]
async fn the_download_folder_must_be_given_and_absolute() {
    let app = TestApp::start().await;
    let mut settings = current(&app).await;

    settings["download"]["directory"] = json!("");
    let missing = app.browser().put("/api/settings", settings.clone()).await;
    settings["download"]["directory"] = json!("relative/music");
    let relative = app.browser().put("/api/settings", settings).await;

    assert_eq!(missing.json()["error"], "missing_download_directory");
    assert_eq!(relative.json()["error"], "download_directory_not_absolute");
}

#[tokio::test]
async fn a_template_that_cannot_work_is_refused() {
    let app = TestApp::start().await;
    for (field, template) in [
        ("track", ""),
        ("album", "/etc/%AlbumTitle%"),
        ("artist", "../%ArtistName%"),
    ] {
        let mut settings = current(&app).await;
        settings["naming"][field] = json!(template);

        let answer = app.browser().put("/api/settings", settings).await;

        assert_eq!(answer.status, 422, "{field}: {template:?}");
        assert_eq!(answer.json()["error"], "invalid_templates");
    }
    assert_eq!(current(&app).await["naming"]["album"], "%AlbumTitle%");
}

#[tokio::test]
async fn a_template_with_an_unknown_variable_is_saved_with_a_warning_shown_elsewhere() {
    let app = TestApp::start().await;
    let mut settings = current(&app).await;
    settings["naming"]["album"] = json!("%AlbumTitel%");

    let saved = app.browser().put("/api/settings", settings.clone()).await;
    let tried = app.browser().post("/api/settings/preview", settings).await;

    assert_eq!(saved.status, 200);
    assert_eq!(tried.json()["issues"][0]["kind"], "unknown_variable");
    assert_eq!(tried.json()["issues"][0]["field"], "album");
}

#[tokio::test]
async fn the_templates_are_tried_on_an_invented_release_without_being_saved() {
    let app = TestApp::start().await;
    let mut draft = current(&app).await;
    draft["naming"]["artist"] = json!("%ArtistName%");
    draft["naming"]["album"] = json!("%Year% - %AlbumTitle%");
    draft["naming"]["track"] = json!("%TrackNumber% - %TrackTitle%");

    let answer = app.browser().post("/api/settings/preview", draft).await;

    assert_eq!(answer.status, 200);
    let tried = answer.json();
    assert!(
        tried["preview"]["album"]
            .as_str()
            .unwrap()
            .contains("Glenn Gould/1965 - The Well-Tempered Clavier, Book 1/"),
        "{tried}"
    );
    assert!(
        tried["preview"]["album"]
            .as_str()
            .unwrap()
            .contains("1965 - The Well-Tempered Clavier, Book 1")
    );
    assert!(
        tried["preview"]["compilation"]
            .as_str()
            .unwrap()
            .contains("The Classical Adagio Collection")
    );
    assert!(tried["issues"].as_array().unwrap().is_empty());
    assert_eq!(current(&app).await["naming"]["album"], "%AlbumTitle%");
}

// ─── Checking the configuration ───

fn verdict<'a>(report: &'a Value, check: &str) -> &'a Value {
    report
        .as_array()
        .unwrap()
        .iter()
        .find(|verdict| verdict["check"] == check)
        .unwrap_or_else(|| panic!("no verdict for {check}"))
}

#[tokio::test]
async fn a_healthy_configuration_checks_out() {
    let app = TestApp::start().await;
    app.catalogue
        .server
        .on_json("/api/user/get", json!({ "id": 3, "display_name": "Ada" }));
    app.catalogue
        .server
        .on_json("/api/track/getFileUrl", json!({ "url": "https://cdn/x" }));

    let report = app.browser().get("/api/settings/check").await.json();

    assert_eq!(verdict(&report, "download_folder")["status"], "ok");
    assert_eq!(verdict(&report, "config_folder")["code"], "writable");
    assert_eq!(verdict(&report, "session")["code"], "valid");
    assert_eq!(verdict(&report, "session")["detail"], "Ada");
    assert_eq!(verdict(&report, "credentials")["status"], "ok");
}

#[tokio::test]
async fn a_folder_that_cannot_be_written_is_named() {
    let app = TestApp::start().await;
    // A file where the folder should be: nothing can be created inside
    let blocker = app.music.path().join("blocker");
    std::fs::write(&blocker, b"in the way").unwrap();
    let mut settings = current(&app).await;
    settings["download"]["directory"] = json!(blocker.join("music").to_str().unwrap());
    app.browser().put("/api/settings", settings).await;

    let report = app.browser().get("/api/settings/check").await.json();

    assert_eq!(verdict(&report, "download_folder")["status"], "error");
    assert_eq!(verdict(&report, "download_folder")["code"], "not_writable");
}

#[tokio::test]
async fn a_refused_session_and_refused_identifiers_are_reported_apart() {
    let app = TestApp::start().await;
    app.catalogue
        .server
        .on("/api/user/get", |_| Reply::error(401, "expired"));
    app.catalogue.server.on("/api/track/getFileUrl", |_| {
        Reply::error(400, "bad signature")
    });

    let report = app.browser().get("/api/settings/check").await.json();

    assert_eq!(verdict(&report, "session")["code"], "rejected");
    assert_eq!(verdict(&report, "credentials")["code"], "refused");
}

#[tokio::test]
async fn without_a_session_the_report_says_so() {
    let app = TestApp::start().await;
    app.state.client.end_session();

    let report = app.browser().get("/api/settings/check").await.json();

    assert_eq!(verdict(&report, "session")["code"], "not_signed_in");
}

#[tokio::test]
async fn a_locked_application_shows_no_settings() {
    let app = TestApp::start_with(Some("correct horse battery")).await;

    assert_eq!(app.browser().get("/api/settings").await.status, 401);
    assert_eq!(app.browser().get("/api/settings/check").await.status, 401);
}
