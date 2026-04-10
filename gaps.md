# ScreenCast Portal RFC — Gap Analysis

Focus: Cosmic DE. All patches included for completeness.

---

## Phase 1 — Cosmic backend correctness

Self-contained fixes within `xdg-desktop-portal-cosmic`. All items parallel.

### 1A. Version mismatch: Cosmic reports 5, spec says 6

**File:** `src/screencast.rs:599` — `fn version(&self) -> u32 { 5 }`

Cosmic bumps to 5. xdg-desktop-portal bumps to 6. OBS checks
`get_screencast_version() >= 6`. Result: OBS never sends `source_label` or
`restore_fail_mode` to Cosmic. The entire feature is dead.

**Fix:** Bump to 6.

### 1B. Parallel vec structural integrity

**Files:** `src/screencast.rs`, `src/screencast_dialog.rs`

`toplevels`, `toplevel_app_ids`, `toplevel_titles`, and `rescue_modes` are
parallel vecs with no structural coupling:

- V1→V2 migration fills missing vecs with `Vec::new()`: 3 toplevels, 0
  app_ids, 0 titles, 0 rescue_modes. `.get(i).unwrap_or(default)` papers
  over this.
- `SetRescueMode` indexes `rescue_modes[idx]` directly
  (`screencast_dialog.rs:553`). If `rescue_modes` is shorter than
  `toplevels` (V1 data), this panics.
- `from_capture_sources()` takes a separate `&[u32]` slice with no length
  check against toplevels count.

**Fix:** Replace parallel vecs with `Vec<ToplevelEntry>` struct, or enforce
equal lengths at every construction site (pad on V1 deserialize, assert on
`from_capture_sources`).

### 1C. Partial restoration on multi-window failure

**File:** `src/screencast.rs`, `to_capture_sources()` (~line 82–213)

Tiered fallback pushes matched toplevels one at a time. If window 2 of 3
fails, function returns `None` — but window 1 was already pushed. Partial
vec silently dropped. No diagnostic says which window failed.

**Fix:** Match into a temp vec (all-or-nothing), or return a `Result` with
per-window error info.

### 1D. Race between `to_capture_sources()` calls

**File:** `src/screencast.rs`, start handler (~line 348–397)

Called twice in succession:
1. Before `register_rescue()` (line ~351) — returns `None`
2. After `register_rescue()` (line ~369) — may return `Some` now

Window appearing between calls: rescue registered but unnecessary (orphaned
in `pending_rescues`). Window disappearing: both return `None`, rescue waits
for vanished window.

**Fix:** Single lock scope: check, and if fail, register rescue atomically.

### 1E. Unbounded pending rescues

**File:** `src/wayland/mod.rs`, `pending_rescues: Mutex<Vec<PendingRescue>>`

No cap. Misbehaving caller can register thousands of rescue hooks via
repeated stale-token sessions.

**Fix:** Hard cap (e.g. 64). Reject beyond limit, return
`PortalResponse::Other`, log warning.

### 1F. `source_label` not sanitized

**File:** `src/screencast.rs:309`

`session_data.source_label = options.source_label` stores verbatim:

- `Some("")` not normalized to `None` (spec: treat empty as absent).
- No Unicode control character stripping (spec: SHOULD strip
  U+0000–U+001F, U+007F, U+200E–U+200F, U+202A–U+202E, U+2066–U+2069).
- No length cap (spec: SHOULD truncate at 256 bytes).

**Fix:** Strip control chars, truncate to 256 bytes (char boundary),
normalize empty/whitespace-only to `None`.

### 1G. `OtherWithResults` serialization has no fallback

**File:** `src/main.rs:73–75`

```rust
Self::OtherWithResults(res) => {
    (PORTAL_RESPONSE_OTHER, res).serialize(serializer),
}
```

If serialization fails, the D-Bus message is incomplete. No error handling.

**Fix:** Add `.map_err()` or ensure `StartResult` always serializes (it
does today, but no compile-time guarantee).

---

## Phase 2 — Rescue lifecycle

Depends on 1B (structural integrity) and 1D (race fix). Items within phase
are parallel.

### 2A. No rescue timeout

**File:** `src/screencast.rs`, rescue await (~line 374)

`rx.await` waits indefinitely. If application never launches, session hangs
forever. Only exit: session close (receiver drop).

**Fix:** `tokio::time::timeout(Duration::from_secs(30), rx).await`. On
expiry, apply policy (skip → `Cancelled`, error → `OtherWithResults`).

### 2B. Rescue hook cleanup is lazy

**File:** `src/wayland/mod.rs:162`

Dead rescues purged only when a toplevel event fires. Idle desktop → dead
rescues accumulate.

