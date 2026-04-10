mod helpers;

use std::collections::HashMap;

use screencast_portal_mock::NormalSession;
use zbus::zvariant::{OwnedValue, Value};

#[tokio::test]
async fn empty_string_label() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;
    let session = helpers::create_session(&client).await;

    let mut opts = HashMap::new();
    opts.insert(
        "source_label".into(),
        OwnedValue::try_from(Value::new(String::new())).unwrap(),
    );

    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 0);

    handle.assert_call_count(1);
    let calls = handle.calls();
    assert_eq!(calls[0].source_label, Some(String::new()));
}

#[tokio::test]
async fn whitespace_only_label() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;
    let session = helpers::create_session(&client).await;

    let mut opts = HashMap::new();
    opts.insert(
        "source_label".into(),
        OwnedValue::try_from(Value::new("   ".to_string())).unwrap(),
    );

    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 0);

    handle.assert_call_count(1);
    let calls = handle.calls();
    assert_eq!(calls[0].source_label, Some("   ".to_string()));
}

#[tokio::test]
async fn oversized_label() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;
    let session = helpers::create_session(&client).await;

    let big_label = "A".repeat(300);
    let mut opts = HashMap::new();
    opts.insert(
        "source_label".into(),
        OwnedValue::try_from(Value::new(big_label.clone())).unwrap(),
    );

    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 0);

    handle.assert_call_count(1);
    let calls = handle.calls();
    assert_eq!(calls[0].source_label, Some(big_label));
}

#[tokio::test]
async fn unicode_label() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;
    let session = helpers::create_session(&client).await;

    let mut opts = HashMap::new();
    opts.insert(
        "source_label".into(),
        OwnedValue::try_from(Value::new("\u{1f4f9} Camera".to_string())).unwrap(),
    );

    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 0);

    handle.assert_call_count(1);
    handle.assert_source_label(0, "\u{1f4f9} Camera");
}

#[tokio::test]
async fn control_char_label() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;
    let session = helpers::create_session(&client).await;

    let mut opts = HashMap::new();
    opts.insert(
        "source_label".into(),
        OwnedValue::try_from(Value::new("test\nrest".to_string())).unwrap(),
    );

    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 0);

    handle.assert_call_count(1);
    let calls = handle.calls();
    assert_eq!(calls[0].source_label, Some("test\nrest".to_string()));
}
