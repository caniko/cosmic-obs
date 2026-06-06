//! Mock implementation of the XDG Desktop Portal `org.freedesktop.portal.ScreenCast` interface.
//!
//! This crate spins up an isolated `dbus-daemon`, registers a mock ScreenCast portal on it,
//! and lets you drive the portal through scenarios — recording every `SelectSources` call
//! for later assertion.
//!
//! Designed for testing portal clients against the ScreenCast RFC v6 extensions:
//! `source_label`, `restore_policy`, and `restore_match_rules`.
//!
//! # Usage
//!
//! ```rust,no_run
//! use screencast_portal_mock::{MockPortal, NormalSession, PrivateBus};
//!
//! # async fn example() {
//! let bus = PrivateBus::spawn().await.unwrap();
//! let handle = MockPortal::start(&bus, NormalSession).await.unwrap();
//! // Connect a zbus client to bus.address() and make portal calls,
//! // then assert on handle.calls().
//! # }
//! ```

pub mod bus;
pub mod handle;
pub mod obs;
pub mod portal;
pub mod records;
pub mod scenario;

pub use bus::PrivateBus;
pub use handle::MockPortalHandle;
pub use portal::MockPortal;
pub use records::{
    ACCEPTED_SELECT_SOURCES_KEYS, Call, RestoreAction, RestoreFailReason, RestoreFailure,
    RestoreMatchRule, RestoreMatchScope, RestorePolicy, RestoreTokenRecord, SourceDef, SourceTypes,
    parse_restore_match_rules, parse_restore_policy, restore_failure_value,
    restore_match_rules_value,
};
pub use scenario::{
    DelayedRestore, ErrorWithoutFlag, ExternallyTriggeredRescue, FailThenSucceed, MultiSource,
    NormalSession, OldPortal, Options, RescueController, RescueOutcome, RestoreTokenFails,
    RestoreTokenValid, Scenario, SlowResponse, StartResult, UserCancels,
};
