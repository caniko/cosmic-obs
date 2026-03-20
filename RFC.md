# RFC: Add `source_label` and `restore_fail_mode` to ScreenCast.SelectSources

**Author:** Can H. Tartanoglu
**Date:** 2026-03-19
**Status:** Draft
**Interface Version:** 5 → 6
**Affects:** xdg-desktop-portal, libportal, portal backends, portal consumers

---

## Problem Statement

Applications that create multiple concurrent ScreenCast sessions — OBS Studio
being the primary example — expose their users to two pain points that the
current portal interface cannot address:

### 1. Indistinguishable source pickers

When OBS creates three PipeWire sources (game capture, webcam, browser window),
the portal shows three identical "Share your screen" dialogs. The user has no
way to know which OBS source each dialog corresponds to. They must guess, and
guessing wrong means reconfiguring sources after the fact.

### 2. Stale restore tokens interrupt unattended workflows

OBS persists restore tokens so that scene collections can be reopened without
re-prompting. When a token goes stale (monitor unplugged, window closed, system
rebooted), the portal silently falls back to showing the source picker. In
headless recording setups, kiosk displays, and CI pipelines, no one is present
to interact with the picker — the application hangs indefinitely.

Both problems are architectural: the portal interface provides no mechanism for
the caller to label its sources or to express a preference for what should
happen when restore fails.

---

## Design

Two new optional keys in the `SelectSources()` options dictionary, and one new
key in the `SelectSources()` response results dictionary. All gated behind
interface version 6.

### New SelectSources options

#### `source_label` (`s`)

A human-readable string provided by the application to identify the purpose or
origin of this `SelectSources` call. Portal backends that support this key
should surface the label in the source picker dialog. Backends that do not
support it **must** ignore the key.

- Type: string (`s`)
- Optional. Omitting it preserves current behaviour.
- Backends SHOULD truncate labels exceeding 256 bytes for display.
- This string is plain text. Backends MUST NOT interpret it as markup.
- Same trust model as window titles — the string comes from the calling
  application and may contain arbitrary text.
- Backends SHOULD treat an empty string the same as an absent key.

#### `restore_fail_mode` (`u`)

Determines what the portal does when a `restore_token` is provided but the
session cannot be restored. Allowed values:

| Value | Name   | Behaviour |
|-------|--------|-----------|
| 0     | prompt | Show the source picker as if no token was provided. **Default.** Preserves current behaviour. |
| 1     | skip   | Do not show the picker. Return `response=1` via `Request::Response`. |
| 2     | error  | Do not show the picker. Return `response=2` with `restore_failed=true` in the results vardict. |

- Type: uint32 (`u`)
- Optional. When absent, defaults to 0 (prompt).
- Has no effect when no `restore_token` is provided.
- Has no effect on RemoteDesktop sessions, where `restore_token` is not
  supported.
- Values > 2 are rejected with `InvalidArgument`.

### New SelectSources response key

#### `restore_failed` (`b`)

Present and set to `true` when `response=2` and the failure was caused by an
inability to honour a provided `restore_token` combined with
`restore_fail_mode=2`. This allows callers to distinguish a restore failure
from other error conditions.

- Type: boolean (`b`)
- Only present when the conditions above are met.
- Applications receiving this result SHOULD discard the stale `restore_token`
  and either re-initiate `SelectSources` without a token (to trigger the
  picker) or defer to a user action.

### Version bump

The `org.freedesktop.portal.ScreenCast` interface version is bumped from **5**
to **6**. Callers must check the `Version` property and only send the new keys
when `Version >= 6`.

---

## Backward Compatibility

The design is fully backward-compatible in all four directions:

| Caller | Portal | Behaviour |
|--------|--------|-----------|
| Old (no v6 keys) | Old (v5) | Unchanged. |
| Old (no v6 keys) | New (v6) | Unchanged — new keys are optional with safe defaults. |
| New (sends v6 keys) | Old (v5) | Old portal validates `SelectSources` options and drops unknown keys. Session proceeds normally. Callers should check `Version >= 6` before sending, but even if they don't, the session does not break. |
| New (sends v6 keys) | New (v6) | New features active. |

Key properties that enable this:

1. Both new keys are **optional** entries in the existing options vardict.
2. No new D-Bus methods or signals are introduced.
3. The default value for `restore_fail_mode` (0 = prompt) preserves the
   exact current behaviour.
4. The existing `XdpOptionKey` validation in `screen-cast.c` drops entries
   whose keys are not registered — old portals silently ignore unknown keys.

---

## Alternatives Considered

