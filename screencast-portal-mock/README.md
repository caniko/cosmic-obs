# screencast-portal-mock

<!-- simit:badges:start -->

[![Nix](https://img.shields.io/badge/Nix-managed-5277c3)](flake.nix) [![crates.io](https://img.shields.io/badge/crates.io-ready-f46623)](https://crates.io/crates/screencast-portal-mock)

<!-- simit:badges:end -->

A mock implementation of the [XDG Desktop Portal](https://flatpak.github.io/xdg-desktop-portal/)
`org.freedesktop.portal.ScreenCast` interface, running on a private D-Bus session bus.

Built for testing portal clients — especially the new options proposed in the
ScreenCast portal **RFC v6**:

| Key | Type | Description |
|-----|------|-------------|
| `source_label` | `string` | Human-readable hint identifying the application-level source |
| `restore_policy` | `a{sv}` | Per-reason restore policy with `default_action` and optional `actions` overrides |
| `restore_match_rules` | `aa{sv}` | Optional title-regex restore aliases recorded on `SelectSources` calls |

`restore_policy.default_action` and all reason-specific `actions` use `0` =
prompt, `1` = skip, and `2` = error. Policy-triggered skip/error responses
include a `restore_failure` result object with `reason`, `action`, and
`token_invalid`, emit `org.freedesktop.portal.Session::Closed`, and reject
later `Start` calls for that session.

## Quick start

Add to your `Cargo.toml`:

```toml
[dev-dependencies]
screencast-portal-mock = "0.1"
tokio = { version = "1", features = ["full", "test-util"] }
```

Write a test:

```rust
use screencast_portal_mock::{MockPortal, NormalSession, PrivateBus};

#[tokio::test]
async fn smoke_test() {
    // Spawn an isolated dbus-daemon
    let bus = PrivateBus::spawn().await.unwrap();

    // Start the mock portal with a scenario
    let handle = MockPortal::start(&bus, NormalSession).await.unwrap();

    // Connect your client to bus.address() and make portal calls...
    // Then assert on what the mock recorded:
    handle.assert_call_count(0); // no SelectSources calls yet
}
```

## Scenarios

Scenarios control how the mock portal responds. Implement the `Scenario` trait
for custom behaviour, or use one of the built-in scenarios:

| Scenario | Behaviour |
|----------|-----------|
| `NormalSession` | Everything succeeds with defaults (one 1920×1080 stream) |
| `UserCancels` | `Start` returns `response=1` (user cancelled) |
| `RestoreTokenValid` | Restore succeeds; `Start` returns a restore token |
| `RestoreTokenFails` | `SelectSources` returns a restore failure |
| `MultiSource` | `Start` returns multiple streams |
| `SlowResponse` | Wraps another scenario, adding a configurable delay |
| `OldPortal` | Reports a lower interface version (for backward-compat testing) |

Example built-in scenario values:

```rust
use std::time::Duration;

use screencast_portal_mock::{
    MultiSource, NormalSession, OldPortal, RestoreFailReason, RestoreTokenFails,
    RestoreTokenValid, SlowResponse, SourceDef,
};

let valid_restore = RestoreTokenValid {
    token: "restored-token".to_string(),
    node_id: 42,
};

let restore_failure = RestoreTokenFails {
    reason: RestoreFailReason::TokenNotFound,
};

let multi_source = MultiSource {
    sources: vec![
        SourceDef::monitor(1).with_label("Camera"),
        SourceDef::monitor(2).with_label("Desktop"),
    ],
};

let old_portal = OldPortal { version: 5 };

let slow_response = SlowResponse {
    delay: Duration::from_millis(50),
    inner: Box::new(NormalSession),
};
```

### Custom scenario

```rust
use async_trait::async_trait;
use screencast_portal_mock::scenario::{ExtraResults, Options, Scenario, StartResult};

pub struct MyScenario;

#[async_trait]
impl Scenario for MyScenario {
    async fn on_start(&self, _options: &Options) -> StartResult {
        // Return a custom response
        StartResult {
            response: 0,
            results: Default::default(),
        }
    }
}
```

## Assertions

`MockPortalHandle` records every `SelectSources` call and provides assertion
helpers:

- `assert_call_count(n)` — exactly `n` calls were recorded
- `assert_source_label(index, label)` — call at `index` has the given label
- `assert_no_source_label(index)` — call at `index` has no label
- `assert_restore_default_action(index, action)` — call at `index` has the given default action
- `assert_restore_reason_action(index, reason, action)` — call at `index` has the given reason-specific action
- `assert_no_restore_policy(index)` — call at `index` has no restore policy
- `assert_no_prompts()` — no call used `restore_policy.default_action = Prompt`
- `wait_for_calls(n, timeout)` — async wait until `n` calls are recorded
- `calls()` — snapshot of all recorded calls, including parsed `restore_match_rules`

## Requirements

- **Linux** with D-Bus (a `dbus-daemon` binary must be available)
- Rust 1.85+ (edition 2024)

## License

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.
