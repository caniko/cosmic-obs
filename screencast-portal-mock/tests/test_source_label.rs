mod helpers;

use std::collections::HashMap;

use screencast_portal_mock::NormalSession;
use zbus::zvariant::{OwnedValue, Value};

#[tokio::test]
async fn label_recorded_when_present() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;
    let session = helpers::create_session(&client).await;

    let mut opts = HashMap::new();
    opts.insert(
        "source_label".into(),
        OwnedValue::try_from(Value::new("OBS Scene 1".to_string())).unwrap(),
    );

    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 0);

    handle.assert_call_count(1);
    handle.assert_source_label(0, "OBS Scene 1");
}

#[tokio::test]
async fn none_recorded_when_absent() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;
    let session = helpers::create_session(&client).await;

    let (resp, _) = helpers::select_sources(&client, &session, HashMap::new()).await;
    assert_eq!(resp, 0);

    handle.assert_call_count(1);
    handle.assert_no_source_label(0);
}

#[tokio::test]
async fn multiple_distinct_labels() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;

    // First call with label A
    let session1 = helpers::create_session(&client).await;
    let mut opts1 = HashMap::new();
    opts1.insert(
        "source_label".into(),
        OwnedValue::try_from(Value::new("Camera".to_string())).unwrap(),
    );
    let (resp, _) = helpers::select_sources(&client, &session1, opts1).await;
    assert_eq!(resp, 0);

    // Second call with label B
    let session2 = helpers::create_session(&client).await;
    let mut opts2 = HashMap::new();
    opts2.insert(
        "source_label".into(),
        OwnedValue::try_from(Value::new("Desktop".to_string())).unwrap(),
    );
    let (resp, _) = helpers::select_sources(&client, &session2, opts2).await;
    assert_eq!(resp, 0);

    handle.assert_call_count(2);
    handle.assert_source_label(0, "Camera");
    handle.assert_source_label(1, "Desktop");
}
