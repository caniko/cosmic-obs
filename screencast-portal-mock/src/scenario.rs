use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use zbus::zvariant::OwnedValue;

use crate::records::{
    RestoreAction, RestoreFailReason, RestoreFailure, SourceDef, restore_failure_value,
};

/// Convenience alias for the options vardict.
pub type Options = HashMap<String, OwnedValue>;

/// Convenience alias for extra result entries.
pub type ExtraResults = HashMap<String, OwnedValue>;

/// Result returned by [`Scenario::on_start`].
#[derive(Debug, Clone, Default)]
pub struct StartResult {
    pub response: u32,
    pub results: ExtraResults,
}

/// Defines the behaviour of the mock portal for a particular test.
#[async_trait]
pub trait Scenario: Send + Sync + 'static {
    /// Called when `CreateSession` is invoked.
    async fn on_create_session(&self, _options: &Options) -> ExtraResults {
        HashMap::new()
    }

    /// Called when `SelectSources` is invoked.
    /// Return `Err(RestoreFailure)` to simulate a restore-token failure.
    async fn on_select_sources(&self, _options: &Options) -> Result<ExtraResults, RestoreFailure> {
        Ok(HashMap::new())
    }

    /// Called when `Start` is invoked.
    async fn on_start(&self, _options: &Options) -> StartResult {
        default_start_result()
    }

    /// The interface version to advertise.
    fn version(&self) -> u32 {
        6
    }
}

/// Build the default `StartResult`: response=0, one 1920x1080 stream with node_id=1.
#[must_use]
pub(crate) fn default_start_result() -> StartResult {
    let streams = encode_streams(&[SourceDef::monitor(1)]);
    let mut results = HashMap::new();
    results.insert("streams".into(), streams);
    StartResult {
        response: 0,
        results,
    }
}

/// Encode a slice of [`SourceDef`] into the `a(ua{sv})` GVariant expected by portal consumers.
#[must_use]
pub fn encode_streams(sources: &[SourceDef]) -> OwnedValue {
    use zbus::zvariant::Value;

    let entries: Vec<Value<'_>> = sources
        .iter()
        .map(|s| {
            let mut props: Vec<Value<'_>> = vec![
                Value::new(zbus::zvariant::Structure::from((
                    Value::new("source_type".to_string()),
                    Value::new(zbus::zvariant::Value::U32(1)),
                ))),
                Value::new(zbus::zvariant::Structure::from((
                    Value::new("size".to_string()),
                    Value::new(zbus::zvariant::Value::new((
                        s.size.0 as i32,
                        s.size.1 as i32,
                    ))),
                ))),
                Value::new(zbus::zvariant::Structure::from((
                    Value::new("position".to_string()),
                    Value::new(zbus::zvariant::Value::new((s.position.0, s.position.1))),
                ))),
            ];
            if let Some(label) = &s.expected_label {
                props.push(Value::new(zbus::zvariant::Structure::from((
                    Value::new("source_label".to_string()),
                    Value::new(label.clone()),
                ))));
            }

            Value::new(zbus::zvariant::Structure::from((
                Value::U32(s.node_id),
                Value::new(props),
            )))
        })
        .collect();

    OwnedValue::try_from(Value::new(entries)).expect("stream encoding is infallible")
}

/// Normal session: everything succeeds with defaults.
pub struct NormalSession;

#[async_trait]
impl Scenario for NormalSession {}

/// Restore token is valid: `on_select_sources` succeeds and `on_start` returns the given node_id.
pub struct RestoreTokenValid {
    pub token: String,
    pub node_id: u32,
}

#[async_trait]
impl Scenario for RestoreTokenValid {
    async fn on_start(&self, _options: &Options) -> StartResult {
        let streams = encode_streams(&[SourceDef::monitor(self.node_id)]);
        let mut results = HashMap::new();
        results.insert("streams".into(), streams);
        results.insert(
            "restore_token".into(),
            OwnedValue::try_from(zbus::zvariant::Value::new(self.token.clone()))
                .expect("string is infallible"),
        );
        StartResult {
            response: 0,
            results,
        }
    }
}

/// Restore token is invalid: `on_select_sources` returns an error.
pub struct RestoreTokenFails {
    pub reason: RestoreFailReason,
}

#[async_trait]
impl Scenario for RestoreTokenFails {
    async fn on_select_sources(&self, _options: &Options) -> Result<ExtraResults, RestoreFailure> {
        Err(RestoreFailure {
            reason: self.reason,
        })
    }
}