**Fix:** Also purge on `register_rescue` (drain closed entries before push).
Or document lazy model.

### 2C. No rescue progress feedback

While rescue is pending, caller gets no signal. For skip mode: fine. For
error mode: caller can't distinguish "portal hung" from "rescue in progress".

**Fix:** Optional: portal log or signal on rescue state transitions. Low
priority.

---

## Phase 3 — OBS caller robustness

Independent of Phase 2. All items parallel.

### 3A. Transform preservation on skip is accidental

**File:** `patches/obs-studio/screencast-portal.c:325–339`

Skip path bare-returns. Scene item + transform survive only because nothing
destroys them. Scene reload, plugin restart, profile switch → transform lost.

**Fix:** Snapshot `obs_sceneitem_get_info()` on skip, store on capture
struct, re-apply on reconnect. Or mark source "pending reconnect" to prevent
teardown.

### 3B. No retry after skip

**File:** `patches/obs-studio/screencast-portal.c`

After skip return, session abandoned. No mechanism to retry `SelectSources`
when window reappears. Cosmic rescue hook (2A) can recover server-side, but
OBS has already given up.

**Fix:** Keep session alive, retry `SelectSources` with backoff. Or rely on
Cosmic rescue (mode=1 → Cosmic waits → late response=0).

### 3C. `restore_fail_mode` hardcoded to 1

**File:** `patches/obs-studio/screencast-portal.c:56`

OBS always sends `restore_fail_mode=1` (skip). No way to opt into error (2)
or prompt (0) per-source. Error mode is dead code from OBS's perspective.

**Fix:** OBS source property (dropdown: prompt/skip/error). Default skip.

### 3D. GVariant leak on skip path

**File:** `patches/obs-studio/screencast-portal.c:32–38`

```c
g_variant_get(parameters, "(u@a{sv})", &response, &ret);
if (response == 1 && capture->restore_token && *capture->restore_token) {
    blog(LOG_WARNING, "...");
    return;  // ← ret never unref'd
}
```

`@` in format string transfers a new ref. Early return leaks it.

**Fix:** `g_variant_unref(ret)` before return, or `g_autoptr(GVariant)`.

### 3E. `restore_fail_mode` without `restore_token` undefined

**File:** `patches/obs-studio/screencast-portal.c:54–56`

OBS only sends mode when token exists. But no guard on the portal side if a
**different** caller sends mode without a token. xdp-portal spec says "has no
effect if no restore_token is provided" but no code enforces this.

**Fix:** Portal-side: ignore `restore_fail_mode` when no restore_token
present (xdp already does this via `had_restore_token` check). Document the
contract explicitly in the XML.

---

## Phase 4 — Portal-layer hardening (xdg-desktop-portal)

Independent of other phases. All items parallel.

### 4A. `source_label` length not enforced

**File:** `patches/xdg-desktop-portal/screen-cast.c:118`

Validator is `NULL`. 100KB label passes through to all backends.

**Fix:** Add `validate_source_label` that rejects > 256 bytes.

### 4B. `restore_fail_mode` double-validated then silently clamped

**File:** `patches/xdg-desktop-portal/screen-cast.c:95–110, 139–140`

`validate_restore_fail_mode` rejects values > 2 with D-Bus error (good).
Then inline code clamps `> 2` to `0` (dead code under normal flow). If
validation is bypassed, clamp silently hides the error.

**Fix:** Remove redundant clamp. Trust the validator.

### 4C. Session closed before response — no error check

**File:** `patches/xdg-desktop-portal/screen-cast.c:168–177`

`xdp_session_close(session, TRUE)` called after response emitted. If close
fails, no error propagated. Zombie session possible.

**Fix:** Check return, log warning.

### 4D. TOCTOU on `had_restore_token`

**File:** `patches/xdg-desktop-portal/screen-cast.c:133–178`

`had_restore_token` captured before `replace_screen_cast_restore_token_with_data`
mutates options. If replacement removes token on failure, subsequent check
uses stale state.

**Fix:** Capture from replacement result, or re-check after mutation.

### 4E. Missing D-Bus error responses for skip/error

Skip (response=1) and error (response=2) communicated via signal only. If
caller ignores the Response signal, it holds a stale session handle with no
indication session was closed server-side. No D-Bus method error returned.

**Fix:** This is inherent to the portal Request pattern. Document that
callers MUST listen for Response. Optionally emit `Session::Closed` signal
as a belt-and-suspenders.

---

## Phase 5 — libportal hardening

Independent of other phases. All items parallel.

### 5A. No pre-flight validation

**File:** `patches/libportal/remote.c:144`

