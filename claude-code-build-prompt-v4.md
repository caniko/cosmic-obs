# screencast-portal-rfc — full agent team build

You are the **team lead**. Your job is to coordinate an agent team that
produces a complete, working, Nix-packaged implementation of the
ScreenCast portal RFC: two new options (`source_label` and
`restore_fail_policy`) added to `SelectSources()` across the entire
portal stack, from the spec down to OBS Studio.

**Immediately create an agent team with six teammates.** The work spans
multiple languages, multiple upstream repos, and has a clear dependency
graph. Teammates operate in two phases; you manage the handoff.

---

## What gets built

```
patches/xdg-desktop-portal/     ← spec XML + C frontend (phase 1)
patches/libportal/               ← C API wrapper (phase 2)
patches/xdg-desktop-portal-hyprland/  ← C++ picker display (phase 2)
patches/obs-studio/              ← C consumer (phase 3)
screencast-portal-mock/          ← Rust test harness (phase 1)
flake.nix + nix/                 ← packages all of the above (phase 4)
```

## Dependency graph

```
spec (xdg-desktop-portal XML + C)
    ├── [phase 2, parallel] libportal C patch
    ├── [phase 2, parallel] XDPH C++ patch
    └── [phase 1, parallel] mock Rust crate (spec is already known)
              ↓
    [phase 3] obs C patch (needs libportal done)
              ↓
    [phase 4] nix (needs all patches to exist)
```

## Phase sequencing

**Phase 1 — spawn immediately, in parallel:**
- teammate `spec`
- teammate `mock`

**Phase 2 — spawn after `spec` messages "spec done":**
- teammate `libportal`
- teammate `xdph`

**Phase 3 — spawn after `libportal` messages "libportal done":**
- teammate `obs`

**Phase 4 — spawn after ALL five teammates message done:**
- teammate `nix`

**Final pass — after `nix` messages done:**
- You (the lead) run integration, DRY/KISS audit, and nix build.

---

## Before spawning any teammates

### Environment and working directory

All work happens in `/data/lnvme/can/obs-work/`. This is already the
working directory. Do not create a subdirectory — work directly here.

Redirect all temporary build artefacts onto the same fast nvme, away
from the root partition:

```bash
export TMPDIR=/data/lnvme/can/obs-work/.tmp
export NIX_BUILD_TMPDIR=/data/lnvme/can/obs-work/.tmp
mkdir -p "$TMPDIR"
```

### Pre-flight disk check

```bash
df -h /data/lnvme
df -h /nix 2>/dev/null || df -h /
```

If `/nix` or `/` has less than 20 GB free, run a Nix store GC before
proceeding:

```bash
nix store gc
nix-collect-garbage -d
```

### Scaffold

```bash
git init
git config user.email "agent@build" && git config user.name "agent"
mkdir -p patches/xdg-desktop-portal \
         patches/libportal \
         patches/xdg-desktop-portal-hyprland \
         patches/obs-studio \
         screencast-portal-mock/src \
         nix \
         .tmp
touch screencast-portal-mock/Cargo.toml
echo ".tmp/" >> .gitignore
echo "result" >> .gitignore
echo "result-*" >> .gitignore
git add -A && git commit -m "scaffold"
```

Then write `CLAUDE.md` in the repo root with this content — it is shared
context for every teammate:

```markdown
# screencast-portal-rfc

## What this repo is
A mono-repo containing patches for five upstream projects and a new Rust
test harness, all packaged via a single Nix flake. The goal is to ship
two new options in the ScreenCast portal's SelectSources() call:

  source_label (string): human-readable hint from the caller identifying
    which application-level source this D-Bus call corresponds to.
    Displayed by portal backends in the source picker dialog.

  restore_fail_policy (u32): controls what the portal does when a restore
    token cannot be honoured:
      0 = prompt  (default, current behaviour — show picker)
      1 = skip    (fire response=1, no picker shown)
      2 = error   (fire response=2 with restore_failed=true in results)

## RFC summary
Interface version bumped: 5 → 6.
New keys are backward-compatible: old portals and old callers ignore them.
Callers must check portal version >= 6 before sending new keys.

## Repo layout
  patches/xdg-desktop-portal/          patch for upstream xdg-desktop-portal
  patches/libportal/                    patch for upstream libportal
  patches/xdg-desktop-portal-hyprland/ patch for upstream XDPH
  patches/obs-studio/                  patch for upstream OBS Studio
  screencast-portal-mock/              new Rust crate (mock D-Bus server)
  nix/                                 per-package Nix derivation files
  flake.nix                            master flake

## Style rules

### Rust (screencast-portal-mock only)
- Static dispatch by default: `impl Trait` / `<T: Trait>`.
  `dyn Trait` only when the type is caller-chosen at runtime; every such
  usage must have a comment explaining why static dispatch is impossible.
- No `unwrap()` in library code. `?` and `thiserror` enums only.
- One `#[derive(thiserror::Error)]` enum per module boundary.
- `#[must_use]` on all functions returning Result or meaningful values.
- DRY: extract shared logic. KISS: simplest correct impl wins.
- Functions > ~30 lines → split.
- No dead code, no unused imports, no commented-out blocks.