/// User cancels: `on_start` returns response=1.
pub struct UserCancels;

#[async_trait]
impl Scenario for UserCancels {
    async fn on_start(&self, _options: &Options) -> StartResult {
        StartResult {
            response: 1,
            results: HashMap::new(),
        }
    }
}

/// Multiple source streams returned by `on_start`.
pub struct MultiSource {
    pub sources: Vec<SourceDef>,
}

#[async_trait]
impl Scenario for MultiSource {
    async fn on_start(&self, _options: &Options) -> StartResult {
        if self.sources.iter().any(|source| !source.restore_valid) {
            let mut results = HashMap::new();
            results.insert(
                "restore_failure".into(),
                restore_failure_value(RestoreFailReason::SourceUnavailable, RestoreAction::Error),
            );
            return StartResult {
                response: 2,
                results,
            };
        }

        let streams = encode_streams(&self.sources);
        let mut results = HashMap::new();
        results.insert("streams".into(), streams);
        StartResult {
            response: 0,
            results,
        }
    }
}

/// Wraps another scenario, adding a delay before each response.
/// Uses `Box<dyn Scenario>` because the inner scenario is caller-chosen at runtime.
pub struct SlowResponse {
    pub delay: Duration,
    pub inner: Box<dyn Scenario>,
}

#[async_trait]
impl Scenario for SlowResponse {
    async fn on_create_session(&self, options: &Options) -> ExtraResults {
        tokio::time::sleep(self.delay).await;
        self.inner.on_create_session(options).await
    }

    async fn on_select_sources(&self, options: &Options) -> Result<ExtraResults, RestoreFailure> {
        tokio::time::sleep(self.delay).await;
        self.inner.on_select_sources(options).await
    }

    async fn on_start(&self, options: &Options) -> StartResult {
        tokio::time::sleep(self.delay).await;
        self.inner.on_start(options).await
    }

    fn version(&self) -> u32 {
        self.inner.version()
    }
}

/// Simulates an older portal that reports a lower version number.
pub struct OldPortal {
    pub version: u32,
}

#[async_trait]
impl Scenario for OldPortal {
    fn version(&self) -> u32 {
        self.version
    }
}

/// Fails the first `on_select_sources` call, succeeds on subsequent calls.
/// Uses an `AtomicUsize` counter to track invocation count.
pub struct FailThenSucceed {
    counter: AtomicUsize,
}

impl FailThenSucceed {
    /// Create a new `FailThenSucceed` scenario.
    #[must_use]
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }
}

impl Default for FailThenSucceed {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Scenario for FailThenSucceed {
    async fn on_select_sources(&self, _options: &Options) -> Result<ExtraResults, RestoreFailure> {
        let call_num = self.counter.fetch_add(1, Ordering::SeqCst);
        if call_num == 0 {
            Err(RestoreFailure {
                reason: RestoreFailReason::TokenNotFound,
            })
        } else {
            Ok(HashMap::new())
        }
    }
}

/// Simulates a Cosmic-style server-side rescue: the first `on_select_sources`
/// appears to fail (returns `Err`) but `on_start` later succeeds with the given
/// delay. This models a portal that holds the session open while waiting for a
/// missing window to reappear, then fires the late Start response.
///
/// Use this to verify caller behaviour when a stale restore_token is honoured
/// asynchronously after an initial skip would otherwise have fired.
pub struct DelayedRestore {
    pub rescue_delay: Duration,
    pub reason: RestoreFailReason,
}

#[async_trait]
impl Scenario for DelayedRestore {
    async fn on_select_sources(&self, _options: &Options) -> Result<ExtraResults, RestoreFailure> {
        // Report the token failure — the portal layer will convert this into
        // the appropriate response according to the caller's restore_policy.
        // For rescue semantics the caller is expected to send action=Prompt, so
        // the portal fires response=0 and SelectSources appears to succeed;
        // the rescue delay is then paid during `on_start`.
        Err(RestoreFailure {
            reason: self.reason,
        })
    }

