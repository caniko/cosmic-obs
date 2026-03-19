use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

use crate::records::{Call, RestoreFailPolicy, SourceTypes};
use crate::scenario::Scenario;

/// Errors from the portal server.
#[derive(Debug, thiserror::Error)]
pub enum PortalError {
    #[error("zbus error: {0}")]
    Zbus(#[from] zbus::Error),
}

// ---------------------------------------------------------------------------
// Request object
// ---------------------------------------------------------------------------

/// Implements `org.freedesktop.portal.Request`.
pub struct RequestObject;

#[zbus::interface(name = "org.freedesktop.portal.Request")]
impl RequestObject {
    fn close(&self) {}

    #[zbus(signal)]
    async fn response(
        emitter: &SignalEmitter<'_>,
        response: u32,
        results: HashMap<String, OwnedValue>,
    ) -> zbus::Result<()>;
}

// ---------------------------------------------------------------------------
// Session object
// ---------------------------------------------------------------------------

/// Implements `org.freedesktop.portal.Session`.
pub struct SessionObject;

#[zbus::interface(name = "org.freedesktop.portal.Session")]
impl SessionObject {
    fn close(&self) {}

    #[zbus(signal)]
    async fn closed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}

// ---------------------------------------------------------------------------
// ScreenCast portal
// ---------------------------------------------------------------------------

/// Implements `org.freedesktop.portal.ScreenCast`.
pub struct ScreenCastPortal {
    scenario: Arc<dyn Scenario>,
    calls: Arc<Mutex<Vec<Call>>>,
    conn: zbus::Connection,
    counter: Arc<AtomicU64>,
}

impl ScreenCastPortal {
    #[must_use]
    pub fn new(
        scenario: Arc<dyn Scenario>,
        calls: Arc<Mutex<Vec<Call>>>,
        conn: zbus::Connection,
        counter: Arc<AtomicU64>,
    ) -> Self {
        Self {
            scenario,
            calls,
            conn,
            counter,
        }
    }

    fn next_id(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::Relaxed)
    }
}

#[zbus::interface(name = "org.freedesktop.portal.ScreenCast")]
impl ScreenCastPortal {
    async fn create_session(
        &self,
        options: HashMap<String, OwnedValue>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let id = self.next_id();
        let session_path =
            OwnedObjectPath::try_from(format!("/org/freedesktop/portal/desktop/session/{id}"))
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        let request_path =
            OwnedObjectPath::try_from(format!("/org/freedesktop/portal/desktop/request/{id}"))
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        let conn = self.conn.clone();
        let scenario = self.scenario.clone();
        let sp = session_path.clone();
        let rp = request_path.clone();

        // Register session and request objects
        conn.object_server()
            .at(&session_path, SessionObject)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        conn.object_server()
            .at(&request_path, RequestObject)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        tokio::spawn(async move {
            let extra = scenario.on_create_session(&options).await;
            let mut results: HashMap<String, OwnedValue> = extra;
            results.insert(
                "session_handle".into(),
                OwnedValue::try_from(Value::new(sp.as_str().to_owned()))
                    .expect("session_handle string-to-OwnedValue is infallible"),
            );
            fire_response(&conn, &rp, 0, results).await;
        });

        Ok(request_path)
    }

