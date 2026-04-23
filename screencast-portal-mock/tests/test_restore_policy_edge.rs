mod helpers;

use std::collections::HashMap;

use screencast_portal_mock::{NormalSession, RestoreAction};
use zbus::zvariant::{OwnedValue, Value};

#[tokio::test]
async fn policy_without_token_is_recorded_but_not_applied() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;
    let session = helpers::create_session(&client).await;

    let mut opts = HashMap::new();
    opts.insert(
        "restore_policy".into(),
        helpers::restore_policy_value(Some(1), &[]),
    );

    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 0, "NormalSession succeeds regardless of policy");

    handle.assert_call_count(1);
    handle.assert_restore_default_action(0, RestoreAction::Skip);
}

#[tokio::test]
async fn invalid_policy_type_is_rejected() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;
    let session = helpers::create_session(&client).await;

    let mut opts = HashMap::new();
    opts.insert(
        "restore_policy".into(),
        OwnedValue::try_from(Value::U32(3)).unwrap(),
    );

    let err = helpers::select_sources_error(&client, &session, opts).await;
    assert!(
        err.to_string().contains("restore_policy must be an a{sv}"),
        "unexpected error: {err}"
    );
    handle.assert_call_count(0);
}

#[tokio::test]
async fn invalid_default_action_type_is_rejected() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;
    let session = helpers::create_session(&client).await;

    let mut policy: HashMap<String, OwnedValue> = HashMap::new();
    policy.insert(
        "default_action".into(),
        OwnedValue::try_from(Value::new("skip".to_string())).unwrap(),
    );

    let mut opts = HashMap::new();
    opts.insert("restore_policy".into(), OwnedValue::from(policy));

    let err = helpers::select_sources_error(&client, &session, opts).await;
    assert!(
        err.to_string()
            .contains("restore_policy.default_action must be u32"),
        "unexpected error: {err}"
    );
    handle.assert_call_count(0);
}

#[tokio::test]
async fn invalid_actions_type_is_rejected() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;
    let session = helpers::create_session(&client).await;

    let mut policy: HashMap<String, OwnedValue> = HashMap::new();
    policy.insert(
        "actions".into(),
        OwnedValue::try_from(Value::new("bad-actions".to_string())).unwrap(),
    );

    let mut opts = HashMap::new();
    opts.insert("restore_policy".into(), OwnedValue::from(policy));

    let err = helpers::select_sources_error(&client, &session, opts).await;
    assert!(
        err.to_string()
            .contains("restore_policy.actions must be a{su}"),
        "unexpected error: {err}"
    );
    handle.assert_call_count(0);
}