### C / C++ (all patches)
- Match the style of the surrounding upstream code exactly.
- No new allocations without corresponding frees.
- Follow existing GLib/GObject patterns in xdg-desktop-portal and libportal.
- Follow existing GVariant builder patterns in OBS screencast-portal.c.

### Nix
- flake-parts for all structure. Never bare `outputs = { ... }:`.
- crane + fenix for the Rust crate.
- treefmt-nix for formatting (rustfmt + alejandra).
- `nix fmt` and `nix flake check` must pass.

## Disk and resource hygiene (mandatory)

Working drive: /data/lnvme/can/obs-work/ (~444 GB free on /dev/nvme1n1p1).
Temp dir: /data/lnvme/can/obs-work/.tmp — TMPDIR must point here always.

nix build rules — non-negotiable:
- NEVER run two `nix build` commands in parallel. Always strictly sequential.
- After every successful nix build, immediately run: `nix store gc`
- Build obs-studio last and alone, never alongside any other build.
- OBS must always be built with: `nix build .#obs-studio --cores 8`
- Delete result symlinks immediately after inspection: `rm -f result result-*`
- Delete /tmp clones when done with them: `rm -rf /tmp/xdp /tmp/libportal /tmp/xdph /tmp/obs /tmp/verify-*`
- If any build fails mid-way: run `nix store gc` before retrying.
- After the entire session completes: run `nix-collect-garbage -d`
```

---

## Shared blocks — paste into every teammate prompt verbatim

### SHARED STYLE BLOCK
```
STYLE (non-negotiable — apply to every line you write):
Rust: static dispatch by default, dyn only with comment, no unwrap() in lib
code, thiserror enums, #[must_use] on Result-returning fns, DRY, KISS,
functions >30 lines split, no dead code.
C/C++: match surrounding upstream style exactly, GLib/GVariant patterns.
Nix: flake-parts, crane+fenix, treefmt-nix, nix fmt must pass.
Full rules are in CLAUDE.md in the repo root.
```

### SHARED RFC BLOCK
```
RFC SUMMARY:
Two new keys added to SelectSources() options vardict (interface version 5→6):

  source_label (s, optional):
    Human-readable hint from the caller. The portal backend SHOULD display
    this in the picker dialog. Callers MUST only send if portal version >= 6.

  restore_fail_policy (u, optional, default=0):
    Controls behaviour when restore_token is present but cannot be honoured.
      0 = prompt  → current behaviour: show picker (default, backward compat)
      1 = skip    → fire Request::Response(response=1, {})
      2 = error   → fire Request::Response(response=2, {restore_failed: true})
    Callers MUST only send if portal version >= 6.

One new key added to Start() results vardict:
  restore_failed (b, optional):
    Present and true when response=2 and failure was a restore token failure.

Interface version property: 5 → 6.
All changes are backward-compatible. Unknown vardict keys are silently ignored.
```

---

## Teammate instructions

### teammate: `spec`

```
You are the spec teammate. Your domain: the xdg-desktop-portal patch.
[paste SHARED STYLE BLOCK]
[paste SHARED RFC BLOCK]

## Your task
Clone xdg-desktop-portal, apply the RFC changes as a git patch, save the
patch file to patches/xdg-desktop-portal/.

## Steps

1. Clone:
   git clone --depth=1 https://github.com/flatpak/xdg-desktop-portal.git /tmp/xdp
   cd /tmp/xdp