    async fn select_sources(
        &self,
        session_handle: OwnedObjectPath,
        options: HashMap<String, OwnedValue>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let id = self.next_id();
        let request_path =
            OwnedObjectPath::try_from(format!("/org/freedesktop/portal/desktop/request/{id}"))
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        // Parse call fields from options
        let source_types = options
            .get("types")
            .and_then(|v| <u32>::try_from(v).ok())
            .map(SourceTypes::from_bits_truncate)
            .unwrap_or(SourceTypes::MONITOR);

        let multiple = options
            .get("multiple")
            .and_then(|v| <bool>::try_from(v).ok())
            .unwrap_or(false);

        let cursor_mode = options
            .get("cursor_mode")
            .and_then(|v| <u32>::try_from(v).ok())
            .unwrap_or(0);

        let persist_mode = options
            .get("persist_mode")
            .and_then(|v| <u32>::try_from(v).ok())
            .unwrap_or(0);

        let restore_token = options
            .get("restore_token")
            .and_then(|v| v.downcast_ref::<zbus::zvariant::Str<'_>>().ok())
            .map(|s| s.to_string());

        let source_label = options
            .get("source_label")
            .and_then(|v| v.downcast_ref::<zbus::zvariant::Str<'_>>().ok())
            .map(|s| s.to_string());

        let restore_fail_policy = options
            .get("restore_fail_policy")
            .and_then(|v| <u32>::try_from(v).ok())
            .and_then(|v| RestoreFailPolicy::try_from(v).ok());

        let call = Call {
            timestamp: std::time::Instant::now(),
            session_handle: session_handle.clone(),
            source_types,
            multiple,
            cursor_mode,
            persist_mode,
            restore_token,
            source_label,
            restore_fail_policy,
            raw_options: options.clone(),
        };

        self.calls.lock().expect("calls mutex poisoned").push(call);

        let conn = self.conn.clone();
        let scenario = self.scenario.clone();
        let rp = request_path.clone();
        let policy = restore_fail_policy.unwrap_or(RestoreFailPolicy::Prompt);

        conn.object_server()
            .at(&request_path, RequestObject)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        tokio::spawn(async move {
            let result = scenario.on_select_sources(&options).await;
            match result {
                Ok(extra) => {
                    fire_response(&conn, &rp, 0, extra).await;
                }
                Err(_failure) => match policy {
                    RestoreFailPolicy::Prompt => {
                        fire_response(&conn, &rp, 0, HashMap::new()).await;
                    }
                    RestoreFailPolicy::Skip => {
                        fire_response(&conn, &rp, 1, HashMap::new()).await;
                    }
                    RestoreFailPolicy::Error => {
                        let mut results = HashMap::new();
                        results.insert(
                            "restore_failed".into(),
                            OwnedValue::try_from(Value::Bool(true))
                                .expect("bool-to-OwnedValue is infallible"),
                        );
                        fire_response(&conn, &rp, 2, results).await;
                    }
                },
            }
        });

        Ok(request_path)
    }

    async fn start(
        &self,
        _session_handle: OwnedObjectPath,
        _parent_window: String,
        options: HashMap<String, OwnedValue>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let id = self.next_id();
        let request_path =
            OwnedObjectPath::try_from(format!("/org/freedesktop/portal/desktop/request/{id}"))
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        let conn = self.conn.clone();
        let scenario = self.scenario.clone();
        let rp = request_path.clone();

        conn.object_server()
            .at(&request_path, RequestObject)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        tokio::spawn(async move {
            let start_result = scenario.on_start(&options).await;
            fire_response(&conn, &rp, start_result.response, start_result.results).await;
        });

        Ok(request_path)
    }

    fn open_pipe_wire_remote(
        &self,
        _session_handle: OwnedObjectPath,
        _options: HashMap<String, OwnedValue>,
    ) -> zbus::fdo::Result<zbus::zvariant::OwnedFd> {
        // Return a dummy fd — the read end of a pipe.
        let (read_fd, _write_fd) = std::os::unix::net::UnixStream::pair()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(zbus::zvariant::OwnedFd::from(std::os::fd::OwnedFd::from(
            read_fd,
        )))
    }

    #[zbus(property)]
    fn version(&self) -> u32 {
        self.scenario.version()
    }
}

/// Fire the `Response` signal on a request object path.
async fn fire_response(
    conn: &zbus::Connection,
    request_path: &OwnedObjectPath,
    response: u32,
    results: HashMap<String, OwnedValue>,
) {
    // Small yield to ensure the method return is sent first.
    tokio::task::yield_now().await;

    let iface_ref = conn
        .object_server()
        .interface::<_, RequestObject>(request_path.as_ref())
        .await;

    if let Ok(iface_ref) = iface_ref {
        let emitter = iface_ref.signal_emitter();
        let _ = RequestObject::response(emitter, response, results).await;
    }
}

/// A handle to the running mock portal server.
pub struct MockPortal;

impl MockPortal {
    /// Start the mock portal on the given bus with the provided scenario.
    ///
    /// # Errors
    /// Returns `PortalError` if the D-Bus connection or object registration fails.
    ///
    /// `S` is resolved at the call site (static dispatch); internally we
    /// erase to `Arc<dyn Scenario>` because the portal stores it as shared
    /// state across method calls.
    pub async fn start<S: Scenario>(
        bus: &crate::bus::PrivateBus,
        scenario: S,
    ) -> Result<crate::handle::MockPortalHandle, PortalError> {
        let conn = bus.connect().await.map_err(|e| match e {
            crate::bus::BusError::Connect(e) | crate::bus::BusError::Name(e) => {
                PortalError::Zbus(e)
            }
            other => PortalError::Zbus(zbus::Error::Failure(other.to_string())),
        })?;

        let calls: Arc<Mutex<Vec<Call>>> = Arc::new(Mutex::new(Vec::new()));
        let counter = Arc::new(AtomicU64::new(0));
        let scenario: Arc<dyn Scenario> = Arc::new(scenario);

        let portal = ScreenCastPortal::new(
            scenario.clone(),
            calls.clone(),
            conn.clone(),
            counter.clone(),
        );

        conn.object_server()
            .at("/org/freedesktop/portal/desktop", portal)
            .await?;

        let task = tokio::spawn(async move {
            // Keep the connection alive until the task is dropped.
            std::future::pending::<()>().await;
        });

        Ok(crate::handle::MockPortalHandle::new(calls, task))
    }
}
