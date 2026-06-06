# Phase 06 — Make the mock faithful and cover the rescue/confidentiality surface

> **Recommended Codex model: GPT 5.5 high** (`reasoning_effort: high`)
>
> Complex. The mock is the project's primary protocol validator, so it must
> model the *corrected* two-layer contract precisely: where `source_unavailable`
> surfaces (Start, not SelectSources), how `restore_match_rules` is parsed and
> asserted, and a regression test for the cross-app confidentiality case. This
> needs protocol reasoning across the real backend behavior decided in Phases 02
> and 03, plus Rust async test design — a wrong model gives false confidence,
> which is worse than no test. That is a genuine design call (`high`), but it is
> a single Rust crate with a clear target contract, not frontier novelty
> (`max`). A `medium` model risks re-encoding the current SelectSources-time
> `source_unavailable` behavior the audit already flagged as wrong.

## Working tree

`/data/nvme0/can/Projects/cosmic-obs`. Edits are to `screencast-portal-mock/src/`
and `screencast-portal-mock/tests/`. **Dispatch this phase after
[Phase 02](./02-fix-cosmic-rescue.md) and [Phase 03](./03-fix-obs-defaults.md)
have landed their contract decisions** — the mock must encode the corrected
behavior, not the current one. Tests run via `cargo nextest run` (or `cargo
test`) in a session with a D-Bus session bus available (the devShell provides
`dbus`); the Nix integration gate is added in Phase 07.

## Goal

The mock faithfully models the v6 contract: `source_unavailable` is surfaced at
the **Start** phase (matching real COSMIC), `restore_match_rules` is a
first-class parsed + asserted SelectSources option, the OBS helper models the
patched client's default-Skip + label-from-name behavior, dead modeling fields
are wired or removed, and there is automated coverage of the `any_app`
cross-app confidentiality case. `cargo nextest run` passes.

## Why this matters now

Audit findings — the mock currently gives false confidence about the riskiest
features:

1. **High — `source_unavailable` modeled in the wrong layer.** The mock resolves
   any injected reason (including `SourceUnavailable`) at **SelectSources** via
   `policy.action_for` (`src/portal.rs:218–256`), firing `response=1/2` +
   `restore_failure` + a `Session::Closed` signal there. The real stack surfaces
   `source_unavailable` only at the **Start** Response (COSMIC returns
   `PortalResponse::Cancelled` for skip — which carries *no* `restore_failure` —
   or `OtherWithResults{restore_failure}` for error). A client validated against
   the mock handles it in the wrong place and shape.
2. **Medium/test-gap — `restore_match_rules` never parsed or asserted.** The
   mock parses `source_label`/`restore_policy`/etc. but has no handling for the
   `restore_match_rules` (`aav`) option that the COSMIC patch and OBS define —
   the entire newer rescue subsystem is untested.
3. **Medium/test-gap — `obs.rs` does not model the patched OBS client.** It only
   launches the real `obs` binary (documented unavailable). Nothing models the
   default-Skip action or `source_label`-from-source-name behavior.
4. **Medium/test-gap — dead modeling fields.** `SourceDef::expected_label` /
   `restore_valid` and the `with_label` / `invalid` builders in `src/records.rs`
   are never read; per-source label / stale-token modeling is unwired.
5. **Test-gap — zero coverage of the confidentiality surface** (any_app `.`
   alias selecting an arbitrary window) and no `restore_match_rules` / token
   v1-v2-v3 compat scenario.

## Out of scope

- Do **not** modify the patches (Phases 01–05 own those). This phase changes only
  the mock crate.
- Do **not** implement the full COSMIC window matcher in the mock — model the
  *protocol-observable* contract (where responses surface, what fields they
  carry), not COSMIC's internal rescue algorithm. The confidentiality test
  asserts the protocol-level outcome (no arbitrary window shared without prompt),
  consistent with [Phase 02](./02-fix-cosmic-rescue.md)'s gate.
- Do **not** add a Nix check here — that is Phase 07.

## Plan

1. **Re-layer `source_unavailable`.** Restrict the SelectSources policy path
   (`src/portal.rs`) to the three generic-portal reasons (`token_not_found`,
   `permission_revoked`, `token_consumed`), and route `source_unavailable`
   through the `on_start` path to mirror COSMIC's `Cancelled` (skip, no
   `restore_failure`) vs `OtherWithResults{restore_failure}` (error) split.
   Update the affected tests (`tests/test_restore_policy.rs` cases that currently
   assert a SelectSources Response for `source_unavailable`).
2. **Parse + assert `restore_match_rules`.** Add a `restore_match_rules` field
   to the `Call` record and parse the `aav` vardict array in `select_sources`,
   with an assertion helper mirroring `source_label`/`restore_policy`. Match the
   key name/type Phase 01 added to the allow-list and Phase 02 reads.
