# Phase 01 — Repair the xdg-desktop-portal generic patch (memory safety + option forwarding)

> **Recommended Codex model: GPT 5.5 max** (`reasoning_effort: xhigh`)
>
> This is frontier-complexity work: two reachable C memory-safety bugs
> (double-free / use-after-free on the common restore-token path) plus a
> protocol-forwarding gap whose fix must be reasoned across three layers
> (OBS → xdg-desktop-portal → COSMIC). GVariant ownership and GObject refcount
> discipline are exactly the kind of subtle reasoning where a mediocre fix
> *looks* right, compiles, and ships a heap corruption. The role here is a
> careful leaf executor on a single patch file, but the correctness bar is the
> highest in the set — a wrong free crashes the portal for every screencast
> user. Do not downshift; a smaller model is likely to "fix" the GStrv bug by
> changing the cleanup attribute without re-checking the lookup format string,
> reintroducing the same class of bug.

## Working tree

`/data/nvme0/can/Projects/cosmic-obs` — the cosmic-obs repo. All edits are to
the single patch file
`patches/xdg-desktop-portal/0001-screencast-add-source_label-and-restore_policy.patch`;
verification builds the package via `nix build .#xdg-desktop-portal`. No other
phase touches this file, so there is no rebase hazard.

## Goal

The generic `xdg-desktop-portal` v6 patch is **memory-safe** and **forwards
every v6 option key end-to-end**. Specifically: the permissions lookup no longer
frees borrowed GVariant strings; the skip/error session-close path is
refcount-balanced; and `restore_match_rules` is in the SelectSources option
allow-list so it survives the frontend's option filter and reaches the backend.
`nix build .#xdg-desktop-portal` succeeds and `nix flake check` stays green.

## Why this matters now

Three audit findings, all in this one patch file, all on reachable paths:

1. **Critical — `restore_match_rules` is silently dropped (rescue feature dead
   in production).** The patch extends `screen_cast_select_sources_options[]`
   (patch line 189) with `source_label` (L195) and `restore_policy` (L196) but
   **not** `restore_match_rules`. xdg-desktop-portal filters SelectSources
   options against this allow-list before forwarding to the backend `impl`, so
   the key OBS sends (`patches/obs-studio/...` L85) is stripped before COSMIC
   (`patches/xdg-desktop-portal-cosmic/...` L509, L652) ever sees it. The entire
   regex-alias rescue subsystem only "works" in the mock and in COSMIC's
   in-patch unit tests, both of which bypass this filter.

2. **Critical — double-free / heap corruption from `g_auto(GStrv)` over a
   borrowed `^a&s` array.** The patch changes upstream's
   `const char **permissions;` to `g_auto(GStrv) permissions = NULL;` (patch
   L408–409) while keeping `g_variant_lookup(perms, session->app_id, "^a&s",
   &permissions)` (L422). `^a&s` returns a **shallow** array: the container is
   heap-allocated but the strings are *borrowed* pointers into the live
   `g_autoptr(GVariant) perms`. `g_auto(GStrv)` runs `g_strfreev()`, which
   `g_free`s each borrowed string → invalid free + UAF/double-free vs `perms`,
   on the **common restore-token success path**. The codebase's own
   `xdp_get_permissions_sync` (`src/xdp-permissions.c`) pairs the identical
   `^a&s` lookup with `g_autofree char **` precisely to avoid this.

3. **Critical — session use-after-free: `xdp_session_close()` inside a
   non-compensated autolock scope.** The skip/error branch calls
   `xdp_session_close(session, TRUE)` (patch L347–348) under the plain
   `SESSION_AUTOLOCK_UNREF(session)` scope. `xdp_session_from_request` returns
   exactly one owned ref; `xdp_session_close` consumes a ref internally; then the
   autolock cleanup helper unlocks a freed mutex and unrefs a finalized object.
   Every other upstream caller wraps it as
   `SESSION_AUTOLOCK_UNREF(g_object_ref(session))` for exactly this reason
   (verified across ~15 call sites in screen-cast.c / remote-desktop.c /
   input-capture.c). Triggered whenever a restore_token can't be honored and
   policy resolves to skip(1)/error(2).

Deferring #1 means the headline feature stays non-functional; deferring #2/#3
means every restore-token flow can corrupt the heap.

## Out of scope

- Do **not** touch the COSMIC patch, OBS patch, or any rescue *logic* — this
  phase only makes the generic portal forward the key and stop corrupting
  memory. COSMIC's handling of `restore_match_rules` is Phase 02.
