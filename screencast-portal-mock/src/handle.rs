use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::task::JoinHandle;

use crate::records::{Call, RestoreFailPolicy};

/// Errors from waiting on mock portal state.
#[derive(Debug, thiserror::Error)]
pub enum WaitError {
    #[error("timed out waiting for {expected} call(s), got {actual}")]
    Timeout { expected: usize, actual: usize },
}

/// Handle to a running mock portal, used for assertions in tests.
pub struct MockPortalHandle {
    calls: Arc<Mutex<Vec<Call>>>,
    _task: JoinHandle<()>,
}

impl MockPortalHandle {
    #[must_use]
    pub fn new(calls: Arc<Mutex<Vec<Call>>>, task: JoinHandle<()>) -> Self {
        Self { calls, _task: task }
    }

    /// Wait until at least `n` SelectSources calls have been recorded,
    /// polling every 50ms up to `timeout`.
    pub async fn wait_for_calls(&self, n: usize, timeout: Duration) -> Result<(), WaitError> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let count = self.calls.lock().expect("calls mutex poisoned").len();
            if count >= n {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(WaitError::Timeout {
                    expected: n,
                    actual: count,
                });
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Return a snapshot of all recorded `SelectSources` calls.
    #[must_use]
    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().expect("calls mutex poisoned").clone()
    }

    /// Assert exactly `n` calls were recorded.
    ///
    /// # Panics
    /// Panics if the count does not match.
    pub fn assert_call_count(&self, n: usize) {
        let actual = self.calls.lock().expect("calls mutex poisoned").len();
        assert_eq!(actual, n, "expected {n} call(s), got {actual}");
    }

    /// Assert that call at `index` has the given `source_label`.
    ///
    /// # Panics
    /// Panics if the label does not match.
    pub fn assert_source_label(&self, index: usize, expected: &str) {
        let calls = self.calls.lock().expect("calls mutex poisoned");
        let call = &calls[index];
        assert_eq!(
            call.source_label.as_deref(),
            Some(expected),
            "call[{index}] source_label mismatch"
        );
    }

    /// Assert that call at `index` has no `source_label`.
    ///
    /// # Panics
    /// Panics if a label is present.
    pub fn assert_no_source_label(&self, index: usize) {
        let calls = self.calls.lock().expect("calls mutex poisoned");
        let call = &calls[index];
        assert!(
            call.source_label.is_none(),
            "call[{index}] expected no source_label, got {:?}",
            call.source_label
        );
    }

    /// Assert that call at `index` has the given `restore_fail_policy`.
    ///
    /// # Panics
    /// Panics if the policy does not match.
    pub fn assert_restore_fail_policy(&self, index: usize, expected: RestoreFailPolicy) {
        let calls = self.calls.lock().expect("calls mutex poisoned");
        let call = &calls[index];
        assert_eq!(
            call.restore_fail_policy,
            Some(expected),
            "call[{index}] restore_fail_policy mismatch"
        );
    }

    /// Assert that no recorded calls triggered a user prompt
    /// (i.e. none have `restore_fail_policy == Some(Prompt)` with a restore failure).
    ///
    /// This is a lightweight check: it verifies that no call explicitly set
    /// the policy to `Prompt`. For full prompt detection you would need the
    /// scenario's output, which is not tracked here.
    pub fn assert_no_prompts(&self) {
        let calls = self.calls.lock().expect("calls mutex poisoned");
        for (i, call) in calls.iter().enumerate() {
            assert_ne!(
                call.restore_fail_policy,
                Some(RestoreFailPolicy::Prompt),
                "call[{i}] had restore_fail_policy=Prompt"
            );
        }
    }
}