2. Edit data/org.freedesktop.portal.ScreenCast.xml:
   a. Find the <interface version="5"> tag and change to version="6".
   b. Inside the SelectSources method's options documentation block, add
      after the existing restore_token entry:

      * ``source_label`` (``s``)
        A human-readable label for this source request. An optional hint
        provided by the application to identify the purpose or origin of
        this SelectSources call. If supported by the portal backend, this
        string should be surfaced in the source picker dialog. Backends
        that do not support this key must ignore it. This option was added
        in version 6 of this interface.

      * ``restore_fail_policy`` (``u``)
        Determines what the portal does when a restore_token is provided
        but the session cannot be restored. Allowed values:
          0 - prompt (default): show the source picker as if no token was
              provided. This is the default and preserves current behaviour.
          1 - skip: do not show the picker; return response=1 via the
              Request::Response signal.
          2 - error: do not show the picker; return response=2 with
              restore_failed=true in the results vardict.
        Has no effect if no restore_token is provided. This option was
        added in version 6 of this interface.

   c. In the Start method results documentation, after the restore_token
      entry, add:

      * ``restore_failed`` (``b``)
        Present and set to true when response is 2 and the failure was
        caused specifically by an inability to honour a provided
        restore_token combined with restore_fail_policy set to 2.
        This result was added in version 6 of this interface.

3. Edit src/screencast.c (find the file with handle_select_sources or
   similar — use `grep -r "SelectSources" src/` to locate it).

   Find where the portal reads restore_token from the caller's options
   vardict and performs the permission store lookup. This is the section
   that currently always falls through to re-prompting on failure.

   Add the following logic:
   a. After reading restore_token, also read restore_fail_policy (u32,
      default 0) from the caller's options vardict.
   b. Read source_label (string, may be absent) from the caller's options
      vardict and pass it through in the options forwarded to the backend's
      SelectSources call.
   c. When the restore token lookup fails (token not found in permission
      store, or backend signals restoration failure), branch on
      restore_fail_policy:
        policy 0 → existing behaviour (fall through to prompt)
        policy 1 → immediately complete the request with response=1 and
                   empty results, without calling the backend
        policy 2 → immediately complete the request with response=2 and
                   a results vardict containing restore_failed=TRUE,
                   without calling the backend

   Pattern for reading u32 from vardict (match existing code style):
     guint32 restore_fail_policy = 0;
     if (g_variant_lookup (options, "restore_fail_policy", "u",
                           &restore_fail_policy)) {
       /* clamp to known values */
       if (restore_fail_policy > 2)
         restore_fail_policy = 0;
     }

   Pattern for passing source_label through to backend options:
     const char *source_label = NULL;
     if (g_variant_lookup (options, "source_label", "&s", &source_label) &&
         source_label != NULL) {
       g_variant_builder_add (&backend_options_builder, "{sv}",
                               "source_label",
                               g_variant_new_string (source_label));
     }

4. Generate the patch:
   git add -A
   git commit -m "screencast: add source_label and restore_fail_policy to SelectSources"
   git format-patch HEAD~1 -o /path/to/repo/patches/xdg-desktop-portal/