    async fn on_start(&self, _options: &Options) -> StartResult {
        tokio::time::sleep(self.rescue_delay).await;
        default_start_result()
    }
}

/// `on_start` returns response=2 with empty results (no `restore_failure` key).
/// This simulates an error response that lacks the expected failure object.
pub struct ErrorWithoutFlag;

#[async_trait]
impl Scenario for ErrorWithoutFlag {
    async fn on_start(&self, _options: &Options) -> StartResult {
        StartResult {
            response: 2,
            results: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Externally-triggered rescue — deterministic portal→caller push model
// ---------------------------------------------------------------------------
//
// `DelayedRestore` above fakes the Cosmic rescue hook by sleeping inside
// `on_start`. That works but leaves three gaps: sleep-based assertions are
// flaky, rescue cancellation can't be driven from a test, and failure
// responses (rescue timed out / rescue resolved to error) can't be expressed.
//
// `ExternallyTriggeredRescue` models the real shape: `on_start` awaits a
// oneshot channel whose sender (`RescueController`) lives in the test. The
// test triggers rescue success, failure, or cancellation at a deterministic
// point and asserts on the resulting Start response.

/// Outcome the test drives into a pending rescue.
#[derive(Debug)]
pub enum RescueOutcome {
    /// Rescue succeeded — Start fires `response=0` with the default stream.
    Succeed,
    /// Rescue succeeded with a custom `StartResult` (multi-stream tests,
    /// explicit `restore_failure` object, etc.).
    SucceedWith(StartResult),
    /// Rescue resolved to failure — Start fires the given response code
    /// (1 = cancelled, 2 = error) with empty results.
    Fail { response: u32 },
}

/// Test-side handle for triggering an in-flight rescue.
///
/// Dropping the controller without triggering is equivalent to
/// `Fail { response: 1 }` — matches the "session closed mid-rescue"
/// semantics of the real Cosmic portal where a dropped `oneshot::Sender`
/// in `pending_rescues` ends the rescue with a cancelled response.
#[must_use = "Drop the controller or call a trigger method to resolve the rescue"]
pub struct RescueController {
    tx: tokio::sync::oneshot::Sender<RescueOutcome>,
}

impl RescueController {
    /// Fire the default success `StartResult` (one 1920x1080 stream).
    pub fn trigger_succeed(self) {
        let _ = self.tx.send(RescueOutcome::Succeed);
    }

    /// Fire a custom success payload.
    pub fn trigger_succeed_with(self, result: StartResult) {
        let _ = self.tx.send(RescueOutcome::SucceedWith(result));
    }

    /// Fire a failure response (1 = cancelled, 2 = error) with empty results.
    pub fn trigger_fail(self, response: u32) {
        let _ = self.tx.send(RescueOutcome::Fail { response });
    }
}

/// Scenario whose `on_start` blocks on an externally-triggered rescue.
///
/// `on_select_sources` returns `Err(RestoreFailure)` so that under
/// `restore_policy.default_action=Prompt` the portal converts it to `response=0` and
/// the caller proceeds to Start, where the rescue await happens. Pair with
/// `RescueController` to drive the rescue outcome from the test.
///
/// Pair this scenario with its controller via [`Self::new`]; the returned
/// tuple lets the test destructure the controller before moving the
/// scenario into the mock portal.
pub struct ExternallyTriggeredRescue {
    rx: tokio::sync::Mutex<Option<tokio::sync::oneshot::Receiver<RescueOutcome>>>,
    pub reason: RestoreFailReason,
}

impl ExternallyTriggeredRescue {
    /// Build a paired scenario and controller.
    ///
    /// The scenario is passed to [`crate::MockPortal::start`] while the
    /// controller is retained by the test to drive rescue timing.
    pub fn new(reason: RestoreFailReason) -> (Self, RescueController) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        (
            Self {
                rx: tokio::sync::Mutex::new(Some(rx)),
                reason,
            },
            RescueController { tx },
        )
    }
}

#[async_trait]
impl Scenario for ExternallyTriggeredRescue {
    async fn on_select_sources(&self, _options: &Options) -> Result<ExtraResults, RestoreFailure> {
        Err(RestoreFailure {
            reason: self.reason,
        })
    }

    async fn on_start(&self, _options: &Options) -> StartResult {
        // Take the receiver exactly once. If portal.rs ever grows a retry
        // path this will panic, which is the correct failure mode: tests
        // should create a fresh scenario per retry.
        let rx = {
            let mut guard = self.rx.lock().await;
            guard
                .take()
                .expect("ExternallyTriggeredRescue::on_start called twice")
        };

        match rx.await {
            Ok(RescueOutcome::Succeed) => default_start_result(),
            Ok(RescueOutcome::SucceedWith(r)) => r,
            Ok(RescueOutcome::Fail { response }) => StartResult {
                response,
                results: HashMap::new(),
            },
            Err(_) => StartResult {
                // Controller dropped without triggering → cancelled.
                response: 1,
                results: HashMap::new(),
            },
        }
    }
}
