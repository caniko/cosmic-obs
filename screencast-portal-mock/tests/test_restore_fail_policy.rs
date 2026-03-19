mod helpers;

use std::collections::HashMap;

use screencast_portal_mock::{RestoreFailReason, RestoreTokenFails};
use zbus::zvariant::{OwnedValue, Value};

fn policy_options(policy: u32) -> HashMap<String, OwnedValue> {
    let mut opts = HashMap::new();
    opts.insert(
        "restore_token".into(),
        OwnedValue::try_from(Value::new("stale-token".to_string())).unwrap(),
    );
    opts.insert(
        "restore_fail_policy".into(),
        OwnedValue::try_from(Value::U32(policy)).unwrap(),
    );
    opts
}

#[tokio::test]
async fn skip_fires_response_1() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::TokenNotFound,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;
    let session = helpers::create_session(&client).await;

    let (resp, results) = helpers::select_sources(
        &client,
        &session,
        policy_options(1), // Skip
    )
    .await;
    assert_eq!(resp, 1, "skip policy should fire response=1");
    assert!(
        !results.contains_key("restore_failed"),
        "skip should not set restore_failed"
    );

    handle.assert_call_count(1);
    handle.assert_restore_fail_policy(0, screencast_portal_mock::RestoreFailPolicy::Skip);
}

#[tokio::test]
async fn error_fires_response_2_with_flag() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::PermissionRevoked,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;
    let session = helpers::create_session(&client).await;

    let (resp, results) = helpers::select_sources(
        &client,
        &session,
        policy_options(2), // Error
    )
    .await;
    assert_eq!(resp, 2, "error policy should fire response=2");

    let restore_failed = results
        .get("restore_failed")
        .expect("restore_failed key should be present");
    let flag = bool::try_from(restore_failed).expect("restore_failed should be bool");
    assert!(flag, "restore_failed should be true");

    handle.assert_call_count(1);
    handle.assert_restore_fail_policy(0, screencast_portal_mock::RestoreFailPolicy::Error);
}

#[tokio::test]
async fn prompt_fires_response_0() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::SourceUnavailable,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;
    let session = helpers::create_session(&client).await;

    let (resp, results) = helpers::select_sources(
        &client,
        &session,
        policy_options(0), // Prompt
    )
    .await;
    assert_eq!(resp, 0, "prompt policy should fire response=0");
    assert!(
        !results.contains_key("restore_failed"),
        "prompt should not set restore_failed"
    );

    handle.assert_call_count(1);
    handle.assert_restore_fail_policy(0, screencast_portal_mock::RestoreFailPolicy::Prompt);
}