5. Verify the patch applies cleanly:
   cd /tmp/xdp2 && git clone --depth=1 https://github.com/flatpak/xdg-desktop-portal.git .
   git apply /path/to/repo/patches/xdg-desktop-portal/*.patch
   # Must apply with no errors

When done, message the team lead: "spec done — patch at patches/xdg-desktop-portal/"
```

---

### teammate: `mock`

```
You are the mock teammate. Your domain: the screencast-portal-mock Rust crate.
[paste SHARED STYLE BLOCK]
[paste SHARED RFC BLOCK]

## Your task
Build the screencast-portal-mock crate from scratch in screencast-portal-mock/.
This is a headless D-Bus mock server implementing org.freedesktop.portal.ScreenCast.
It enables integration tests for OBS and other portal consumers without a compositor.

## Cargo.toml
[package]
name = "screencast-portal-mock"
version = "0.1.0"
edition = "2024"

[dependencies]
zbus        = { version = "5", features = ["tokio"] }
tokio       = { version = "1", features = ["full"] }
thiserror   = "2"
async-trait = "0.1"
bitflags    = "2"
tracing     = "0.1"
tempfile    = "3"

[dev-dependencies]
tokio              = { version = "1", features = ["full", "test-util"] }
proptest           = "1"
tracing-subscriber = "0.3"

## D-Bus facts (critical)
- Well-known name to request: org.freedesktop.portal.Desktop
- Object path: /org/freedesktop/portal/desktop
- Interface: org.freedesktop.portal.ScreenCast
- Every portal method returns a Request object path immediately.
  The actual result arrives via org.freedesktop.portal.Request::Response
  signal fired asynchronously AFTER the method returns.
  The tokio::spawn for the signal fire is mandatory — never fire inline.
- Session objects: /org/freedesktop/portal/desktop/session/N
- Request objects: /org/freedesktop/portal/desktop/request/N
- Counter for N: Arc<AtomicU64>, increment per object

## Files to create

src/records.rs — all shared types:
  RestoreFailure { reason: RestoreFailReason }
  RestoreFailReason { TokenNotFound, SourceUnavailable, PermissionRevoked, TokenConsumed }
  RestoreFailPolicy #[repr(u32)] { Prompt=0, Skip=1, Error=2 } + TryFrom<u32>
  SourceTypes bitflags { MONITOR=1, WINDOW=2, VIRTUAL=4 }
  Call {
    timestamp: Instant, session_handle: OwnedObjectPath,
    source_types: SourceTypes, multiple: bool, cursor_mode: u32,
    persist_mode: u32, restore_token: Option<String>,
    source_label: Option<String>,            // None = caller did not send
    restore_fail_policy: Option<RestoreFailPolicy>,  // None = not sent
    raw_options: HashMap<String, OwnedValue>,
  }
  SourceDef { node_id: u32, expected_label: Option<String>,
              restore_valid: bool, size: (u32,u32), position: (i32,i32) }
  impl SourceDef: monitor(u32)->Self, with_label(self, impl Into<String>)->Self (#[must_use]),
                  invalid(self)->Self (#[must_use])

src/scenario.rs — Scenario trait + built-ins:
  Type aliases: Options = HashMap<String,OwnedValue>, ExtraResults = HashMap<String,OwnedValue>
  StartResult { response: u32, results: ExtraResults }

  #[async_trait]
  trait Scenario: Send + Sync + 'static {
    async fn on_create_session(&self, options: &Options) -> ExtraResults { HashMap::new() }
    async fn on_select_sources(&self, options: &Options)
      -> Result<ExtraResults, RestoreFailure> { Ok(HashMap::new()) }
    async fn on_start(&self, options: &Options) -> StartResult
      { /* default: response=0, one stream node_id=1 1920x1080 */ }
    fn version(&self) -> u32 { 6 }
  }

  Built-ins: NormalSession, RestoreTokenValid{token,node_id},
  RestoreTokenFails{reason}, UserCancels, MultiSource{sources:Vec<SourceDef>},
  SlowResponse{delay:Duration, inner:Box<dyn Scenario>}
    // Box<dyn Scenario> here because inner is caller-chosen at runtime
  OldPortal{version:u32}

src/bus.rs — PrivateBus:
  spawn() -> Result<Self, BusError>
    dbus-daemon --session --print-address=1 --nofork
    read address from stdout line 1 before returning
  address() -> &str
  connect() -> Result<zbus::Connection, BusError>
    connects + requests name org.freedesktop.portal.Desktop

src/portal.rs — zbus server:
  RequestObject: #[zbus::interface(name="org.freedesktop.portal.Request")]
    close(), signal response(u32, HashMap<String,OwnedValue>)
  SessionObject: #[zbus::interface(name="org.freedesktop.portal.Session")]
    close(), signal closed()
  ScreenCastPortal: #[zbus::interface(name="org.freedesktop.portal.ScreenCast")]
    State: scenario: Arc<dyn Scenario>, calls: Arc<Mutex<Vec<Call>>>,
           conn: zbus::Connection, counter: Arc<AtomicU64>
    Methods: create_session, select_sources, start, open_pipe_wire_remote
    Property: version() -> u32

  SelectSources logic implements RFC semantics exactly:
    parse Call fields (source_label, restore_fail_policy, restore_token, ...)
    push to self.calls
    scenario.on_select_sources().await
    register RequestObject
    tokio::spawn: fire Response based on result + policy:
      Ok(_)  → Response(0, {})
      Err + policy=Prompt → Response(0, {})  // simulated prompt
      Err + policy=Skip   → Response(1, {})
      Err + policy=Error  → Response(2, {"restore_failed": true})

  pub struct MockPortal
  impl MockPortal:
    pub async fn start<S: Scenario>(bus: &PrivateBus, s: S) -> Result<MockPortalHandle, PortalError>
    // S: Scenario at call site (static dispatch). Arc<dyn Scenario> created inside.