3. **Model the patched OBS client.** Either rename `obs.rs` to make clear it is
   an integration-only launcher (e.g. `obs_launcher.rs`) **and** add an
   in-process `ObsClientModel` that builds SelectSources options exactly as the
   patched client does (default `default_action = Skip`, `source_label` from the
   source name, `restore_match_rules` from the alias multiline string per
   [docs/restore-match-rules.md](../../restore-match-rules.md)), reflecting
   [Phase 03](./03-fix-obs-defaults.md)'s corrected defaults.
4. **Wire or remove dead fields.** Either consume `SourceDef::expected_label` /
   `restore_valid` in a scenario (emit a per-source label property; fail restore
   where `restore_valid == false`) with tests that exercise `with_label` /
   `invalid`, or delete the dead fields/builders. Prefer wiring — it closes a
   real coverage gap.
5. **Add confidentiality + compat scenarios.** Add a scenario/test asserting that
   an `any_app` broad-alias rescue does not yield an arbitrary unrelated window
   without a prompt (protocol-level), and a scenario covering token v1/v2/v3
   deserialization expectations (empty alias lists for v1/v2).
6. **Add the end-to-end key-agreement check (Phase 01 step 6's automated form).**
   A test (or build script assertion) proving every v6 key the OBS model sends is
   in the mock's accepted set — the mock's analogue of "OBS-sent ⊆ allow-list ⊆
   backend-read".
7. **Run.** `cargo nextest run` (or `cargo test`) under a session bus; ensure
   `cargo clippy -- -D warnings` stays clean (so the new code doesn't trip the
   `mock-clippy` check).

## Acceptance criteria

- [ ] `source_unavailable` is emitted only via the Start path; the skip variant
      carries no `restore_failure` and the error variant carries
      `restore_failure{reason:"source_unavailable", token_invalid:false}`. The
      previously-wrong SelectSources-time tests are updated to assert the Start
      behavior.
- [ ] The mock parses `restore_match_rules` and has an assertion helper for it;
      at least one test asserts the recorded rules match what the OBS model sent.
- [ ] An in-process OBS client model exists and builds SelectSources options
      with default Skip + label-from-name (or `obs.rs` is clearly renamed to an
      integration-only launcher and the model added separately).
- [ ] `SourceDef::expected_label` / `restore_valid` / `with_label` / `invalid`
      are either exercised by a test or removed (no dead `pub` helpers).
- [ ] A test asserts the `any_app` broad-alias case does not share an arbitrary
      window without a prompt at the protocol level.
- [ ] An automated key-agreement test proves OBS-sent v6 keys ⊆ mock-accepted
      keys.
- [ ] `cargo nextest run` passes; `cargo clippy -- -D warnings` is clean.

## Files likely touched

- `screencast-portal-mock/src/portal.rs` (re-layer `source_unavailable`, parse
  `restore_match_rules`).
- `screencast-portal-mock/src/records.rs`, `src/scenario.rs` (wire/remove dead
  fields; new scenarios).
- `screencast-portal-mock/src/obs.rs` (→ `obs_launcher.rs` + `ObsClientModel`).
- `screencast-portal-mock/tests/*.rs` (update `test_restore_policy.rs`; add
  confidentiality / restore_match_rules / key-agreement tests).

## Pitfalls

- **Symptom:** tests hang. **Cause:** real D-Bus dependency without a session
  bus, or a missing `Request::Response` emission after re-layering. **Recovery:**
  run under `dbus-run-session`; ensure the Start path emits the response signal.
- **Symptom:** the new model drifts from the real patches. **Cause:** modeling
  COSMIC internals instead of the protocol contract. **Recovery:** assert only
  protocol-observable outcomes (response code, results dict, which phase).
- **Symptom:** `mock-clippy` fails after adding code. **Cause:** new `pub` dead
  helpers or unused imports. **Recovery:** wire everything you add, or `#[cfg]`
  /remove it.
- **Symptom:** re-layering breaks the `lifecycle_after_fail` / `concurrent`
  tests. **Cause:** session-close now happens at Start, not SelectSources.
  **Recovery:** update those assertions to the corrected lifecycle.

## Reference

- Originating diagnosis: multi-agent audit, findings: source_unavailable
  layer-fusion (#7), restore_match_rules untested, obs.rs non-modeling, dead
  `SourceDef` fields, zero confidentiality coverage.
- [Phase 02](./02-fix-cosmic-rescue.md) (where `source_unavailable` and the
  any_app gate are decided) and [Phase 03](./03-fix-obs-defaults.md) (OBS
  default Skip) — this phase encodes those decisions.
- [docs/restore-match-rules.md](../../restore-match-rules.md) — the alias
  multiline grammar the OBS model must reproduce.
