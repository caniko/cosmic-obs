//! Mock implementation of the XDG Desktop Portal `org.freedesktop.portal.ScreenCast` interface.
//!
//! This crate spins up an isolated `dbus-daemon`, registers a mock ScreenCast portal on it,
//! and lets you drive the portal through scenarios — recording every `SelectSources` call
//! for later assertion.
//!
//! Designed for testing portal clients against the ScreenCast RFC v6 extensions:
//! `source_label` and `restore_fail_policy`.
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
    Call, RestoreFailPolicy, RestoreFailReason, RestoreFailure, SourceDef, SourceTypes,
};
pub use scenario::{
    MultiSource, NormalSession, OldPortal, Options, RestoreTokenFails, RestoreTokenValid, Scenario,
    SlowResponse, StartResult, UserCancels,
};