src/handle.rs — MockPortalHandle:
  calls: Arc<Mutex<Vec<Call>>>, _task: JoinHandle<()>
  wait_for_calls(n, timeout) -> Result<(), WaitError>   polls every 50ms
  calls() -> Vec<Call>
  assert_call_count, assert_source_label, assert_no_source_label,
  assert_restore_fail_policy, assert_no_prompts

src/obs.rs — ObsProcess:
  launch(bus) -> Result<Self, io::Error>  (DBUS_SESSION_BUS_ADDRESS override)
  kill(), Drop sends SIGTERM

src/lib.rs — pub re-exports of everything

tests/test_scenarios.rs — direct zbus client tests (NO OBS needed):
  normal_session_full_flow, user_cancels, slow_response_timing,
  multi_source_stream_count, restore_token_valid, restore_token_invalid_prompt

tests/test_restore_fail_policy.rs — drive mock via zbus client:
  skip_fires_response_1, error_fires_response_2_with_flag, prompt_fires_response_0

tests/test_source_label.rs — drive mock via zbus client:
  label_recorded_when_present, none_recorded_when_absent, multiple_distinct_labels

tests/test_backward_compat.rs:
  v5_portal_reports_version_5, v6_portal_reports_version_6
  // OBS tests are #[ignore = "requires OBS"] placeholders

Run `cargo check` after each file. Run `cargo test` when all files exist.
All non-ignored tests must pass.

When done, message the team lead: "mock done — all tests pass"
```

---

### teammate: `libportal`

```
You are the libportal teammate. Your domain: the libportal C patch.
[paste SHARED STYLE BLOCK]
[paste SHARED RFC BLOCK]

Wait until the spec teammate has confirmed the XML patch is done, then proceed.

## Your task
Clone libportal, add C API for the two new options, generate patch file.

1. Clone:
   git clone --depth=1 https://github.com/flatpak/libportal.git /tmp/libportal
   cd /tmp/libportal

2. Understand the existing API first:
   cat libportal/remote.h | grep -A5 "create_screencast"
   cat libportal/remote.c | grep -A30 "xdp_portal_create_screencast_session"

3. In libportal/remote.h, after the existing XdpPersistMode enum, add:

   /**
    * XdpRestoreFailPolicy:
    * @XDP_RESTORE_FAIL_POLICY_PROMPT: Show the source picker (default).
    * @XDP_RESTORE_FAIL_POLICY_SKIP: Return cancelled without prompting.
    * @XDP_RESTORE_FAIL_POLICY_ERROR: Return error with restore_failed flag.
    *
    * Determines what happens when a restore token cannot be honoured.
    * Requires portal interface version 6 or later.
    */
   typedef enum {
     XDP_RESTORE_FAIL_POLICY_PROMPT = 0,
     XDP_RESTORE_FAIL_POLICY_SKIP   = 1,
     XDP_RESTORE_FAIL_POLICY_ERROR  = 2,
   } XdpRestoreFailPolicy;

   Add a new function declaration after xdp_portal_create_screencast_session:

   XDP_PUBLIC
   void xdp_portal_create_screencast_session_full
     (XdpPortal              *portal,
      XdpOutputType           output_types,
      XdpScreencastFlags      flags,
      XdpCursorMode           cursor_mode,
      XdpPersistMode          persist_mode,
      const char             *restore_token,
      const char             *source_label,
      XdpRestoreFailPolicy    restore_fail_policy,
      GCancellable           *cancellable,
      GAsyncReadyCallback     callback,
      gpointer                data);

4. In libportal/remote.c, implement xdp_portal_create_screencast_session_full:
   - Copy the body of xdp_portal_create_screencast_session
   - Before the SelectSources call, check portal interface version:
       if (portal_has_screencast_version (portal, 6)) {
         if (source_label != NULL)
           g_variant_builder_add (&options, "{sv}", "source_label",
                                  g_variant_new_string (source_label));
         g_variant_builder_add (&options, "{sv}", "restore_fail_policy",
                                g_variant_new_uint32 (restore_fail_policy));
       }
   - Add a helper function portal_has_screencast_version(portal, min_version)
     that reads the org.freedesktop.portal.ScreenCast.version D-Bus property
     and compares it. Cache the result on the portal object if an appropriate
     field exists, otherwise just do a synchronous property read.

   Make xdp_portal_create_screencast_session call _full with:
     source_label = NULL, restore_fail_policy = XDP_RESTORE_FAIL_POLICY_PROMPT

