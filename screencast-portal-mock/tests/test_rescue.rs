mod helpers;

use std::collections::HashMap;

use screencast_portal_mock::{RestoreAction, RestoreFailReason, RestoreTokenFails};
use zbus::zvariant::OwnedValue;

fn token_and_policy(default_action: u32) -> HashMap<String, OwnedValue> {
    helpers::token_and_policy(Some(default_action), &[])
}

#[tokio::test]
async fn multi_policy_simultaneous() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::TokenNotFound,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;

    // Session with default_action=Prompt (0) => response=0
    let session_0 = helpers::create_session(&client).await;
    let (resp_0, _) = helpers::select_sources(&client, &session_0, token_and_policy(0)).await;
    assert_eq!(resp_0, 0, "prompt policy should fire response=0");

    // Session with default_action=Skip (1) => response=1
    let session_1 = helpers::create_session(&client).await;
    let (resp_1, _) = helpers::select_sources(&client, &session_1, token_and_policy(1)).await;
    assert_eq!(resp_1, 1, "skip policy should fire response=1");

    // Session with default_action=Error (2) => response=2
    let session_2 = helpers::create_session(&client).await;
    let (resp_2, _) = helpers::select_sources(&client, &session_2, token_and_policy(2)).await;
    assert_eq!(resp_2, 2, "error policy should fire response=2");

    handle.assert_call_count(3);
    handle.assert_restore_default_action(0, RestoreAction::Prompt);
    handle.assert_restore_default_action(1, RestoreAction::Skip);
    handle.assert_restore_default_action(2, RestoreAction::Error);
}
