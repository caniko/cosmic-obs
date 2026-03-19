use std::process::Stdio;

use tokio::io::AsyncBufReadExt;
use tokio::process::{Child, Command};

/// Errors from launching or connecting to the private D-Bus.
#[derive(Debug, thiserror::Error)]
pub enum BusError {
    #[error("failed to spawn dbus-daemon: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("dbus-daemon closed stdout before printing address")]
    NoAddress,
    #[error("failed to read dbus-daemon stdout: {0}")]
    Read(#[source] std::io::Error),
    #[error("zbus connection failed: {0}")]
    Connect(#[source] zbus::Error),
    #[error("failed to request bus name: {0}")]
    Name(#[source] zbus::Error),
}

/// A private session bus backed by a child `dbus-daemon` process.
pub struct PrivateBus {
    address: String,
    child: Child,
}

impl PrivateBus {
    /// Spawn a new private session bus and return once the address is known.
    pub async fn spawn() -> Result<Self, BusError> {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--print-address=1", "--nofork"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(BusError::Spawn)?;

        let stdout = child.stdout.take().ok_or(BusError::NoAddress)?;
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut address = String::new();
        reader
            .read_line(&mut address)
            .await
            .map_err(BusError::Read)?;

        let address = address.trim().to_owned();
        if address.is_empty() {
            return Err(BusError::NoAddress);
        }

        Ok(Self { address, child })
    }

    /// The bus address string (e.g. `unix:abstract=...`).
    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Connect to this bus and request the well-known portal name.
    pub async fn connect(&self) -> Result<zbus::Connection, BusError> {
        let conn = zbus::connection::Builder::address(self.address.as_str())
            .map_err(BusError::Connect)?
            .build()
            .await
            .map_err(BusError::Connect)?;

        conn.request_name("org.freedesktop.portal.Desktop")
            .await
            .map_err(BusError::Name)?;

        Ok(conn)
    }

    /// Kill the daemon process.
    pub fn kill(&mut self) {
        let _ = self.child.start_kill();
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        self.kill();
    }
}
