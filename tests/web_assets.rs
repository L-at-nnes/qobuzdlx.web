mod common;

use common::app::TestApp;

#[tokio::test]
async fn an_unknown_api_route_is_a_json_not_found_never_a_page() {
    let app = TestApp::start().await;

    let answer = app.browser().get("/api/nothing-here").await;

    assert_eq!(answer.status, 404);
    assert_eq!(answer.json()["error"], "not_found");
}

#[tokio::test]
async fn a_missing_script_fails_instead_of_coming_back_as_html() {
    let app = TestApp::start().await;

    let answer = app.browser().get("/_app/immutable/missing.js").await;

    assert_eq!(answer.status, 404);
}

#[tokio::test]
async fn the_interface_files_carry_the_security_headers_too() {
    let app = TestApp::start().await;

    let answer = app.browser().get("/").await;

    // The interface may or may not be built when this runs; the headers are there either way
    assert!(matches!(answer.status, 200 | 503), "{}", answer.status);
    assert_eq!(answer.headers["x-frame-options"], "DENY");
    assert_eq!(answer.headers["x-content-type-options"], "nosniff");
}

#[tokio::test]
async fn the_interface_needs_no_password_to_be_loaded() {
    let app = TestApp::start_with(Some("correct horse battery")).await;

    let answer = app.browser().get("/").await;

    // It holds no data of the account: it asks for the password itself
    assert!(matches!(answer.status, 200 | 503), "{}", answer.status);
}
