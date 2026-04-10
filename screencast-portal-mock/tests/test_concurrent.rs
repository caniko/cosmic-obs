mod helpers;

use std::collections::HashMap;

use screencast_portal_mock::NormalSession;

#[tokio::test]
async fn two_sessions_parallel() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;

    let session_a = helpers::create_session(&client).await;
    let session_b = helpers::create_session(&client).await;

    let (result_a, result_b) = tokio::join!(
        helpers::select_sources(&client, &session_a, HashMap::new()),
        helpers::select_sources(&client, &session_b, HashMap::new()),
    );

    assert_eq!(result_a.0, 0, "session A should succeed");
    assert_eq!(result_b.0, 0, "session B should succeed");

    handle.assert_call_count(2);
    let calls = handle.calls();
    assert_ne!(
        calls[0].session_handle, calls[1].session_handle,
        "sessions should have different handles"
    );
}

#[tokio::test]
async fn interleaved_create_select() {
    let (_bus, handle, client) = helpers::setup(NormalSession).await;

    // Create both sessions first
    let session_a = helpers::create_session(&client).await;
    let session_b = helpers::create_session(&client).await;

    // SelectSources on A, then B (interleaved with create order)
    let (resp_a, _) = helpers::select_sources(&client, &session_a, HashMap::new()).await;
    assert_eq!(resp_a, 0);

    let (resp_b, _) = helpers::select_sources(&client, &session_b, HashMap::new()).await;
    assert_eq!(resp_b, 0);

    handle.assert_call_count(2);
    let calls = handle.calls();
    assert_eq!(
        calls[0].session_handle, session_a,
        "first call should be session A"
    );
    assert_eq!(
        calls[1].session_handle, session_b,
        "second call should be session B"
    );
}