### Use a separate D-Bus method instead of option dict keys

The portal pattern for additive features is to extend the options dict, not
add new methods. `persist_mode` and `restore_token` (added in v4) followed
this pattern. A new method would require new object paths and complicate the
session lifecycle.

### Put `source_label` on `CreateSession` instead of `SelectSources`

`CreateSession` creates the D-Bus session object; `SelectSources` is where
the source type and restore token are specified. Since `source_label`
describes the source being requested (not the session), it belongs alongside
the other source-selection parameters in `SelectSources`.

### Use a string instead of uint32 for `restore_fail_mode`

The `persist_mode` precedent uses uint32 for its enum. Enumerated integers
are simpler to validate (range check) and extend (add new values in future
versions) than strings. They also avoid locale/encoding concerns.

### Always prompt on restore failure (status quo)

This is the current behaviour and the default for `restore_fail_mode=0`.
However, headless and kiosk deployments cannot show a picker — displaying one
causes an indefinite hang. The opt-in policy mechanism lets applications
explicitly choose non-prompt behaviour only when they need it.

### Use `source_name` instead of `source_label`

The portal spec uses `name` for identity semantics (e.g., output names that are
stable identifiers). `label` is a display hint — following the `accept_label`
precedent in the FileChooser portal. The label is not an identifier and backends
are free to truncate or ignore it.

### Use a boolean `restore_required` instead of a three-value mode

A boolean would only express "prompt or don't prompt" — losing the `skip` mode
(return `response=1`), which is the primary use case for OBS. Three values match
the `persist_mode` pattern (also uint32, also three values) and cover all
meaningful caller intents without overloading response codes.

### Add a `RestoreFailed` signal instead of a response code

Adding a new signal to the portal would require changes to the session lifecycle
model across all backends and all callers. The current approach — returning a
response code from the existing `Request::Response` signal — is consistent with
the portal architecture and requires no new signal subscriptions.

### Use structured/nested options (e.g., `restore_options: {token, fail_mode}`)

The portal spec uses flat vardict keys for all options added in versions 2–5
(`cursor_mode`, `persist_mode`, `restore_token`, etc.). Nesting would break this
pattern and require recursive validation infrastructure that does not exist in
the `XdpOptionKey` framework.

### Add a "retry with new token" policy (value 3)

This would ask the portal to automatically retry restoration with a different
token. The caller can already achieve this by catching `response=2` with
`restore_failed=true` and re-calling `SelectSources` with a new token. Adding
a retry policy to the portal pushes application logic into the wrong layer.
Deferred to future work if demand emerges.

---

## Backend Implementation Notes

### Minimum requirement

A backend that does **not** want to implement these features needs to do
**nothing**. The base `xdg-desktop-portal` daemon handles option validation,
restore token resolution, and policy enforcement in `screen-cast.c` before
delegating to the backend. Unrecognised option keys are dropped by the
validation layer.

### Displaying `source_label`

Backends that want to display the label in their source picker need to:

1. Read `source_label` from the options vardict passed to the
   `impl_select_sources` callback.
2. Display it in whatever picker UI they provide.

This is purely a presentation concern. The string is a hint — backends may
truncate, sanitise, or ignore it as they see fit.

### `restore_fail_mode` enforcement

The base portal daemon (`screen-cast.c`) enforces the policy **before**
delegating to the backend. When a restore token fails and the policy is
`skip` or `error`, the daemon returns the appropriate response directly
without calling into the backend at all. Backends do not need to handle
these cases.

### Example: xdg-desktop-portal-hyprland

XDPH's implementation is 10 lines of C++: read `source_label` from the
options, set `XDPH_SOURCE_LABEL` environment variable for the picker
subprocess. The picker can display it however it likes.

---

## Migration Guide for Application Developers

### Step 1: Check the portal version

```c
// Using libportal
guint version = xdp_portal_get_screencast_version(portal);
if (version >= 6) {
    // Safe to use source_label and restore_fail_mode
}
```

```c
// Using GDBus directly
GVariant *version = g_dbus_proxy_get_cached_property(proxy, "Version");
guint32 v = g_variant_get_uint32(version);
```

### Step 2: Send the new keys in SelectSources

```c
// Using libportal (new _full function)
xdp_portal_create_screencast_session_full(
    portal,
    XDP_OUTPUT_MONITOR,
    XDP_SCREENCAST_FLAG_NONE,
    XDP_CURSOR_MODE_HIDDEN,
    XDP_PERSIST_MODE_PERSISTENT,
    restore_token,
    "OBS Game Capture",              // source_label
    XDP_RESTORE_FAIL_MODE_SKIP,    // restore_fail_mode
    cancellable, callback, data);
```

