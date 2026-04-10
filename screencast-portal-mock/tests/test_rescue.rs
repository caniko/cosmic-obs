mod helpers;

use std::collections::HashMap;

use screencast_portal_mock::{RestoreFailMode, RestoreFailReason, RestoreTokenFails};
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
async fn multi_policy_simultaneous() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::TokenNotFound,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;

    // Session with policy=Prompt (0) => response=0
    let session_0 = helpers::create_session(&client).await;
    let (resp_0, _) = helpers::select_sources(&client, &session_0, token_and_mode(0)).await;
    assert_eq!(resp_0, 0, "prompt policy should fire response=0");

    // Session with policy=Skip (1) => response=1
    let session_1 = helpers::create_session(&client).await;
    let (resp_1, _) = helpers::select_sources(&client, &session_1, token_and_mode(1)).await;
    assert_eq!(resp_1, 1, "skip policy should fire response=1");

    // Session with policy=Error (2) => response=2
    let session_2 = helpers::create_session(&client).await;
    let (resp_2, _) = helpers::select_sources(&client, &session_2, token_and_mode(2)).await;
    assert_eq!(resp_2, 2, "error policy should fire response=2");

    handle.assert_call_count(3);
    handle.assert_restore_fail_mode(0, RestoreFailMode::Prompt);
    handle.assert_restore_fail_mode(1, RestoreFailMode::Skip);
    handle.assert_restore_fail_mode(2, RestoreFailMode::Error);
}