5. In libportal/portal-helpers.c or wherever portal_version helpers live
   (grep for version-checking patterns): add portal_has_screencast_version.

6. Generate patch:
   git add -A
   git commit -m "remote: add source_label and restore_fail_policy to screencast API"
   git format-patch HEAD~1 -o /path/to/repo/patches/libportal/

7. Verify the patch applies cleanly to a fresh clone.

When done, message the team lead: "libportal done — patch at patches/libportal/"
```

---

### teammate: `xdph`

```
You are the xdph teammate. Your domain: the xdg-desktop-portal-hyprland patch.
[paste SHARED STYLE BLOCK]
[paste SHARED RFC BLOCK]

Wait until the spec teammate has confirmed done, then proceed.

## Your task
Clone XDPH, make the picker UI display source_label, generate patch file.

1. Clone:
   git clone --depth=1 https://github.com/hyprwm/xdg-desktop-portal-hyprland.git /tmp/xdph
   cd /tmp/xdph

2. Understand the picker structure:
   find . -name "*.cpp" | xargs grep -l "source\|label\|dialog\|picker" | head -10
   # The picker is likely in src/shared/hyprland-share-picker.cpp or similar
   # Find the code that renders the list of pending share requests

3. Find where SelectSources requests are received and the picker is shown.
   Look for where the options vardict is parsed.
   The options dict is a GVariant a{sv} passed from xdg-desktop-portal frontend.

4. Add source_label extraction after existing option parsing:
   std::string sourceLabel;
   if (auto* v = g_variant_lookup_value(options, "source_label", G_VARIANT_TYPE_STRING)) {
     sourceLabel = g_variant_get_string(v, nullptr);
     g_variant_unref(v);
   }

5. Pass sourceLabel to the picker window/dialog constructor (or wherever
   the window title/subtitle is set). Display it as a subtitle line under
   the main "Select a source" heading if non-empty.

   The exact code depends on the picker implementation — use Qt/GTK widget
   calls that match the surrounding code. If the picker uses Qt:
     if (!sourceLabel.empty())
       dialog->setWindowTitle(QString::fromStdString(sourceLabel));
   If it uses GTK:
     if (!sourceLabel.empty())
       gtk_window_set_subtitle(GTK_WINDOW(dialog), sourceLabel.c_str());

   Do not crash or misbehave if source_label is absent — it's optional.

6. Generate patch:
   git add -A
   git commit -m "screencast: display source_label in share picker"
   git format-patch HEAD~1 -o /path/to/repo/patches/xdg-desktop-portal-hyprland/

7. Verify the patch applies cleanly.

When done, message the team lead: "xdph done — patch at patches/xdg-desktop-portal-hyprland/"
```

---

### teammate: `obs`

```
You are the obs teammate. Your domain: the OBS Studio C patch.
[paste SHARED STYLE BLOCK]
[paste SHARED RFC BLOCK]

Wait until the libportal teammate confirms done, then proceed.

## Your task
Clone OBS Studio, patch plugins/linux-pipewire/screencast-portal.c to send
source_label and restore_fail_policy in SelectSources().

1. Clone:
   git clone --depth=1 https://github.com/obsproject/obs-studio.git /tmp/obs
   cd /tmp/obs

2. Study the file:
   cat plugins/linux-pipewire/screencast-portal.c

   Key functions to understand:
   - screencast_portal_create_session(): starts the portal flow
   - On the response to CreateSession: select_sources() is called
   - select_sources(): builds a GVariantBuilder and calls SelectSources D-Bus method
   - The GVariantBuilder adds: cursor_mode, types, multiple, persist_mode,
     restore_token (if present), handle_token

3. Find the struct holding capture state (struct screencast_portal_capture or similar).
   It has fields like session_handle, cancellable, obs_pw, source, capture_type.
   Add two new fields:
     uint32_t portal_version;   /* cached version property, 0 = unknown */
     bool portal_version_read;  /* true after first read */

4. Add a helper function that reads the portal version property exactly once
   and caches it:

   static uint32_t get_portal_screencast_version(
       struct screencast_portal_capture *capture)
   {
     if (capture->portal_version_read)
       return capture->portal_version;
     capture->portal_version_read = true;
     capture->portal_version = 0;

     GDBusProxy *proxy = get_screencast_portal_proxy();
     if (!proxy)
       return 0;

     GVariant *v = g_dbus_proxy_get_cached_property(proxy, "version");
     if (v) {
       capture->portal_version = g_variant_get_uint32(v);
       g_variant_unref(v);
     }
     return capture->portal_version;
   }

