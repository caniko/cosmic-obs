mod helpers;

use std::collections::HashMap;

use screencast_portal_mock::{
    FailThenSucceed, RestoreFailMode, RestoreFailReason, RestoreTokenFails,
};
use zbus::zvariant::{OwnedValue, Value};

fn token_and_mode(mode: u32) -> HashMap<String, OwnedValue> {
    let mut opts = HashMap::new();
    opts.insert(
        "restore_token".into(),
        OwnedValue::try_from(Value::new("stale-token".to_string())).unwrap(),
    );
    opts.insert(
        "restore_fail_mode".into(),
        OwnedValue::try_from(Value::U32(mode)).unwrap(),
    );
    opts
}

#[tokio::test]
async fn re_select_sources_after_skip() {
    let (_bus, handle, client) = helpers::setup(FailThenSucceed::new()).await;

    // First call: with stale token + mode=Skip => response=1
    let session1 = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session1, token_and_mode(1)).await;
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

    // SelectSources with mode=Error => response=2
    let (resp, results) = helpers::select_sources(&client, &session, token_and_mode(2)).await;
    assert_eq!(resp, 2);
    assert!(bool::try_from(results.get("restore_failed").unwrap()).unwrap());

    // Start on the same session should still return a response (mock doesn't tear down sessions)
    let (start_resp, _) = helpers::start_session(&client, &session).await;
    // RestoreTokenFails does not override on_start, so default_start_result fires => response=0
    assert_eq!(start_resp, 0, "Start should still return a response");
}

#[tokio::test]
async fn fresh_token_after_skip() {
    let (_bus, handle, client) = helpers::setup(FailThenSucceed::new()).await;

    // First call: stale token + mode=Skip => skip
    let session1 = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session1, token_and_mode(1)).await;
    assert_eq!(resp, 1, "first call should skip");

    // Second call: no token => succeeds
    let session2 = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session2, HashMap::new()).await;
    assert_eq!(resp, 0, "second call without token should succeed");

    handle.assert_call_count(2);
    handle.assert_restore_fail_mode(0, RestoreFailMode::Skip);
    let calls = handle.calls();
    assert_eq!(
        calls[1].restore_fail_mode, None,
        "second call should have no restore_fail_mode"
    );
}

#[tokio::test]
async fn prompt_fallback_after_skip() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::TokenNotFound,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;

    // First call: token + mode=Skip => response=1
    let session1 = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session1, token_and_mode(1)).await;
    assert_eq!(resp, 1, "first call should skip");

    // Second call: same token + mode=Prompt => response=0 (prompt fallback)
    let session2 = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session2, token_and_mode(0)).await;
    assert_eq!(resp, 0, "second call with prompt should succeed");

    handle.assert_call_count(2);
    handle.assert_restore_fail_mode(0, RestoreFailMode::Skip);
    handle.assert_restore_fail_mode(1, RestoreFailMode::Prompt);
}