`restore_fail_mode=99` or 1MB `source_label` sent to portal as-is. Errors
surface asynchronously (or silently, per 4B).

**Fix:** Clamp `restore_fail_mode` to [0,2] in
`xdp_portal_create_screencast_session_full`. Truncate `source_label` to
256 bytes. Log warnings on invalid input.

### 5B. Empty string vs NULL `source_label` not normalized

**File:** `patches/libportal/remote.c:106`

`g_strdup("")` → non-NULL empty string → added to D-Bus call. Spec says
treat empty as absent, but libportal passes it through.

**Fix:** In `select_sources()`: treat `source_label` of `""` as `NULL`
(skip the `g_variant_builder_add`).

### 5C. `restore_fail_mode` always sent when version >= 6

**File:** `patches/libportal/remote.c:141`

```c
g_variant_builder_add(&options, "{sv}", "restore_fail_mode",
                      g_variant_new_uint32(call->restore_fail_mode));
```

Always added when v6+, even if caller didn't set it (defaults to 0 = prompt).
This is technically harmless (prompt is default behavior), but it sends a
redundant key that older backends may not expect.

**Fix:** Only add `restore_fail_mode` when explicitly set by caller and
value != 0. Or accept the current behavior as valid (0 = default).

---

## Phase 6 — Cross-layer contract gaps

These are architectural/spec-level gaps, not code bugs. Address after
implementation phases.

### 7A. No validation contract between layers

libportal: no pre-flight validation. xdg-desktop-portal: silently clamps.
Cosmic: stores verbatim. OBS: hardcodes mode=1. Each layer trusts the next.
No single layer rejects invalid input with a clear error to the caller.

**Fix:** Define validation responsibility: portal validates, backends trust
portal, libportal validates for better error messages. Document in spec XML.

### 7B. Missing D-Bus error propagation

When `restore_fail_mode` triggers skip or error at the portal layer, the
session is closed server-side. But the method returns normally
(`complete_select_sources`), and the failure is communicated only via
the async Response signal. A caller that doesn't subscribe to signals will
never know.

**Fix:** Spec-level: document that callers MUST subscribe to Response before
calling SelectSources. This is already implicit in the portal pattern, but
not stated for the new failure modes.

---

## Phase 7 — Test coverage

Independent of all phases. Work in parallel with everything.

### Mock: source_label edge cases (5 tests)

- `8A` Empty string `""` — recorded as `Some("")` or `None`?
- `8B` Whitespace-only `"   "`
- `8C` String exceeding 256 bytes
- `8D` Unicode: emoji (`"📹 Camera"`), CJK, RTL markers
- `8E` Control characters: `\0`, `\n`, `\t`

Currently only tested: ASCII labels.

### Mock: restore_fail_mode edge cases (3 tests)

- `8F` `restore_fail_mode` without `restore_token` — verify no effect,
  response=0.
- `8G` Invalid values (3, 255, `u32::MAX`) through D-Bus — verify behavior
  (currently: silently falls back to Prompt via `.unwrap_or()`).
- `8H` `restore_fail_mode=0` with stale token — explicit prompt-mode test
  (currently tested in `prompt_fires_response_0` but only with
  `RestoreTokenFails` scenario).

### Mock: session lifecycle after skip/error (4 tests)

- `8I` After response=1 (skip), call `SelectSources` again — does it work?
- `8J` After response=2 (error), call `Start` — what happens?
- `8K` After response=1, call `SelectSources` with fresh token (no
  `restore_fail_mode`) — recovery path.
- `8L` After response=1, call `SelectSources` with same stale token but
  mode=0 — prompt fallback.

### Mock: concurrent/interleaved sessions (2 tests)

- `8M` Two sessions selecting sources simultaneously.
- `8N` Interleaved: `CreateSession(A)` → `CreateSession(B)` →
  `SelectSources(A)` → `SelectSources(B)`.

### Mock: protocol edge cases (4 tests)

- `8O` `SelectSources` with non-existent `session_handle`.
- `8P` Double `SelectSources` on same session with different options.
- `8Q` `response=2` without `restore_failed` flag — verify caller handles
  gracefully.
- `8R` `Start` results on success: assert `restore_failed` is absent,
  `streams` contains expected `node_id`.

### Mock: version negotiation (3 tests)

- `8S` Portal version 0 — degenerate case.
- `8T` Portal version 4 — previous stable.
- `8U` Portal version 99 — future-proof.

### Mock: rescue-like scenario (2 tests)

- `8V` `DelayedRestore` scenario: `on_select_sources` returns `Err`, then
  fires late response=0 from background task after delay. Simulates rescue.
- `8W` Multiple stale-token sessions pending simultaneously with different
  policies.