- Do **not** redesign the `restore_policy` / `restore_failure` vardict shapes.
- Do **not** change the `source_label` 256-byte limit *mechanism* beyond the
  one decision called out in step 5 (keep the change minimal and documented).
- Do **not** add the mock test for end-to-end key agreement here — that lives in
  Phase 06 (mock). This phase's e2e proof can be a build + a code-level
  assertion that the three key-sets agree (step 4).
- No unrelated upstream-line cleanup inside the patch (keep hunks minimal).

## Plan

1. **Read the patch and the pinned upstream.** Open the patch file. Resolve the
   upstream source it applies to (`nix build .#xdg-desktop-portal --dry-run` or
   inspect `nix/xdg-desktop-portal.nix`) and read the real
   `src/xdp-session-persistence.c`, `src/screen-cast.c`, and
   `src/xdp-permissions.c` for the functions touched, so you fix against the
   actual base, not the diff context.

2. **Fix #1 — add `restore_match_rules` to the allow-list.** In the
   `screen_cast_select_sources_options[]` array (patch ~L189–196), add an entry
   for `restore_match_rules` with the correct GVariant type. OBS sends it as an
   array of vardicts (`aav` — `a a{sv}`), so use the matching
   `G_VARIANT_TYPE("aav")` (confirm against the OBS builder in
   `patches/obs-studio/...` L39–85 and the COSMIC deserializer signature
   `Vec<HashMap<String, OwnedValue>>` at `patches/.../cosmic...` L509). Add a
   `validate_restore_match_rules` validator if the sibling keys have validators,
   mirroring `validate_restore_policy` — at minimum assert the array element
   type and bound the count (see Phase 02 for the matching backend cap).

3. **Fix #2 — the borrowed-array free.** Pick ONE (prefer (b), it preserves
   upstream semantics most precisely):
   - (a) Change the lookup format to a deep copy: `"^as"` instead of `"^a&s"`,
     keeping `g_auto(GStrv) permissions`. `g_strfreev` is then correct.
   - (b) Keep `"^a&s"` and revert the variable to
     `g_autofree const char **permissions = NULL;` (frees only the array, never
     the borrowed strings) — matching `xdp_get_permissions_sync`. Confirm the
     newly-added `xdp_permissions_to_tristate(permissions)` call only *reads* the
     array (the audit verified it does: `g_strv_length`/`g_strjoinv`/`strcmp`,
     no mutation), so the array-only free is sound.

4. **Fix #3 — ref-balance the session close.** Change patch L347–348 to
   `if (IS_SCREEN_CAST_SESSION (session)) xdp_session_close (g_object_ref
   (session), TRUE);`. Verify against the real `xdp_session_close` body that it
   consumes exactly one ref, and that `handle_select_sources` uses the plain
   `SESSION_AUTOLOCK_UNREF(session)` (no pre-existing extra ref) — i.e. confirm
   you are matching the upstream `g_object_ref` idiom, not double-reffing.

5. **Resolve the `source_label` length contract (info-level, decide + document).**
   The base portal hard-rejects `source_label` > 256 bytes as `INVALID_ARGUMENT`
   while libportal/COSMIC truncate. Since the spec text says the label is
   display-only hygiene, prefer **truncate-and-accept** at this layer too (or, if
   you keep the hard limit, leave a one-line comment that 256 is the
   authoritative hard cap and Phase 05 must make libportal reject rather than
   silently truncate). Make the smallest change consistent with the rest of the
   set and note the decision in the commit message.

