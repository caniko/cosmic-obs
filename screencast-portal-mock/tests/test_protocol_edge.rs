mod helpers;

use std::collections::HashMap;

use screencast_portal_mock::{ErrorWithoutFlag, NormalSession};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

#[tokio::test]
async fn select_sources_bad_handle() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;

    let fabricated =
        OwnedObjectPath::try_from("/org/freedesktop/portal/desktop/session/999".to_string())
            .unwrap();

    let (resp, _) = helpers::select_sources(&client, &fabricated, HashMap::new()).await;
    // Mock doesn't validate handles, so the call completes
    assert_eq!(resp, 0);

    handle.assert_call_count(1);
    let calls = handle.calls();
    assert_eq!(
        calls[0].session_handle.as_str(),
        "/org/freedesktop/portal/desktop/session/999"
    );
}

#[tokio::test]
async fn double_select_sources() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;
    let session = helpers::create_session(&client).await;

    // First SelectSources with label A
    let mut opts_a = HashMap::new();
    opts_a.insert(
        "source_label".into(),
        OwnedValue::try_from(Value::new("Label A".to_string())).unwrap(),
    );
    let (resp, _) = helpers::select_sources(&client, &session, opts_a).await;
    assert_eq!(resp, 0);

    // Second SelectSources with label B on the same session
    let mut opts_b = HashMap::new();
    opts_b.insert(
        "source_label".into(),
        OwnedValue::try_from(Value::new("Label B".to_string())).unwrap(),
    );
    let (resp, _) = helpers::select_sources(&client, &session, opts_b).await;
    assert_eq!(resp, 0);

    handle.assert_call_count(2);
    handle.assert_source_label(0, "Label A");
    handle.assert_source_label(1, "Label B");
}

#[tokio::test]
async fn error_response_without_flag() {
    let (_bus, _handle, client) = helpers::setup(ErrorWithoutFlag).await;
    let session = helpers::create_session(&client).await;

    let (resp, _) = helpers::select_sources(&client, &session, HashMap::new()).await;
    assert_eq!(resp, 0);

    let (resp, results) = helpers::start_session(&client, &session).await;
    assert_eq!(resp, 2, "ErrorWithoutFlag should fire response=2");
    assert!(
        !results.contains_key("restore_failure"),
        "results should NOT contain restore_failure key"
    );
}

#[tokio::test]
async fn start_results_verification() {
    let (_bus, _handle, client) = helpers::setup(NormalSession).await;
    let session = helpers::create_session(&client).await;

    let (resp, _) = helpers::select_sources(&client, &session, HashMap::new()).await;
    assert_eq!(resp, 0);

    let (resp, results) = helpers::start_session(&client, &session).await;
    assert_eq!(resp, 0);
    assert!(
        results.contains_key("streams"),
        "results should contain streams"
    );
    assert!(
        !results.contains_key("restore_failure"),
        "results should NOT contain restore_failure"
    );
}
