# Phase 02 — Fix COSMIC rescue: deadlock, cross-app confidentiality, regex bounds

> **Recommended Codex model: GPT 5.5 max** (`reasoning_effort: xhigh`)
>
> Frontier complexity: a cross-thread AB/BA lock-order inversion (correct fix
> requires reasoning about which mutex is held across which `.await` and on
> which thread) *and* a confidentiality boundary (an `any_app` regex alias can
> silently hand an unrelated app's window to a recorder). Mediocre work here
> ships either a compositor deadlock that hangs every screencast session, or a
> silent privacy break — both worse than visible bugs. The role is a careful
> leaf executor on one Rust patch file, but the security + concurrency stakes
> justify the top tier and a full pre-mortem. A smaller model is likely to
> "fix" the deadlock by reordering one site without unifying the global lock
> order, or to gate `any_app` with a regex-specificity heuristic that is
> trivially defeated.

## Working tree

`/data/nvme0/can/Projects/cosmic-obs`. All edits are to
`patches/xdg-desktop-portal-cosmic/0001-screencast-implement-restore_policy-with-rescue.patch`
(plus possibly `nix/xdg-desktop-portal-cosmic.nix` for the Cargo.lock issue).
Verification builds via `nix build .#xdg-desktop-portal-cosmic` and the
`cosmic-patch-applies` check. No other phase touches these files.

**Dependency:** the *code* edits here are independent of all other phases. Only
the **end-to-end** rescue smoke test (OBS → xdg-desktop-portal → COSMIC)
requires [Phase 01](./01-repair-xdp-generic-patch.md)'s `restore_match_rules`
forwarding fix to have landed (Gate A) — until then COSMIC never receives the
rules on the real path. Unit-level acceptance (deadlock, any_app gating, regex
bounds) is fully checkable here without Phase 01.

## Goal

The COSMIC rescue path is **deadlock-free**, **cannot silently share an
unrelated application's window**, and **bounds caller-supplied regex** in both
compile cost and cardinality. The patch builds (`nix build
.#xdg-desktop-portal-cosmic`), the misleading lock-order comment is corrected to
match reality, and the COSMIC package's lockfile reflects the new `regex`
dependency so the Nix build is reproducible.

## Why this matters now

Audit findings, all in this one patch file (these are *latent* today because
Phase 01 strips `restore_match_rules` before COSMIC sees it — they go live the
moment Phase 01 lands, so they must ship together):

1. **High — AB/BA lock-order inversion → portal deadlock.** `check_or_rescue`
   acquires `toplevels` then `pending_rescues` (T→R, both held), and correctly
   uses the snapshot variant `to_capture_sources_with(&toplevel_list, …)` so it
   does not re-lock. But `update_output_toplevels`'s drain loop holds
   `pending_rescues` (the `let mut rescues = …pending_rescues.lock()` guard,
   alive through the loop) and then calls the *non-snapshot*
   `rescue.persisted.to_capture_sources(&self.wayland_helper)`, whose body calls
   `wayland_helper.toplevels()` → `self.inner.toplevels.lock()` (R→T). The two
   run on different threads (the Wayland `blocking_dispatch` thread vs the tokio
   Start handler), so the opposite orders can interleave and deadlock the
   compositor's portal with no recovery short of restart. The inline comment
   `// Lock order: toplevels → pending_rescues (matches update_output_toplevels)`
   is **false** — `update_output_toplevels` does the reverse.

2. **High — `any_app` regex alias silently shares an unrelated window
   (confidentiality break).** `title_regex_rule_matches` (patch ~L329–357) skips
   the app-id guard entirely for `RestoreMatchScope::AnyApp` (the
   `if matches!(rule.scope, RestoreMatchScope::SameApp) { … }` at L335 is the
   only gate), leaving `RegexBuilder::new(&rule.pattern).case_insensitive(true)
   .build()` + `re.is_match(candidate_title)` (L344) as the sole check.
   `match_title_regex_rules` returns the *first* matching toplevel. On the silent
   rescue path (`source_unavailable_action` 1|2 → `check_or_rescue` →
   `to_capture_sources_with` mode-2, and the background `update_output_toplevels`
   re-check), an alias like `pattern="."`/`".*"` with `any_app` matches *any*
   window and hands its PipeWire stream to the recorder with **no prompt and no
   re-consent**. Caller-supplied rules persist into the v3 token, so an app that
   once got one capture consent can later redirect capture to a password
   manager / banking / chat window.

3. **Medium — rescue can fire on the wrong window.** A pending rescue stores
   only `PersistedCaptureSources` (app_id/title/regex); `update_output_toplevels`
   re-runs matching whenever *any* toplevel appears, and `RESCUE_MODE_APP` grabs
   the first window with a matching app_id. It doesn't record which specific
   identifier it is waiting for, so an unrelated window of the same app can be
   captured.

4. **Medium/perf + Low/security — unbounded caller regex.**
   `title_regex_rule_matches` calls `RegexBuilder::new(&rule.pattern).build()` on
   *every* invocation inside `for rule { for info { … } }` (N×M recompiles), with
   no `.size_limit()`/`.dfa_size_limit()` and no cap on rule count
   (`parse_restore_match_rules` pushes every accepted rule unbounded; the v3
   `TryFrom` likewise). Compile-bomb / memory-DoS surface plus wasted work.

5. **Nix-build — `regex` dep added without a lockfile update.** The patch adds
   `regex = "1.12.2"` to the COSMIC `Cargo.toml` but does not update
   `Cargo.lock`; the `buildRustPackage` build currently relies on `regex` being
   an incidental transitive dep, which is fragile.

## Out of scope

- Do **not** touch the generic xdg-desktop-portal patch (Phase 01 owns
  forwarding) or the OBS patch (Phase 03 owns the rule-authoring UI).
- Do **not** split the COSMIC patch into generic-vs-backend (a real upstreaming
  refactor) — that is noted as future work in Phase 08, not done here.
- Do **not** change the wire shape of `restore_match_rules` or token v3.
- Do **not** add mock tests here — the mock confidentiality test is Phase 06.
  This phase's tests are COSMIC in-patch unit tests.

## Plan

1. **Read against the pinned source.** Resolve the COSMIC source the patch
   applies to (`nix/xdg-desktop-portal-cosmic.nix` through the refreshed
   `nixos-unstable` flake lock) and read the real
   `src/wayland/mod.rs` and `src/screencast.rs` for `update_output_toplevels`,
   `check_or_rescue`, `to_capture_sources`, `to_capture_sources_with`, and
   `WaylandHelper::toplevels`.

2. **Fix #1 — unify lock order.** In `update_output_toplevels`'s rescue drain
   loop, snapshot the toplevel list **once before** locking `pending_rescues`,
   then match each pending rescue with the existing
   `to_capture_sources_with(&snapshot, &self.wayland_helper)` so `toplevels` is
   never re-acquired while `pending_rescues` is held. This makes both paths T→R.
   Correct the `// Lock order:` comment to state the true, now-consistent order.
   Audit both functions for any guard held across `.await`.

3. **Fix #2 — close the `any_app` silent-share hole.** On the unattended rescue
   path, an `any_app` alias must **not** auto-select a new, different-app window
   without user confirmation. Implement one of (prefer (a)):
   - (a) When a rescue match would be satisfied only by an `any_app` rule (or by
     a window whose `app_id` differs from the saved `app_id`), fall back to the
     **picker / prompt** instead of auto-sharing. `same_app` matches and exact
     identifier matches may still auto-resolve.
   - (b) Restrict regex aliases on the unattended rescue path to `same_app`, and
     only honor `any_app` when the user is actively re-consenting in the picker.
   Do **not** rely on regex-specificity heuristics (`.`/`.*`/`^`/`$`/any
   unanchored substring all defeat a naive check).

4. **Fix #3 — make rescue identifier-specific.** Record on the pending rescue
   which identifier/title+app_id it is waiting for, and prefer exact-identifier
   and exact `title+app_id` matches before the first-app-id-wins fallback, so a
   reappearing unrelated same-app window isn't grabbed.

5. **Fix #4 — bound and precompile regex.** Compile each rule's `Regex` once
   (build a `Vec<Regex>` when constructing `PersistedCaptureSources` / parsing
   rules) and reuse it across candidates. Set explicit `.size_limit(...)` and
   `.dfa_size_limit(...)` on the `RegexBuilder`. Cap the number of accepted rules
   in `parse_restore_match_rules` and the v3 `TryFrom`, and cap per-pattern
   length; log dropped rules with a warning (matching the documented "malformed
   external rules are ignored with a warning" behavior). Use the **same cap
   value** Phase 01 uses in its `validate_restore_match_rules` validator.

6. **Fix #5 — lockfile.** Ship the `Cargo.lock` change alongside the
   `Cargo.toml` `regex` edit inside the same patch, or switch
   `nix/xdg-desktop-portal-cosmic.nix` to use `cargoPatches` / regenerate
   `cargoHash` so the dependency is explicit and reproducible.

7. **Tests + build.** Extend the in-patch COSMIC unit tests: a test that an
   `any_app` `"."` alias on the unattended path does **not** return an arbitrary
   unrelated window (asserts the picker-fallback / same_app restriction); a test
   that the lock-order fix path uses the snapshot variant. Then
   `nix build .#xdg-desktop-portal-cosmic` and `nix flake check` (including the
   `cosmic-patch-applies` check).

## Acceptance criteria

- [ ] `update_output_toplevels` snapshots toplevels before locking
      `pending_rescues` and uses `to_capture_sources_with` in the drain loop;
      `toplevels` is never re-locked while `pending_rescues` is held.
- [ ] The `// Lock order:` comment matches the actual, globally-consistent order.
- [ ] On the unattended rescue path, an `any_app` alias (or a match whose
      `app_id` differs from the saved one) routes to the picker / re-consent
      instead of auto-sharing — covered by a new COSMIC unit test using a `"."`
      pattern + unrelated toplevels.
- [ ] Each regex rule is compiled at most once per parse (not N×M), and
      `RegexBuilder` sets `size_limit` and `dfa_size_limit`.
- [ ] Rule count and pattern length are capped (same cap as Phase 01's
      validator); over-cap rules are dropped with a logged warning, leaving
      valid rules intact.
- [ ] `Cargo.lock` (or `cargoHash`/`cargoPatches`) reflects the `regex`
      dependency; `nix build .#xdg-desktop-portal-cosmic` succeeds reproducibly.
- [ ] `nix flake check` passes, including `cosmic-patch-applies`.
- [ ] The patch's `From:`/`Subject:`/`Signed-off-by:`/SPDX header is normalized
      to the agreed convention; the Subject accurately describes the content
      (the audit flagged the current Subject as misdescribing scope). Apply here
      so Phase 08 never re-touches this file.

## Files likely touched

- `patches/xdg-desktop-portal-cosmic/0001-screencast-implement-restore_policy-with-rescue.patch`
  (embedded `src/wayland/mod.rs`, `src/screencast.rs`, `Cargo.toml`, and the
  in-patch test hunks).
- `nix/xdg-desktop-portal-cosmic.nix` (only if you handle the lockfile via
  `cargoPatches`/`cargoHash` rather than inside the patch).

## Pitfalls

- **Symptom:** deadlock persists. **Cause:** you reordered one site but a third
  path still nests the other way, or a guard is held across `.await`. **Recovery:**
  enumerate every acquisition of both mutexes and confirm a single global order.
- **Symptom:** legitimate same-app rescue now prompts unnecessarily. **Cause:**
  the picker-fallback gate is too broad. **Recovery:** gate only on `any_app` /
  differing-`app_id`, not on all regex matches.
- **Symptom:** `nix build .#xdg-desktop-portal-cosmic` fails on `cargoHash`
  mismatch. **Cause:** lockfile/deps changed. **Recovery:** update the hash to
  the value Nix reports, or use `cargoPatches` to inject the lockfile delta.
- **Symptom:** `regex` compile-limit rejects a previously-working pattern.
  **Cause:** `size_limit` too small. **Recovery:** pick a limit comfortably above
  realistic title patterns; document it.

## Risk profile

- R1 (deadlock): a wrong fix hangs the compositor portal — no screencast,
  requires restart.
- R2 (confidentiality): a defeatable `any_app` gate silently leaks an unrelated
  window's contents to a recorder.
- R3 (over-strict): an over-broad picker fallback regresses the intended UX
  (legit same-app rescue should stay silent).
- R4 (build): a missing lockfile update makes the Nix build non-reproducible.

## Strategy

Five independent fixes → five commits, each `nix build
.#xdg-desktop-portal-cosmic`-gated and individually revertable. Land the
lock-order fix and the any_app gate first (highest stakes); regex bounds and
lockfile after.

## Rollback drill

`git stash && nix build .#xdg-desktop-portal-cosmic && nix flake check` to
confirm the baseline builds (SLA: < 10 min cold, COSMIC is a large Rust build).
`git checkout -- patches/xdg-desktop-portal-cosmic/ nix/xdg-desktop-portal-cosmic.nix`
restores the pre-phase state.

## Failure modes and recoveries

- **F1 — interleaving deadlock under load.** Symptom: portal hangs when a
  restore and a toplevel event race. Cause: residual lock-order inversion.
  Recovery: prove single global order; consider a single combined lock if the
  two are always taken together.
- **F2 — privacy test passes but real desktop still shares.** Symptom: manual
  checklist step 6 (scope behavior) shares across app-ids. Cause: a second
  rescue entry point (`update_output_toplevels` background re-check) not gated.
  Recovery: apply the any_app gate at *every* unattended match site, not just
  `check_or_rescue`.
- **F3 — ReDoS still reachable.** Symptom: high CPU on a crafted pattern. Cause:
  Rust `regex` is linear but a huge pattern still costs compile time/memory.
  Recovery: `size_limit` + length cap; reject (don't truncate) over-limit
  patterns with a warning.

## Reference

- Originating diagnosis: multi-agent audit, findings: AB/BA lock inversion
  (verified against the pinned COSMIC source, both finders reconciled to a
  cross-thread mechanism), `any_app` cross-app share, wrong-window rescue,
  unbounded/recompiled regex, missing Cargo.lock.
- [docs/restore-match-rules.md](../../restore-match-rules.md) — matching order
  and scope semantics.
- [docs/manual-restore-alias-checklist.md](../../manual-restore-alias-checklist.md)
  — steps 4–7 are the desktop smoke for this phase.
- Plan README [Gate A](./README.md): end-to-end rescue verify needs
  [Phase 01](./01-repair-xdp-generic-patch.md).
