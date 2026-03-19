use std::collections::HashMap;
use std::time::Instant;

use bitflags::bitflags;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

/// Reason a restore token could not be honoured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreFailReason {
    TokenNotFound,
    SourceUnavailable,
    PermissionRevoked,
    TokenConsumed,
}

/// Failure returned by `Scenario::on_select_sources` when restore cannot proceed.
#[derive(Debug, Clone)]
pub struct RestoreFailure {
    pub reason: RestoreFailReason,
}

/// Controls behaviour when a restore token cannot be honoured (RFC v6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum RestoreFailPolicy {
    /// Show the picker dialog (default, backward-compatible).
    Prompt = 0,
    /// Fire `Response(1, {})` — skip without prompting.
    Skip = 1,
    /// Fire `Response(2, {restore_failed: true})` — hard error.
    Error = 2,
}

/// Error returned when converting an invalid `u32` to [`RestoreFailPolicy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid restore_fail_policy value: {0}")]
pub struct InvalidRestoreFailPolicy(pub u32);

impl TryFrom<u32> for RestoreFailPolicy {
    type Error = InvalidRestoreFailPolicy;

    fn try_from(v: u32) -> Result<Self, <Self as TryFrom<u32>>::Error> {
        match v {
            0 => Ok(Self::Prompt),
            1 => Ok(Self::Skip),
            2 => Ok(Self::Error),
            other => Err(InvalidRestoreFailPolicy(other)),
        }
    }
}

bitflags! {
    /// Source type flags for `SelectSources`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SourceTypes: u32 {
        const MONITOR = 1;
        const WINDOW  = 2;
        const VIRTUAL = 4;
    }
}

/// A recorded `SelectSources` call for later assertion.
#[derive(Debug, Clone)]
pub struct Call {
    pub timestamp: Instant,
    pub session_handle: OwnedObjectPath,
    pub source_types: SourceTypes,
    pub multiple: bool,
    pub cursor_mode: u32,
    pub persist_mode: u32,
    pub restore_token: Option<String>,
    pub source_label: Option<String>,
    pub restore_fail_policy: Option<RestoreFailPolicy>,
    pub raw_options: HashMap<String, OwnedValue>,
}

/// Definition of a single source stream returned by the mock.
#[derive(Debug, Clone)]
pub struct SourceDef {
    pub node_id: u32,
    pub expected_label: Option<String>,
    pub restore_valid: bool,
    pub size: (u32, u32),
    pub position: (i32, i32),
}

impl SourceDef {
    /// Create a monitor source with the given PipeWire node id.
    #[must_use]
    pub fn monitor(node_id: u32) -> Self {
        Self {
            node_id,
            expected_label: None,
            restore_valid: true,
            size: (1920, 1080),
            position: (0, 0),
        }
    }

    /// Attach a human-readable label to this source definition.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.expected_label = Some(label.into());
        self
    }

    /// Mark this source's restore token as invalid.
    #[must_use]
    pub fn invalid(mut self) -> Self {
        self.restore_valid = false;
        self
    }
}
