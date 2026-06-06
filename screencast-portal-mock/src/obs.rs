use std::collections::{HashMap, HashSet};
use std::process::Stdio;

use tokio::process::{Child, Command};
use zbus::zvariant::{OwnedValue, Value};

use crate::bus::PrivateBus;
use crate::records::{
    RestoreAction, RestoreMatchRule, RestoreMatchScope, restore_match_rules_value,
};

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

/// In-process model of the patched OBS PipeWire ScreenCast client.
#[derive(Debug, Clone)]
pub struct ObsClientModel {
    source_name: String,
    restore_token: Option<String>,
    restore_action: RestoreAction,
    restore_match_rules: Option<String>,
}

impl ObsClientModel {
    /// Create a model with OBS' patched defaults.
    #[must_use]
    pub fn new(source_name: impl Into<String>) -> Self {
        Self {
            source_name: source_name.into(),
            restore_token: None,
            restore_action: RestoreAction::Skip,
            restore_match_rules: None,
        }
    }

    /// Set the persisted restore token OBS would send.
    #[must_use]
    pub fn with_restore_token(mut self, restore_token: impl Into<String>) -> Self {
        self.restore_token = Some(restore_token.into());
        self
    }

    /// Override OBS' restore-failure action.
    #[must_use]
    pub fn with_restore_action(mut self, restore_action: RestoreAction) -> Self {
        self.restore_action = restore_action;
        self
    }

    /// Set OBS' multiline restore-alias text.
    #[must_use]
    pub fn with_restore_match_rules(mut self, restore_match_rules: impl Into<String>) -> Self {
        self.restore_match_rules = Some(restore_match_rules.into());
        self
    }

    /// Build the `SelectSources` options OBS would send to a portal version.
    #[must_use]
    pub fn select_sources_options(&self, portal_version: u32) -> HashMap<String, OwnedValue> {
        let mut options = HashMap::new();

        if let Some(token) = self
            .restore_token
            .as_deref()
            .filter(|token| !token.is_empty())
        {
            options.insert(
                "restore_token".into(),
                OwnedValue::try_from(Value::new(token.to_string()))
                    .expect("string-to-OwnedValue is infallible"),
            );
        }

        if portal_version < 6 {
            return options;
        }

        if !self.source_name.is_empty() {
            options.insert(
                "source_label".into(),
                OwnedValue::try_from(Value::new(self.source_name.clone()))
                    .expect("string-to-OwnedValue is infallible"),
            );
        }

        if self
            .restore_token
            .as_deref()
            .is_some_and(|token| !token.is_empty())
        {
            let mut policy: HashMap<String, OwnedValue> = HashMap::new();
            policy.insert(
                "default_action".into(),
                OwnedValue::try_from(Value::U32(self.restore_action as u32))
                    .expect("u32-to-OwnedValue is infallible"),
            );
            options.insert("restore_policy".into(), OwnedValue::from(policy));

            let rules = self.parsed_restore_match_rules();
            if !rules.is_empty() {
                options.insert(
                    "restore_match_rules".into(),
                    restore_match_rules_value(&rules),
                );
            }
        }

        options
    }

    /// Return the keys this model sends for a representative v6 restore call.
    #[must_use]
    pub fn representative_v6_keys() -> HashSet<&'static str> {
        Self::new("OBS Source")
            .with_restore_token("token")
            .with_restore_match_rules("same: project\nany_app: shared")
            .select_sources_options(6)
            .into_keys()
            .map(|key| match key.as_str() {
                "restore_token" => "restore_token",
                "source_label" => "source_label",
                "restore_policy" => "restore_policy",
                "restore_match_rules" => "restore_match_rules",
                other => panic!("unexpected dynamic OBS key {other}"),
            })
            .collect()
    }

    /// Parse OBS' multiline alias setting using the patched client grammar.
    #[must_use]
    pub fn parsed_restore_match_rules(&self) -> Vec<RestoreMatchRule> {
        self.restore_match_rules
            .as_deref()
            .map(parse_restore_match_rules_text)
            .unwrap_or_default()
    }
}

/// Parse OBS' multiline restore alias text into public wire rules.
#[must_use]
pub fn parse_restore_match_rules_text(text: &str) -> Vec<RestoreMatchRule> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }

            let (scope, pattern) = if let Some(pattern) = line.strip_prefix("any_app:") {
                (RestoreMatchScope::AnyApp, pattern.trim())
            } else if let Some(pattern) = line.strip_prefix("any:") {
                (RestoreMatchScope::AnyApp, pattern.trim())
            } else if let Some(pattern) = line.strip_prefix("same_app:") {
                (RestoreMatchScope::SameApp, pattern.trim())
            } else if let Some(pattern) = line.strip_prefix("same:") {
                (RestoreMatchScope::SameApp, pattern.trim())
            } else {
                (RestoreMatchScope::SameApp, line)
            };

            if pattern.is_empty() {
                None
            } else {
                Some(RestoreMatchRule::title_regex(pattern, scope))
            }
        })
        .collect()
}
