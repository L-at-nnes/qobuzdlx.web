mod common;

use common::app::TestApp;
use serde_json::json;

#[tokio::test]
async fn no_extension_is_listed_when_none_is_loaded() {
    let app = TestApp::start().await;

    let answer = app.browser().get("/api/extensions").await;

    assert_eq!(answer.status, 200);
    assert_eq!(answer.json(), json!({ "extensions": [], "problems": [] }));
}

#[tokio::test]
async fn the_list_of_extensions_is_behind_the_password() {
    let app = TestApp::start_with(Some("correct horse battery")).await;

    let answer = app.browser().get("/api/extensions").await;

    assert_eq!(answer.status, 401);
}
