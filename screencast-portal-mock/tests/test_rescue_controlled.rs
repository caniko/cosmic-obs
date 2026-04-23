//! Deterministic rescue tests using `ExternallyTriggeredRescue` and its
//! paired `RescueController`. These replace the sleep-based assertions in
//! `test_rescue_delayed.rs` and exercise paths the sleep model can't reach:
//! explicit failure responses and cancel-by-drop.
//!
//! The scenario's `on_start` awaits a `tokio::sync::oneshot::Receiver`. The
//! test drives the outcome by calling a trigger method on the controller
//! (or dropping it for cancellation). No wall-clock assertions beyond a
//! small poll window to confirm Start is blocked before the trigger fires.

mod helpers;

use std::collections::HashMap;
use std::time::Duration;

use screencast_portal_mock::{ExternallyTriggeredRescue, RestoreFailReason};
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

/// Build a `SelectSources` options dict with a stale restore token and
/// `restore_policy.default_action=Prompt` (0). Under Prompt the mock portal converts
/// the scenario's `Err(RestoreFailure)` into `response=0`, so the caller
/// proceeds to Start where the rescue await happens.
fn prompt_with_stale_token() -> HashMap<String, OwnedValue> {
    helpers::token_and_policy(Some(0), &[])
}

/// Poll `start_fut.is_finished()` for ~50ms to confirm Start is still
/// blocked on the rescue await. Returns once the deadline expires. Panics
/// if Start completes before the deadline — the rescue hook should be
/// pending until the test triggers it.
async fn assert_start_blocked<T>(task: &tokio::task::JoinHandle<T>) {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(50);
    while tokio::time::Instant::now() < deadline {
        if task.is_finished() {
            panic!("Start completed before rescue was triggered");
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// Spawn a Start call and return its JoinHandle. The connection and
/// session handle are cloned so the caller can continue driving the
/// scenario while Start is pending.
fn spawn_start(
    client: zbus::Connection,
    session: OwnedObjectPath,
) -> tokio::task::JoinHandle<(u32, HashMap<String, OwnedValue>)> {
    tokio::spawn(async move { helpers::start_session(&client, &session).await })
}

#[tokio::test]
async fn rescue_start_blocks_until_triggered() {
    let (scenario, rescue) = ExternallyTriggeredRescue::new(RestoreFailReason::SourceUnavailable);
    let (_bus, _handle, client) = helpers::setup(scenario).await;

    let session = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session, prompt_with_stale_token()).await;
    assert_eq!(resp, 0, "prompt mode should not surface a skip");

    let start = spawn_start(client.clone(), session);
    assert_start_blocked(&start).await;

    rescue.trigger_succeed();
    let (resp, results) = start.await.expect("start task panicked");

    assert_eq!(resp, 0, "rescue succeed should fire response=0");
    assert!(
        results.contains_key("streams"),
        "success payload should include streams"
    );
}

#[tokio::test]
async fn rescue_trigger_fail_1_surfaces_cancelled() {
    let (scenario, rescue) = ExternallyTriggeredRescue::new(RestoreFailReason::TokenNotFound);
    let (_bus, _handle, client) = helpers::setup(scenario).await;

    let session = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session, prompt_with_stale_token()).await;
    assert_eq!(resp, 0);

    let start = spawn_start(client.clone(), session);
    assert_start_blocked(&start).await;

    rescue.trigger_fail(1);
    let (resp, results) = start.await.expect("start task panicked");

    assert_eq!(resp, 1, "rescue trigger_fail(1) should fire response=1");
    assert!(
        !results.contains_key("streams"),
        "failure response must not include streams"
    );
}

#[tokio::test]
async fn rescue_trigger_fail_2_surfaces_error() {
    let (scenario, rescue) = ExternallyTriggeredRescue::new(RestoreFailReason::PermissionRevoked);
    let (_bus, _handle, client) = helpers::setup(scenario).await;

    let session = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session, prompt_with_stale_token()).await;
    assert_eq!(resp, 0);

    let start = spawn_start(client.clone(), session);
    assert_start_blocked(&start).await;

    rescue.trigger_fail(2);
    let (resp, _) = start.await.expect("start task panicked");

    assert_eq!(resp, 2, "rescue trigger_fail(2) should fire response=2");
}

#[tokio::test]
async fn rescue_cancel_by_drop_surfaces_response_1() {
    let (scenario, rescue) = ExternallyTriggeredRescue::new(RestoreFailReason::TokenConsumed);
    let (_bus, _handle, client) = helpers::setup(scenario).await;

    let session = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session, prompt_with_stale_token()).await;
    assert_eq!(resp, 0);

    let start = spawn_start(client.clone(), session);
    assert_start_blocked(&start).await;

    // Dropping the controller closes the oneshot sender. The scenario's
    // receiver then errors, which maps to `response=1` — mirrors Cosmic's
    // "session closed mid-rescue" path.
    drop(rescue);
    let (resp, results) = start.await.expect("start task panicked");

    assert_eq!(resp, 1, "dropped controller should fire response=1");
    assert!(!results.contains_key("streams"));
}

#[tokio::test]
async fn rescue_cancel_after_session_close_is_clean() {
    // Call Close on the session object mid-rescue, then drop the
    // controller. Verify both operations complete without panicking and
    // that Start resolves to response=1. The mock's `SessionObject::close`
    // is a no-op at the D-Bus level, so this test exercises the worst
    // case: the client Close signal has no effect on the scenario, and
    // the controller drop is what actually ends the rescue.
    let (scenario, rescue) = ExternallyTriggeredRescue::new(RestoreFailReason::SourceUnavailable);
    let (_bus, _handle, client) = helpers::setup(scenario).await;

    let session = helpers::create_session(&client).await;
    let (resp, _) = helpers::select_sources(&client, &session, prompt_with_stale_token()).await;
    assert_eq!(resp, 0);

    let start = spawn_start(client.clone(), session.clone());
    assert_start_blocked(&start).await;

    // Call Session::Close via D-Bus. The mock's handler is a no-op, so
    // this should return Ok without affecting the pending rescue.
    let session_proxy = zbus::Proxy::new(
        &client,
        "org.freedesktop.portal.Desktop",
        session.as_ref(),
        "org.freedesktop.portal.Session",
    )
    .await
    .expect("create session proxy");
    session_proxy
        .call::<_, _, ()>("Close", &())
        .await
        .expect("session close should not error");

    // Now drop the controller — this is what actually ends the rescue.
    drop(rescue);
    let (resp, _) = start.await.expect("start task panicked");

    assert_eq!(
        resp, 1,
        "cancel-by-drop after session close should surface response=1"
    );
}
