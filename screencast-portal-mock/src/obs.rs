use std::process::Stdio;

use tokio::process::{Child, Command};

use crate::bus::PrivateBus;

/// Stub for launching OBS against a private bus.
///
/// This is a placeholder: OBS is not available in the test environment.
/// Real integration tests would use this to drive OBS end-to-end.
pub struct ObsProcess {
    child: Child,
}

impl ObsProcess {
    /// Launch OBS with `DBUS_SESSION_BUS_ADDRESS` overridden.
    ///
    /// # Errors
    /// Returns an error if the OBS process cannot be spawned.
    #[must_use = "the ObsProcess must be held alive to keep OBS running"]
    pub fn launch(bus: &PrivateBus) -> Result<Self, std::io::Error> {
        let child = Command::new("obs")
            .env("DBUS_SESSION_BUS_ADDRESS", bus.address())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        Ok(Self { child })
    }

    /// Send SIGTERM to the OBS process.
    pub fn kill(&mut self) {
        let _ = self.child.start_kill();
    }
}

impl Drop for ObsProcess {
    fn drop(&mut self) {
        self.kill();
    }
}