5. In the select_sources() function, after the existing builder adds
   (restore_token, persist_mode, multiple, types, cursor_mode), add:

   /* RFC: source_label and restore_fail_policy (portal version >= 6) */
   if (get_portal_screencast_version(capture) >= 6) {
     const char *source_name = obs_source_get_name(capture->source);
     if (source_name && *source_name) {
       g_variant_builder_add(&builder, "{sv}", "source_label",
                             g_variant_new_string(source_name));
     }
     /* On startup restore, skip re-prompting if the token is stale */
     if (capture->restore_token) {
       g_variant_builder_add(&builder, "{sv}", "restore_fail_policy",
                             g_variant_new_uint32(1)); /* SKIP */
     }
   }

   IMPORTANT: the restore_fail_policy=1 (skip) block must only execute
   when capture->restore_token is non-NULL (i.e. we are attempting a
   restore, not a fresh capture). A fresh capture has no restore token
   and must always show the picker.

6. In the response handler for SelectSources (on_select_source_response_received_cb
   or similar), add handling for response=1 (skip due to policy):
   If response == 1 and there was a restore_token, log a warning:
     blog(LOG_WARNING, "[pipewire] Source restore skipped: token stale or unavailable");
   And gracefully skip this source (treat like a cancelled fresh session —
   clear the source state without crashing).

7. In the response handler for Start (on_start_response_received_cb), check
   for restore_failed in the results vardict:
     g_autoptr(GVariant) restore_failed_v =
       g_variant_lookup_value(result, "restore_failed", G_VARIANT_TYPE_BOOLEAN);
     if (restore_failed_v && g_variant_get_boolean(restore_failed_v)) {
       blog(LOG_WARNING, "[pipewire] Source restore failed: portal reported error");
       /* treat same as response != 0: clear state, do not crash */
       return;
     }

8. Generate patch:
   git add -A
   git commit -m "linux-pipewire: send source_label and restore_fail_policy to portal"
   git format-patch HEAD~1 -o /path/to/repo/patches/obs-studio/

9. Verify the patch applies cleanly.

When done, message the team lead: "obs done — patch at patches/obs-studio/"
```

---

### teammate: `nix`

```
You are the nix teammate. Your domain: all Nix files.
Do not touch any Rust or C files.
[paste SHARED STYLE BLOCK]

Wait until all five other teammates have confirmed done before starting.
All patches now exist under patches/.

## Your task
Write a Nix flake that packages every component: the mock crate, and
patched builds of xdg-desktop-portal, libportal, XDPH, and OBS Studio.

## File: flake.nix

Inputs:
  nixpkgs        = github:NixOS/nixpkgs/nixos-unstable
  flake-parts    = github:hercules-ci/flake-parts
  crane          = github:ipetkov/crane
  fenix          = github:nix-community/fenix        (follows nixpkgs)
  treefmt-nix    = github:numtide/treefmt-nix
  rust-flake     = github:srid/rust-flake

Imports in flake.nix:
  [ rust-flake.flakeModules.default treefmt-nix.flakeModule ]

perSystem configuration:
  rust-project.toolchain = fenix stable with components
    ["rustc" "cargo" "clippy" "rustfmt" "rust-src" "rust-analyzer"]
  rust-project.crates."screencast-portal-mock" = {
    path = ./screencast-portal-mock;
    autoWire = ["crate" "clippy"];
  }
  treefmt.config.programs.rustfmt.enable = true;
  treefmt.config.programs.alejandra.enable = true;

  devShells.default: includes rust toolchain, pkgs.dbus, pkgs.pkg-config,
    pkgs.cargo-nextest, pkgs.glib, pkgs.pipewire

Top-level packages (flake-parts perSystem):
  packages.screencast-portal-mock  (from crane autoWire)
  packages.xdg-desktop-portal      (patched, see nix/xdg-desktop-portal.nix)
  packages.libportal               (patched, see nix/libportal.nix)
  packages.xdg-desktop-portal-hyprland (patched, see nix/xdph.nix)
  packages.obs-studio              (patched, see nix/obs-studio.nix)

