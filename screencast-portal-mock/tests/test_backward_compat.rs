mod helpers;

use std::collections::HashMap;

use screencast_portal_mock::{NormalSession, OldPortal, RestoreFailMode};
use zbus::zvariant::{OwnedValue, Value};

#[tokio::test]
async fn v5_portal_reports_version_5() {
    let (_bus, _handle, client) = helpers::setup(OldPortal { version: 5 }).await;
    let version = helpers::get_version(&client).await;
    assert_eq!(version, 5);
}

#[tokio::test]
async fn v6_portal_reports_version_6() {
    let (_bus, _handle, client) = helpers::setup(NormalSession).await;
    let version = helpers::get_version(&client).await;
    assert_eq!(version, 6);
}

/// An old caller (sends no v6 keys) against a v6 portal completes normally.
#[tokio::test]
async fn unpatched_client_against_v6_portal() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;

    let session = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session, HashMap::new()).await;
    assert_eq!(resp, 0, "old client should succeed against v6 portal");

    let (resp, results) = helpers::start_session(&client, &session).await;
    assert_eq!(resp, 0);
    assert!(results.contains_key("streams"));

    handle.assert_call_count(1);
    handle.assert_no_source_label(0);
    let calls = handle.calls();
    assert_eq!(
        calls[0].restore_fail_mode, None,
        "old client should not send restore_fail_mode"
    );
}

/// A new caller (sends v6 keys) against a v5 portal still completes successfully.
/// The v5 portal ignores unknown option keys.
#[tokio::test]
async fn v6_client_against_v5_portal() {
    let (_bus, handle, client) = helpers::setup(OldPortal { version: 5 }).await;

    let version = helpers::get_version(&client).await;
    assert_eq!(version, 5, "portal should report v5");

    let session = helpers::create_session(&client).await;

    // Send v6 keys even though portal is v5 — should not break the session.
    let mut opts = HashMap::new();
    opts.insert(
        "source_label".into(),
        OwnedValue::try_from(Value::new("OBS Camera".to_string())).unwrap(),
    );
    opts.insert(
        "restore_fail_mode".into(),
        OwnedValue::try_from(Value::U32(1)).unwrap(),
    );

    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 0, "v5 portal should not reject unknown keys");

    let (resp, results) = helpers::start_session(&client, &session).await;
    assert_eq!(resp, 0);
    assert!(results.contains_key("streams"));

    handle.assert_call_count(1);
}

/// Simulates proper client behaviour: check version before sending v6 keys.
/// Against a v5 portal, client should NOT send the new keys.
#[tokio::test]
async fn client_checks_version_before_sending_v6_keys() {
    let (_bus, handle, client) = helpers::setup(OldPortal { version: 5 }).await;

    let version = helpers::get_version(&client).await;

    let session = helpers::create_session(&client).await;

    let mut opts = HashMap::new();
    if version >= 6 {
        opts.insert(
            "source_label".into(),
            OwnedValue::try_from(Value::new("OBS Camera".to_string())).unwrap(),
        );
        opts.insert(
            "restore_fail_mode".into(),
            OwnedValue::try_from(Value::U32(1)).unwrap(),
        );
    }

    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 0);

    handle.assert_call_count(1);
    handle.assert_no_source_label(0);
    let calls = handle.calls();
    assert_eq!(
        calls[0].restore_fail_mode, None,
        "client should not send restore_fail_mode to v5 portal"
    );
}

/// Simulates proper client behaviour: check version and SEND v6 keys on a v6 portal.
#[tokio::test]
async fn client_sends_v6_keys_when_version_sufficient() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;

    let version = helpers::get_version(&client).await;
    assert_eq!(version, 6);

    let session = helpers::create_session(&client).await;

    let mut opts = HashMap::new();
    if version >= 6 {
        opts.insert(
            "source_label".into(),
            OwnedValue::try_from(Value::new("Desktop Capture".to_string())).unwrap(),
        );
        opts.insert(
            "restore_fail_mode".into(),
            OwnedValue::try_from(Value::U32(1)).unwrap(),
        );
    }

    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 0);

    let (resp, results) = helpers::start_session(&client, &session).await;
    assert_eq!(resp, 0);
    assert!(results.contains_key("streams"));

    handle.assert_call_count(1);
    handle.assert_source_label(0, "Desktop Capture");
    handle.assert_restore_fail_mode(0, RestoreFailMode::Skip);
}

/// Full OBS-like flow: multiple sources, each with a unique label, version-gated.
#[tokio::test]
async fn obs_like_multi_source_with_version_check() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;
    let version = helpers::get_version(&client).await;

    let labels = ["Game Capture", "Webcam", "Browser Source"];
    for label in &labels {
        let session = helpers::create_session(&client).await;
        let mut opts = HashMap::new();
        if version >= 6 {
            opts.insert(
                "source_label".into(),
                OwnedValue::try_from(Value::new(label.to_string())).unwrap(),
            );
            opts.insert(
                "restore_fail_mode".into(),
                OwnedValue::try_from(Value::U32(1)).unwrap(),
            );
        }
        let (resp, _) = helpers::select_sources(&client, &session, opts).await;
        assert_eq!(resp, 0);
    }

    handle.assert_call_count(3);
    for (i, label) in labels.iter().enumerate() {
        handle.assert_source_label(i, label);
        handle.assert_restore_fail_mode(i, RestoreFailMode::Skip);
    }
}
