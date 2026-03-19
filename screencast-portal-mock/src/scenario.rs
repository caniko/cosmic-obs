use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use zbus::zvariant::OwnedValue;

use crate::records::{RestoreFailReason, RestoreFailure, SourceDef};

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
fn default_start_result() -> StartResult {
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
            let props: Vec<Value<'_>> = vec![
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
