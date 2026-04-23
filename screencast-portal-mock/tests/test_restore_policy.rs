mod helpers;

use std::collections::HashMap;

use screencast_portal_mock::{RestoreAction, RestoreFailReason, RestoreTokenFails};
use zbus::zvariant::{OwnedValue, Value};

fn policy_options(default_action: u32) -> HashMap<String, OwnedValue> {
    helpers::token_and_policy(Some(default_action), &[])
}

fn policy_options_with_label(default_action: u32, label: &str) -> HashMap<String, OwnedValue> {
    let mut opts = policy_options(default_action);
    opts.insert(
        "source_label".into(),
        OwnedValue::try_from(Value::new(label.to_string())).unwrap(),
    );
    opts
}

#[tokio::test]
async fn skip_fires_response_1_with_failure_object() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::TokenNotFound,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;
    let session = helpers::create_session(&client).await;

    let (resp, results) = helpers::select_sources(&client, &session, policy_options(1)).await;
    assert_eq!(resp, 1, "skip policy should fire response=1");
    assert!(
        !results.contains_key("restore_failed"),
        "legacy restore_failed flag must not be emitted"
    );

    let failure = helpers::restore_failure(&results);
    assert_eq!(helpers::restore_failure_reason(&failure), "token_not_found");
    assert_eq!(helpers::restore_failure_action(&failure), 1);
    assert!(helpers::restore_failure_token_invalid(&failure));

    handle.assert_call_count(1);
    handle.assert_restore_default_action(0, RestoreAction::Skip);
}

#[tokio::test]
async fn error_fires_response_2_with_failure_object() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::PermissionRevoked,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;
    let session = helpers::create_session(&client).await;

    let (resp, results) = helpers::select_sources(&client, &session, policy_options(2)).await;
    assert_eq!(resp, 2, "error policy should fire response=2");

    let failure = helpers::restore_failure(&results);
    assert_eq!(
        helpers::restore_failure_reason(&failure),
        "permission_revoked"
    );
    assert_eq!(helpers::restore_failure_action(&failure), 2);
    assert!(helpers::restore_failure_token_invalid(&failure));

    handle.assert_call_count(1);
    handle.assert_restore_default_action(0, RestoreAction::Error);
}

#[tokio::test]
async fn prompt_fires_response_0_without_failure_object() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::SourceUnavailable,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;
    let session = helpers::create_session(&client).await;

    let (resp, results) = helpers::select_sources(&client, &session, policy_options(0)).await;
    assert_eq!(resp, 0, "prompt policy should fire response=0");
    assert!(
        !results.contains_key("restore_failure"),
        "prompt should not set restore_failure"
    );

    handle.assert_call_count(1);
    handle.assert_restore_default_action(0, RestoreAction::Prompt);
}

#[tokio::test]
async fn source_unavailable_failure_marks_token_valid() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::SourceUnavailable,
    };
    let (_bus, _handle, client) = helpers::setup(scenario).await;
    let session = helpers::create_session(&client).await;

    let (resp, results) = helpers::select_sources(&client, &session, policy_options(2)).await;
    assert_eq!(resp, 2);

    let failure = helpers::restore_failure(&results);
    assert_eq!(
        helpers::restore_failure_reason(&failure),
        "source_unavailable"
    );
    assert!(!helpers::restore_failure_token_invalid(&failure));
}

#[tokio::test]
async fn reason_specific_action_overrides_default() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::PermissionRevoked,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;
    let session = helpers::create_session(&client).await;

    let opts = helpers::token_and_policy(Some(1), &[("permission_revoked", 2)]);
    let (resp, results) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(
        resp, 2,
        "reason-specific error should override default skip"
    );

    let failure = helpers::restore_failure(&results);
    assert_eq!(helpers::restore_failure_action(&failure), 2);

    handle.assert_call_count(1);
    handle.assert_restore_default_action(0, RestoreAction::Skip);
    handle.assert_restore_reason_action(
        0,
        RestoreFailReason::PermissionRevoked,
        RestoreAction::Error,
    );
}

#[tokio::test]
async fn unknown_reason_is_ignored() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::TokenConsumed,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;
    let session = helpers::create_session(&client).await;

    let opts = helpers::token_and_policy(Some(1), &[("future_reason", 2)]);
    let (resp, results) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 1, "unknown reason entry should not override default");

    let failure = helpers::restore_failure(&results);
    assert_eq!(helpers::restore_failure_action(&failure), 1);

    let calls = handle.calls();
    assert!(
        calls[0]
            .restore_policy
            .as_ref()
            .expect("policy should be recorded")
            .actions
            .is_empty(),
        "unknown reason entries should be ignored"
    );
}

#[tokio::test]
async fn unknown_action_falls_back_to_default_then_prompt() {
    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::PermissionRevoked,
    };
    let (_bus, _handle, client) = helpers::setup(scenario).await;
    let session = helpers::create_session(&client).await;

    let opts = helpers::token_and_policy(Some(1), &[("permission_revoked", 99)]);
    let (resp, results) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 1, "invalid reason action should fall back to default");
    assert_eq!(
        helpers::restore_failure_action(&helpers::restore_failure(&results)),
        1
    );

    let session = helpers::create_session(&client).await;
    let opts = helpers::token_and_policy(None, &[("permission_revoked", 99)]);
    let (resp, results) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 0, "invalid action without default should prompt");
    assert!(!results.contains_key("restore_failure"));
}

#[tokio::test]
async fn label_with_restore_policy() {
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
    handle.assert_restore_default_action(0, RestoreAction::Skip);
}

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
        assert_eq!(
            resp, 2,
            "error policy should fire response=2 for {reason:?}"
        );

        let failure = helpers::restore_failure(&results);
        assert_eq!(helpers::restore_failure_reason(&failure), reason.as_str());

        handle.assert_call_count(1);
    }
}
