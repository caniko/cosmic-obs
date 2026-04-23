use std::collections::HashMap;
use std::time::Instant;

use bitflags::bitflags;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

/// Reason a restore token could not be honoured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RestoreFailReason {
    TokenNotFound,
    SourceUnavailable,
    PermissionRevoked,
    TokenConsumed,
}

impl RestoreFailReason {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TokenNotFound => "token_not_found",
            Self::SourceUnavailable => "source_unavailable",
            Self::PermissionRevoked => "permission_revoked",
            Self::TokenConsumed => "token_consumed",
        }
    }

    #[must_use]
    pub fn token_invalid(self) -> bool {
        !matches!(self, Self::SourceUnavailable)
    }
}

impl TryFrom<&str> for RestoreFailReason {
    type Error = InvalidRestoreFailReason;

    fn try_from(v: &str) -> Result<Self, Self::Error> {
        match v {
            "token_not_found" => Ok(Self::TokenNotFound),
            "source_unavailable" => Ok(Self::SourceUnavailable),
            "permission_revoked" => Ok(Self::PermissionRevoked),
            "token_consumed" => Ok(Self::TokenConsumed),
            other => Err(InvalidRestoreFailReason(other.to_string())),
        }
    }
}

/// Error returned when converting an unknown reason string.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid restore failure reason: {0}")]
pub struct InvalidRestoreFailReason(pub String);

/// Failure returned by `Scenario::on_select_sources` when restore cannot proceed.
#[derive(Debug, Clone)]
pub struct RestoreFailure {
    pub reason: RestoreFailReason,
}

/// Action to take when a restore token cannot be honoured (RFC v6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum RestoreAction {
    /// Show the picker dialog (default, backward-compatible).
    Prompt = 0,
    /// Fire `Response(1, {})` — skip without prompting.
    Skip = 1,
    /// Fire `Response(2, {restore_failure: ...})` — hard error.
    Error = 2,
}

/// Error returned when converting an invalid `u32` to [`RestoreAction`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid restore action value: {0}")]
pub struct InvalidRestoreAction(pub u32);

impl TryFrom<u32> for RestoreAction {
    type Error = InvalidRestoreAction;

    fn try_from(v: u32) -> Result<Self, <Self as TryFrom<u32>>::Error> {
        match v {
            0 => Ok(Self::Prompt),
            1 => Ok(Self::Skip),
            2 => Ok(Self::Error),
            other => Err(InvalidRestoreAction(other)),
        }
    }
}

/// Per-reason restore failure policy supplied as `restore_policy: a{sv}`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RestorePolicy {
    pub default_action: Option<RestoreAction>,
    pub actions: HashMap<RestoreFailReason, RestoreAction>,
}

impl RestorePolicy {
    #[must_use]
    pub fn action_for(&self, reason: RestoreFailReason) -> RestoreAction {
        self.actions
            .get(&reason)
            .copied()
            .or(self.default_action)
            .unwrap_or(RestoreAction::Prompt)
    }
}

/// Error returned when the `restore_policy` vardict shape is invalid.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidRestorePolicy {
    #[error("restore_policy must be an a{{sv}} vardict")]
    NotVardict,
    #[error("restore_policy.default_action must be u32")]
    InvalidDefaultAction,
    #[error("restore_policy.actions must be a{{su}}")]
    InvalidActions,
}

/// Parse the standardized `restore_policy: a{sv}` object.
///
/// Unknown action values are treated as absent so they fall through to
/// `default_action`, then to the prompt default. Unknown reason strings are
/// ignored for forward compatibility.
pub fn parse_restore_policy(value: &OwnedValue) -> Result<RestorePolicy, InvalidRestorePolicy> {
    let dict: HashMap<String, OwnedValue> = value
        .clone()
        .try_into()
        .map_err(|_| InvalidRestorePolicy::NotVardict)?;

    let default_action = match dict.get("default_action") {
        Some(value) => {
            let raw =
                u32::try_from(value).map_err(|_| InvalidRestorePolicy::InvalidDefaultAction)?;
            RestoreAction::try_from(raw).ok()
        }
        None => None,
    };

    let mut actions = HashMap::new();
    if let Some(value) = dict.get("actions") {
        let raw_actions: HashMap<String, u32> = value
            .clone()
            .try_into()
            .map_err(|_| InvalidRestorePolicy::InvalidActions)?;
        for (reason, action) in raw_actions {
            if let (Ok(reason), Ok(action)) = (
                RestoreFailReason::try_from(reason.as_str()),
                RestoreAction::try_from(action),
            ) {
                actions.insert(reason, action);
            }
        }
    }

    Ok(RestorePolicy {
        default_action,
        actions,
    })
}

#[must_use]
pub fn restore_failure_value(reason: RestoreFailReason, action: RestoreAction) -> OwnedValue {
    let mut failure: HashMap<String, OwnedValue> = HashMap::new();
    failure.insert(
        "reason".into(),
        OwnedValue::try_from(Value::new(reason.as_str().to_string()))
            .expect("string-to-OwnedValue is infallible"),
    );
    failure.insert(
        "action".into(),
        OwnedValue::try_from(Value::U32(action as u32)).expect("u32-to-OwnedValue is infallible"),
    );
    failure.insert(
        "token_invalid".into(),
        OwnedValue::try_from(Value::Bool(reason.token_invalid()))
            .expect("bool-to-OwnedValue is infallible"),
    );
    OwnedValue::from(failure)
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
    pub restore_policy: Option<RestorePolicy>,
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
