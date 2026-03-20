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
        "restore_fail_mode".into(),
        OwnedValue::try_from(Value::U32(policy)).unwrap(),
    );
    opts
}

fn policy_options_with_label(policy: u32, label: &str) -> HashMap<String, OwnedValue> {
    let mut opts = policy_options(policy);
    opts.insert(
        "source_label".into(),
        OwnedValue::try_from(Value::new(label.to_string())).unwrap(),
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
    handle.assert_restore_fail_mode(0, screencast_portal_mock::RestoreFailMode::Skip);
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
    handle.assert_restore_fail_mode(0, screencast_portal_mock::RestoreFailMode::Error);
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
    handle.assert_restore_fail_mode(0, screencast_portal_mock::RestoreFailMode::Prompt);
}

// ---------------------------------------------------------------------------
// Session cleanup tests — verify session is torn down after skip/error
// ---------------------------------------------------------------------------

/// After policy=Skip fires response=1, calling Start on the same session
/// should still work (the mock does not tear down sessions, matching the
/// real portal where the *caller* decides to close the session).
/// What we verify: the SelectSources completed and the policy was honoured.
#[tokio::test]
async fn skip_session_state_after_response() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::TokenNotFound,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;
    let session = helpers::create_session(&client).await;

    let (resp, _) = helpers::select_sources(&client, &session, policy_options(1)).await;
    assert_eq!(resp, 1);

    handle.assert_call_count(1);
    handle.assert_restore_fail_mode(0, screencast_portal_mock::RestoreFailMode::Skip);
}

/// After policy=Error fires response=2, verify the restore_failed flag is set
/// and the call is properly recorded.
#[tokio::test]
async fn error_session_state_after_response() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::SourceUnavailable,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;
    let session = helpers::create_session(&client).await;

    let (resp, results) = helpers::select_sources(&client, &session, policy_options(2)).await;
    assert_eq!(resp, 2);

    let restore_failed = results
        .get("restore_failed")
        .expect("restore_failed should be present");
    let flag = bool::try_from(restore_failed).expect("should be bool");
    assert!(flag);

    handle.assert_call_count(1);
    handle.assert_restore_fail_mode(0, screencast_portal_mock::RestoreFailMode::Error);
}

// ---------------------------------------------------------------------------
// Combined feature tests — source_label + restore_fail_mode together
// ---------------------------------------------------------------------------

/// Send both source_label and restore_fail_mode=Skip together.
/// Verify both are recorded and the skip response fires correctly.
#[tokio::test]
async fn label_with_skip_policy() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::TokenNotFound,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;
    let session = helpers::create_session(&client).await;

    let opts = policy_options_with_label(1, "OBS Camera");
    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 1, "skip should fire response=1");

    handle.assert_call_count(1);
    handle.assert_source_label(0, "OBS Camera");
    handle.assert_restore_fail_mode(0, screencast_portal_mock::RestoreFailMode::Skip);
}

/// Send both source_label and restore_fail_mode=Error together.
/// Verify label is recorded even when restore fails with error response.
#[tokio::test]
async fn label_with_error_policy() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::PermissionRevoked,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;
    let session = helpers::create_session(&client).await;

    let opts = policy_options_with_label(2, "Desktop Share");
    let (resp, results) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 2);
    assert!(bool::try_from(results.get("restore_failed").unwrap()).unwrap());

    handle.assert_call_count(1);
    handle.assert_source_label(0, "Desktop Share");
    handle.assert_restore_fail_mode(0, screencast_portal_mock::RestoreFailMode::Error);
}

/// Send both source_label and restore_fail_mode=Prompt together.
/// Verify label survives when restore falls back to the picker.
#[tokio::test]
async fn label_with_prompt_policy() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::SourceUnavailable,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;
    let session = helpers::create_session(&client).await;

    let opts = policy_options_with_label(0, "Browser Window");
    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 0, "prompt should fire response=0");

    handle.assert_call_count(1);
    handle.assert_source_label(0, "Browser Window");
    handle.assert_restore_fail_mode(0, screencast_portal_mock::RestoreFailMode::Prompt);
}

/// Each of the four RestoreFailReason variants should work with policy=Error.
#[tokio::test]
async fn all_fail_reasons_with_error_policy() {
    let reasons = [
        RestoreFailReason::TokenNotFound,
        RestoreFailReason::SourceUnavailable,
        RestoreFailReason::PermissionRevoked,
        RestoreFailReason::TokenConsumed,
    ];

    for reason in reasons {
        let scenario = RestoreTokenFails { reason };
        let (_bus, handle, client) = helpers::setup(scenario).await;
        let session = helpers::create_session(&client).await;

        let (resp, results) = helpers::select_sources(&client, &session, policy_options(2)).await;
        assert_eq!(resp, 2, "error policy should fire response=2 for {reason:?}");
        assert!(
            bool::try_from(results.get("restore_failed").unwrap()).unwrap(),
            "restore_failed should be true for {reason:?}"
        );

        handle.assert_call_count(1);
    }
}
