//! Sleep-based smoke tests for the `DelayedRestore` scenario.
//!
//! `DelayedRestore` models the Cosmic portal→caller push by sleeping inside
//! `on_start`. Deterministic coverage of the same push model lives in
//! `test_rescue_controlled.rs`, which uses `ExternallyTriggeredRescue` and
//! drives rescue timing from the test rather than the scenario.
//!
//! This file retains two tests:
//!
//! 1. `prompt_mode_rescue_smoke` — minimal sleep-based success path. Kept as
//!    a smoke test for the "real sleep inside `on_start`" case. Uses a 50ms
//!    delay; the assertion is only that Start completes successfully, not
//!    that it took exactly 50ms.
//! 2. `skip_mode_surfaces_response_1_even_with_rescue_scenario` — policy
//!    assertion: when the caller sets `restore_fail_mode=Skip`, the portal
//!    surfaces `response=1` immediately via `SelectSources` and never
//!    reaches the scenario's rescue delay. This is the "permission store
//!    empty" fallback path in gaps.md §3B.

mod helpers;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use screencast_portal_mock::{DelayedRestore, RestoreFailReason};
use zbus::zvariant::{OwnedValue, Value};

fn prompt_with_stale_token() -> HashMap<String, OwnedValue> {
    let mut opts = HashMap::new();
    opts.insert(
        "restore_token".into(),
        OwnedValue::try_from(Value::new("stale-token".to_string())).unwrap(),
    );
    opts.insert(
        "restore_fail_mode".into(),
        OwnedValue::try_from(Value::U32(0)).unwrap(),
    );
    opts
}

#[tokio::test]
async fn prompt_mode_rescue_smoke() {
    let scenario = DelayedRestore {
        rescue_delay: Duration::from_millis(50),
        reason: RestoreFailReason::SourceUnavailable,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;

    let session = helpers::create_session(&client).await;

    let (resp, _) = helpers::select_sources(&client, &session, prompt_with_stale_token()).await;
    assert_eq!(resp, 0, "prompt mode should not surface a skip");

    let (resp, results) = helpers::start_session(&client, &session).await;
    assert_eq!(resp, 0, "rescue should fire late response=0");
    assert!(
        results.contains_key("streams"),
        "rescue success should deliver a stream"
    );
    handle.assert_call_count(1);
}

#[tokio::test]
async fn skip_mode_surfaces_response_1_even_with_rescue_scenario() {
    // Policy assertion: Skip mode must *not* wait for the rescue delay.
    // A 30-second delay is used to make any regression loud — the elapsed
    // bound of 5s is slack for bus warm-up, not a timing measurement.
    let scenario = DelayedRestore {
        rescue_delay: Duration::from_secs(30),
        reason: RestoreFailReason::TokenNotFound,
    };
    let (_bus, handle, client) = helpers::setup(scenario).await;

    let session = helpers::create_session(&client).await;

    let mut opts = prompt_with_stale_token();
    opts.insert(
        "restore_fail_mode".into(),
        OwnedValue::try_from(Value::U32(1)).unwrap(),
    );

    let t0 = Instant::now();
    let (resp, _) = helpers::select_sources(&client, &session, opts).await;
    let elapsed = t0.elapsed();

    assert_eq!(resp, 1, "skip mode should fire response=1");
    assert!(
        elapsed < Duration::from_secs(5),
        "skip should not wait for the rescue delay ({elapsed:?})"
    );

    handle.assert_call_count(1);
}
