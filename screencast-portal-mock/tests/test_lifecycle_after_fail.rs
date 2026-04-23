mod helpers;

use std::collections::HashMap;

use screencast_portal_mock::{
    FailThenSucceed, RestoreAction, RestoreFailReason, RestoreTokenFails,
};
use zbus::zvariant::OwnedValue;

fn token_and_policy(default_action: u32) -> HashMap<String, OwnedValue> {
    helpers::token_and_policy(Some(default_action), &[])
}

#[tokio::test]
async fn re_select_sources_after_skip() {
    let (_bus, handle, client) = helpers::setup(FailThenSucceed::new()).await;

    // First call: with stale token + default_action=Skip => response=1
    let session1 = helpers::create_session(&client).await;
    let (resp, _) =
        helpers::select_sources_and_wait_closed(&client, &session1, token_and_policy(1)).await;
    assert_eq!(resp, 1, "first call should skip (response=1)");

    // Second call: no token, no mode => succeeds (counter > 0)
    let session2 = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session2, HashMap::new()).await;
    assert_eq!(resp, 0, "second call should succeed");

    handle.assert_call_count(2);
}

#[tokio::test]
async fn start_after_error_response() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::TokenNotFound,
    };
    let (_bus, _handle, client) = helpers::setup(scenario).await;
    let session = helpers::create_session(&client).await;

    // SelectSources with default_action=Error => response=2
    let (resp, results) =
        helpers::select_sources_and_wait_closed(&client, &session, token_and_policy(2)).await;
    assert_eq!(resp, 2);
    assert_eq!(
        helpers::restore_failure_reason(&helpers::restore_failure(&results)),
        "token_not_found"
    );

    let err = helpers::start_session_error(&client, &session).await;
    assert!(
        err.to_string().contains("session is closed"),
        "Start on a closed session should fail, got: {err}"
    );
}

#[tokio::test]
async fn fresh_token_after_skip() {
    let (_bus, handle, client) = helpers::setup(FailThenSucceed::new()).await;

    // First call: stale token + default_action=Skip => skip
    let session1 = helpers::create_session(&client).await;
    let (resp, _) =
        helpers::select_sources_and_wait_closed(&client, &session1, token_and_policy(1)).await;
    assert_eq!(resp, 1, "first call should skip");

    // Second call: no token => succeeds
    let session2 = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session2, HashMap::new()).await;
    assert_eq!(resp, 0, "second call without token should succeed");

    handle.assert_call_count(2);
    handle.assert_restore_default_action(0, RestoreAction::Skip);
    let calls = handle.calls();
    assert_eq!(
        calls[1].restore_policy, None,
        "second call should have no restore_policy"
    );
}

#[tokio::test]
async fn prompt_fallback_after_skip() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::TokenNotFound,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;

    // First call: token + default_action=Skip => response=1
    let session1 = helpers::create_session(&client).await;
    let (resp, _) =
        helpers::select_sources_and_wait_closed(&client, &session1, token_and_policy(1)).await;
    assert_eq!(resp, 1, "first call should skip");

    // Second call: same token + default_action=Prompt => response=0 (prompt fallback)
    let session2 = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session2, token_and_policy(0)).await;
    assert_eq!(resp, 0, "second call with prompt should succeed");

    handle.assert_call_count(2);
    handle.assert_restore_default_action(0, RestoreAction::Skip);
    handle.assert_restore_default_action(1, RestoreAction::Prompt);
}