```c
// Using GDBus directly
g_variant_builder_add(&options, "{sv}", "source_label",
                      g_variant_new_string("OBS Game Capture"));
g_variant_builder_add(&options, "{sv}", "restore_fail_mode",
                      g_variant_new_uint32(1));
```

### Step 3: Handle the new response codes

```c
// In your Response signal handler:
switch (response) {
    case 0:
        // Success — sources selected (either via picker or restored)
        break;
    case 1:
        // Cancelled. If you sent restore_fail_mode=1 (skip) and had a
        // restore_token, this means the token was stale and the app chose
        // to skip rather than show the picker.
        break;
    case 2: {
        // Error. Check for restore_failed to distinguish from other errors.
        gboolean restore_failed = FALSE;
        g_variant_lookup(results, "restore_failed", "b", &restore_failed);
        if (restore_failed) {
            // Restore token could not be honoured and app requested error.
            // You may retry with a different token or inform the user.
        }
        break;
    }
}
```

---

## Test Evidence

A comprehensive mock test harness (`screencast-portal-mock`, Rust) validates
the protocol behaviour:

- **29 integration tests** covering all scenarios, version negotiation,
  backward compatibility, all three policy values, combined label+policy
  flows, all four `RestoreFailReason` variants, and session lifecycle.
- **4 property-based tests** (proptest) validating `RestoreFailMode` enum
  round-trip and `SourceTypes` bitflag invariants.
- **Backward compatibility tests** proving:
  - Old client (no v6 keys) succeeds against v6 portal
  - New client (with v6 keys) succeeds against v5 portal
  - Version-checking client correctly gates v6 key usage

The test harness spawns an isolated `dbus-daemon` per test, registers a mock
`org.freedesktop.portal.ScreenCast` implementation, and records all
`SelectSources` calls for assertion. No display server or compositor required.

A Nix flake packages the complete patched stack (xdg-desktop-portal,
libportal, XDPH, cosmic-portal, OBS Studio) for reproducible end-to-end
verification.

---

## Scope of Changes

| Project | Patch size | Summary |
|---------|-----------|---------|
| xdg-desktop-portal | +101 lines | XML spec docs, option validation, policy enforcement in `screen-cast.c` |
| libportal | +95 lines | New `XdpRestoreFailMode` enum, `xdp_portal_create_screencast_session_full()` |
| xdg-desktop-portal-hyprland | +20 lines | Pass `source_label` to picker via env var |
| OBS Studio | +25 lines | Send `source_label` (OBS source name) and `restore_fail_mode=skip` |
| xdg-desktop-portal-cosmic | +371 lines | Full rescue mechanism with tiered matching (COSMIC value-add) |

All patches are guarded by version checks and preserve existing behaviour
when the new keys are absent.

---

## Security Considerations

### `source_label` trust model

`source_label` is untrusted application-provided input, with the same trust
model as window titles already displayed in portal picker dialogs. A malicious
application can already set arbitrary window titles that appear in the picker.

Backends MUST:
- Treat `source_label` as **plain text** — never interpret as Pango markup,
  HTML, or any rich text format.
- Strip Unicode control characters (U+0000–U+001F, U+007F, U+200E–U+200F,
  U+202A–U+202E, U+2066–U+2069) before display to prevent RTL override
  attacks and invisible characters.

Backends SHOULD:
- Truncate labels exceeding 256 bytes for display.
- Display the label as visually distinct from portal-generated UI text.

### `restore_fail_mode` and consent

The portal's fundamental purpose is mediating user consent for screen access.
The `skip` and `error` modes do **not** bypass this consent:

- They only activate when a `restore_token` was provided, meaning the user
  **already granted consent** in a prior session.
- The token represents stored consent for a **specific source**. If that
  source is no longer available, the stored consent cannot be fulfilled.
- No new source access is granted without explicit user interaction.
- The default (`prompt`) preserves the existing behaviour exactly.

## Future Work

These are explicitly **not** part of this proposal but may be considered in
future versions:

- **i18n markers for `source_label`**: The label is application-provided and
  already in the user's locale. Portal-side i18n is unnecessary.
- **`SourceLabelChanged` signal**: For long-running sessions where the source
  name changes. Adds a new signal to the Session interface — significant
  complexity for a niche use case.
- **Retry-with-new-token policy**: Callers can already retry by catching
  `response=2` and re-calling `SelectSources`. Pushing retry logic into the
  portal conflates layers.