nixosModules.default:
  A NixOS module that wires the patched stack together:
    options.services.screencast-portal-rfc.enable = mkEnableOption "..."
    config = mkIf cfg.enable {
      nixpkgs.overlays = [ (final: prev: {
        xdg-desktop-portal = packages.xdg-desktop-portal;
        libportal           = packages.libportal;
        xdg-desktop-portal-hyprland = packages.xdg-desktop-portal-hyprland;
        obs-studio          = packages.obs-studio;
      }) ];
    }

## File: nix/xdg-desktop-portal.nix

pkgs.xdg-desktop-portal.overrideAttrs (old: {
  patches = (old.patches or []) ++ [
    ../patches/xdg-desktop-portal/0001-screencast-add-source-label-and-restore-fail-policy.patch
  ];
})

(Use the actual filename from patches/xdg-desktop-portal/ — glob it.)

## File: nix/libportal.nix

pkgs.libportal.overrideAttrs (old: {
  patches = (old.patches or []) ++ [
    ../patches/libportal/0001-remote-add-source-label-restore-fail-policy-api.patch
  ];
})

## File: nix/xdph.nix

pkgs.xdg-desktop-portal-hyprland.overrideAttrs (old: {
  patches = (old.patches or []) ++ [
    ../patches/xdg-desktop-portal-hyprland/0001-screencast-display-source-label-in-picker.patch
  ];
})

## File: nix/obs-studio.nix

pkgs.obs-studio.overrideAttrs (old: {
  patches = (old.patches or []) ++ [
    ../patches/obs-studio/0001-linux-pipewire-source-label-restore-fail-policy.patch
  ];
})

## File: rust-toolchain.toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy", "rust-src", "rust-analyzer"]

## File: .envrc
use flake

## After writing all Nix files:
git add flake.nix nix/ rust-toolchain.toml .envrc
nix flake lock
nix fmt
nix flake check --no-build   # checks fmt + clippy

Fix any errors until all three commands succeed.
Then attempt: nix build .#screencast-portal-mock
If network issues in sandbox: retry with --option sandbox false and note in README.md.

When done, message the team lead: "nix done — flake.lock generated, checks pass"
```

---

## Lead: final integration pass

After receiving all six "done" messages:

### 1. Cargo integration
```bash
cd screencast-portal-mock
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --lib
cargo test --test '*'
```
Fix all failures without asking.

### 2. DRY/KISS audit (Rust only)
Read every `src/*.rs` file:
- Duplicated logic → extract
- Functions >30 lines → split
- Every `dyn Trait` usage → verify comment exists
- Remove all `#[allow(...)]` without justification
- Dead code, unused imports → delete

### 3. Patch sanity check
For each patch file in `patches/*/`:
```bash
# Verify each patch applies to a fresh clone
git clone --depth=1 <upstream-url> /tmp/verify-<name>
git -C /tmp/verify-<name> apply /path/to/patch
echo "Exit: $?"  # must be 0
```

### 4. Full Nix build — strictly sequential, GC between each

```bash
nix fmt
nix flake check --no-build

# Each build immediately followed by GC. Never run two builds in parallel.

nix build .#screencast-portal-mock
rm -f result && nix store gc

nix build .#xdg-desktop-portal
rm -f result && nix store gc

nix build .#libportal
rm -f result && nix store gc

nix build .#xdg-desktop-portal-hyprland
rm -f result && nix store gc

# OBS last, cores capped to prevent OOM
nix build .#obs-studio --cores 8
rm -f result && nix store gc
```

Use `--option sandbox false` if a package needs network during build;
document each exception in README.md.

### 5. Final cleanup

```bash
rm -rf /tmp/xdp /tmp/libportal /tmp/xdph /tmp/obs /tmp/verify-*
rm -rf /data/lnvme/can/obs-work/.tmp/*
nix-collect-garbage -d
df -h /data/lnvme
df -h /nix 2>/dev/null || df -h /
echo "Nix store size:" && du -sh /nix/store
```

### 6. Write README.md
Cover:
- What this repo is
- How to use the NixOS module to deploy the patched stack
- How to run the mock against a patched OBS
- Known limitations
- How to contribute upstream (links to relevant issues/PRs)

### 7. Print final summary
- Full file tree (`find . -not -path './.tmp/*' -not -path './.git/*' | sort`)
- `cargo test --lib` result (pass count)
- `nix flake check --no-build` result
- Which `nix build` targets succeeded
- Which patch(es) may need manual adjustment (note honestly)
- Final disk usage: `df -h /data/lnvme` and `du -sh /nix/store`
