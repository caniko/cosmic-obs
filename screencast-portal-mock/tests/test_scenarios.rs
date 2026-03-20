mod helpers;

use std::time::{Duration, Instant};

use screencast_portal_mock::{
    MultiSource, NormalSession, RestoreTokenValid, SlowResponse, SourceDef, UserCancels,
};

#[tokio::test]
async fn normal_session_full_flow() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;

    let session = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session, Default::default()).await;
    assert_eq!(resp, 0);

    let (resp, results) = helpers::start_session(&client, &session).await;
    assert_eq!(resp, 0);
    assert!(
        results.contains_key("streams"),
        "start results should contain streams"
    );

    handle.assert_call_count(1);
}

#[tokio::test]
async fn user_cancels() {
    let (_bus, _handle, client) = helpers::setup(UserCancels).await;

    let session = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session, Default::default()).await;
    assert_eq!(resp, 0);

    let (resp, _) = helpers::start_session(&client, &session).await;
    assert_eq!(resp, 1, "UserCancels should fire response=1");
}

#[tokio::test]
async fn slow_response_timing() {
    let delay = Duration::from_millis(200);
    let scenario = SlowResponse {
        delay,
        inner: Box::new(NormalSession),
    };
    let (_bus, _handle, client) = helpers::setup(scenario).await;

    let start = Instant::now();
    let session = helpers::create_session(&client).await;
    let elapsed = start.elapsed();
    assert!(
        elapsed >= delay,
        "CreateSession should take at least {delay:?}, took {elapsed:?}"
    );

    let start = Instant::now();
    let (resp, _) = helpers::select_sources(&client, &session, Default::default()).await;
    let elapsed = start.elapsed();
    assert_eq!(resp, 0);
    assert!(
        elapsed >= delay,
        "SelectSources should take at least {delay:?}, took {elapsed:?}"
    );
}

#[tokio::test]
async fn multi_source_stream_count() {
    let scenario = MultiSource {
        sources: vec![
            SourceDef::monitor(1),
            SourceDef::monitor(2),
            SourceDef::monitor(3),
        ],
    };
    let (_bus, _handle, client) = helpers::setup(scenario).await;

    let session = helpers::create_session(&client).await;
    let _ = helpers::select_sources(&client, &session, Default::default()).await;
    let (resp, results) = helpers::start_session(&client, &session).await;
    assert_eq!(resp, 0);
    assert!(results.contains_key("streams"));
}

#[tokio::test]
async fn restore_token_valid() {
    let scenario = RestoreTokenValid {
        token: "test-token-42".to_string(),
        node_id: 42,
    };
    let (_bus, _handle, client) = helpers::setup(scenario).await;

    let session = helpers::create_session(&client).await;

    let mut opts = std::collections::HashMap::new();
    opts.insert(
        "restore_token".into(),
        zbus::zvariant::OwnedValue::try_from(zbus::zvariant::Value::new(
            "test-token-42".to_string(),
        ))
        .unwrap(),
    );
    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 0);

    let (resp, results) = helpers::start_session(&client, &session).await;
    assert_eq!(resp, 0);
    assert!(results.contains_key("restore_token"));
}

#[tokio::test]
async fn restore_token_invalid_prompt() {
    use screencast_portal_mock::{RestoreFailReason, RestoreTokenFails};

    let scenario = RestoreTokenFails {
        reason: RestoreFailReason::TokenNotFound,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;

    let session = helpers::create_session(&client).await;

    // No restore_fail_mode => defaults to Prompt => response=0
    let mut opts = std::collections::HashMap::new();
    opts.insert(
        "restore_token".into(),
        zbus::zvariant::OwnedValue::try_from(zbus::zvariant::Value::new("bad-token".to_string()))
            .unwrap(),
    );
    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    assert_eq!(resp, 0, "default policy=prompt should fire response=0");

    handle.assert_call_count(1);
}
