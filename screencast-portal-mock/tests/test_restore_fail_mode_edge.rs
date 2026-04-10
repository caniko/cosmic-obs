mod helpers;

use std::collections::HashMap;

use screencast_portal_mock::{NormalSession, RestoreFailMode};
use zbus::zvariant::{OwnedValue, Value};

#[tokio::test]
async fn mode_without_token() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;
    let session = helpers::create_session(&client).await;

    let mut opts = HashMap::new();
    opts.insert(
        "restore_fail_mode".into(),
        OwnedValue::try_from(Value::U32(1)).unwrap(),
    );
    // No restore_token in opts

    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 0, "NormalSession succeeds regardless of mode");

    handle.assert_call_count(1);
    handle.assert_restore_fail_mode(0, RestoreFailMode::Skip);
}

#[tokio::test]
async fn invalid_mode_via_dbus() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;
    let session = helpers::create_session(&client).await;

    let mut opts = HashMap::new();
    opts.insert(
        "restore_fail_mode".into(),
        OwnedValue::try_from(Value::U32(3)).unwrap(),
    );

    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 0);

    handle.assert_call_count(1);
    let calls = handle.calls();
    assert_eq!(
        calls[0].restore_fail_mode, None,
        "TryFrom should reject value 3"
    );
}

#[tokio::test]
async fn mode_max_value() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;
    let session = helpers::create_session(&client).await;

    let mut opts = HashMap::new();
    opts.insert(
        "restore_fail_mode".into(),
        OwnedValue::try_from(Value::U32(u32::MAX)).unwrap(),
    );

    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 0);

    handle.assert_call_count(1);
    let calls = handle.calls();
    assert_eq!(
        calls[0].restore_fail_mode, None,
        "TryFrom should reject u32::MAX"
    );
}