6. **Add an in-tree key-agreement assertion (cheap e2e proof for #1).** Without
   standing up a full desktop, prove the three key-sets agree: grep that every
   key OBS adds to its SelectSources builder appears in
   `screen_cast_select_sources_options[]` and in the COSMIC options struct.
   Capture this as a comment/check in the patch commit message or a tiny shell
   assertion in the phase notes; the *automated* version lands in Phase 06.

7. **Build and gate.** `nix build .#xdg-desktop-portal` (proves the patch still
   applies and compiles). Then `nix flake check` (runs the `xdp-patch-applies`
   check + clippy). Both green.

## Acceptance criteria

- [ ] `restore_match_rules` appears in `screen_cast_select_sources_options[]`
      with a GVariant type matching OBS's `aav` builder and COSMIC's
      `Vec<HashMap<String, OwnedValue>>` reader.
- [ ] No `g_auto(GStrv)` is paired with a `^a&s` lookup anywhere in the patch;
      the permissions array is freed with the cleanup that matches its ownership
      (either `^as` + `g_auto(GStrv)`, or `^a&s` + `g_autofree const char **`).
- [ ] The skip/error branch calls `xdp_session_close(g_object_ref(session),
      TRUE)` (or equivalently balanced), matching the upstream idiom for closing
      a session held under `SESSION_AUTOLOCK_UNREF`.
- [ ] `nix build .#xdg-desktop-portal` succeeds.
- [ ] `nix flake check` passes (the `xdp-patch-applies` check is green).
- [ ] A documented decision exists for the `source_label` 256-byte contract.
- [ ] The patch's `From:`/`Subject:`/`Signed-off-by:`/SPDX header is normalized
      to the agreed convention (see Phase 08 for the convention; apply it to
      *this* patch here so Phase 08 never re-touches this file).

## Files likely touched

- `patches/xdg-desktop-portal/0001-screencast-add-source_label-and-restore_policy.patch`
  (the only code edit; touches its embedded `src/screen-cast.c` and
  `src/xdp-session-persistence.c` hunks, and the option-array hunk).

## Pitfalls

- **Symptom:** you change `g_auto(GStrv)` → `g_autofree const char **` but leave
  a later `g_strfreev`/`g_clear_pointer` call. **Cause:** half-applied fix.
  **Recovery:** grep the function for every cleanup of `permissions` and ensure
  exactly one array-only free.
- **Symptom:** `nix build .#xdg-desktop-portal` fails with "patch does not
  apply" after editing. **Cause:** your edit changed hunk context line counts or
  the `@@ -a,b +c,d @@` headers are now wrong. **Recovery:** regenerate the hunk
  header offsets, or apply the patch to a checkout, make the edit there, and
  `git diff` a fresh patch. Keep context minimal.
- **Symptom:** wrong GVariant type for `restore_match_rules` → the value is
  filtered out again or `g_variant_lookup` mis-parses. **Cause:** `aav` vs
  `a{sv}` confusion. **Recovery:** match the *exact* type OBS builds; an array
  of vardicts is `aav` where each element is `a{sv}`.
- **Symptom:** double-free still occurs after the session-close fix. **Cause:**
  the function actually held an extra ref upstream that you didn't account for.
  **Recovery:** read the real `handle_select_sources` autolock site; only add
  `g_object_ref` if the scope is the plain `SESSION_AUTOLOCK_UNREF(session)`.

## Risk profile

- A wrong free (R1) ships a heap corruption that crashes xdg-desktop-portal for
  *every* screencast on the common restore path — worse than the status quo.
- An over-eager ref (R2): adding `g_object_ref` where upstream already balanced
  it leaks a session per skip/error call.
- A wrong allow-list type (R3): `restore_match_rules` is still dropped or
  mis-parsed, silently re-breaking the feature while tests pass.
- Patch-apply drift (R4): minor context changes break `nix flake check` for
  consumers on a different nixpkgs pin.

## Strategy

Three independent fixes in one file → three commits on a branch, each
individually revertable:
1. `restore_match_rules` allow-list (+ validator).
2. permissions borrowed-array free.
3. session-close ref balance.
Run `nix build .#xdg-desktop-portal` after each commit. Revert cost per commit
is one `git revert`; the patch file is the only artifact.

## Rollback drill

Before starting, confirm you can rebuild the unmodified package and that the
check passes: `git stash && nix build .#xdg-desktop-portal && nix flake check`
(SLA: < 5 min on a warm store). If any commit breaks the build, `git checkout --
patches/xdg-desktop-portal/` restores the pre-phase patch instantly.

## Failure modes and recoveries

- **F1 — heap corruption survives the fix.** Symptom: ASan/valgrind or a crash
  on restore. Cause: cleanup attribute and lookup format still mismatched.
  Recovery: enforce the invariant "`&` in the format ⇒ array-only free; no `&`
  ⇒ deep-copy free" and pick one consistently.
- **F2 — feature still dead.** Symptom: COSMIC logs show no
  `restore_match_rules`. Cause: type mismatch or the key added to the wrong
  options array (CreateSession vs SelectSources). Recovery: confirm it is the
  `screen_cast_select_sources_options[]` array and the `aav` type.
- **F3 — unrelated package patch won't apply.** Symptom: a non-XDP patch-apply
  check fails while repairing this file. Recovery: isolate the affected package
  before changing unrelated patches, and widen context only as needed.

## Reference

- Originating diagnosis: multi-agent audit, findings #1 (allow-list), #2
  (GStrv double-free), #3 (session UAF) — all confirmed against upstream
  xdg-desktop-portal 1.20.x.
- Plan README: [./README.md](./README.md) (Gate A: Phase 02's rescue e2e
  depends on this phase's fix #1).
- Sibling: [Phase 02](./02-fix-cosmic-rescue.md) consumes the now-forwarded
  `restore_match_rules`.
