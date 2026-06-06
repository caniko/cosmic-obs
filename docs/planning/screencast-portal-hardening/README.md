# Plan: ScreenCast Portal Hardening

> **Recommended Codex model for plan-set orchestration: GPT 5.5 high**
>
> Coordinating this set requires holding the cross-layer ScreenCast v6 contract
> in mind (OBS → xdg-desktop-portal → COSMIC/XDPH/libportal) while dispatching
> eight mostly-parallel phases with subtle verification gates. That is complex
> orchestration with real design judgement (where each fix lands, which gate
> blocks which), but not frontier novelty — the phase files carry the hard
> detail. A smaller model risks mis-sequencing the verification gates (e.g.
> dispatching the mock-fidelity phase before the contract decisions it depends
> on) or under-weighting the two `max`-tier memory-safety phases.

## Scope and current state

This plan repairs the prototype ScreenCast v6 RFC stack (`source_label` +
`restore_policy` + `restore_match_rules` rescue) after a multi-agent audit
produced **88 adversarially-verified findings** (3 critical, 5 high, plus
mediums and hardening/test/doc gaps). The repo is a competent prototype with a
coherent protocol design and a solid D-Bus mock, but it is **not correct
end-to-end and not upstream-ready**.

The work is five upstream-bound patches (`patches/<project>/`), a Rust mock
crate (`screencast-portal-mock/`), a Nix flake, and docs. Each patch file is a
natural rollback boundary, so the patch phases are file-disjoint and run in
parallel.

**The single most important fact:** the regex-alias rescue feature is *dead on
the real desktop path* today — the base portal strips `restore_match_rules`
before it reaches COSMIC (Phase 01, fix #1). Several COSMIC rescue bugs
(Phase 02) are therefore latent and become live the moment Phase 01 lands.
Phase 01 and Phase 02 must both ship before the rescue feature is enabled for
real use.

## Global constraints (apply to every phase)

- Every code edit is to a `.patch` file (a unified diff against pinned
  upstream) **or** to the Rust mock / Nix / docs. After editing any `.patch`,
  rebuild the affected package to prove the patch still applies:
  `nix build .#<pkg>` (pkgs: `xdg-desktop-portal`, `xdg-desktop-portal-cosmic`,
  `obs-studio`, `xdg-desktop-portal-hyprland`, `libportal`).
- Preserve **v5 backward compatibility** and **default-preserving** behavior:
  absent options ⇒ legacy behavior; clients that never send the new keys must
  see no change.
- Patch bases are pinned through the flake lock: `nixpkgs` tracks the refreshed
  `nixos-unstable` input, and the package/check gates build against that locked
  baseline.
- `nix flake check` is the patch-apply + lint gate. It must pass at the end of
  every phase that touches a patch or the flake.
- Do **not** "improve" unrelated upstream lines inside a patch — minimal,
  well-anchored hunks only (an audit finding flagged an unrelated trailing-
  newline deletion as patch-apply fragility).

## Phase table

| Phase | File | Depends on | Touches | Can parallel with | Blocking? |
|---|---|---|---|---|---|
| 01 | [01-repair-xdp-generic-patch.md](./01-repair-xdp-generic-patch.md) | — | `patches/xdg-desktop-portal/`, `nix/xdg-desktop-portal.nix` | 03,04,05,07,08 | **Yes** — unblocks rescue e2e + ships 2 critical UAF fixes |
| 02 | [02-fix-cosmic-rescue.md](./02-fix-cosmic-rescue.md) | 01 | `patches/xdg-desktop-portal-cosmic/`, `nix/xdg-desktop-portal-cosmic.nix` | 03,04,05,07,08 | **Yes** — deadlock + confidentiality |
| 03 | [03-fix-obs-defaults.md](./03-fix-obs-defaults.md) | — | `patches/obs-studio/`, `nix/obs-studio.nix` | 01,02,04,05,07,08 | High — RFC's core promise |
| 04 | [04-sanitize-xdph-source-label.md](./04-sanitize-xdph-source-label.md) | — | `patches/xdg-desktop-portal-hyprland/`, `nix/xdph.nix` | 01,02,03,05,06,07,08 | Medium |
| 05 | [05-complete-libportal-api.md](./05-complete-libportal-api.md) | — | `patches/libportal/`, `nix/libportal.nix` | 01,02,03,04,06,07,08 | Medium |
| 06 | [06-mock-fidelity-coverage.md](./06-mock-fidelity-coverage.md) | 02, 03 | `screencast-portal-mock/src/`, `screencast-portal-mock/tests/` | 04,05,07,08 | Medium |
| 07 | [07-ci-and-build-coherence.md](./07-ci-and-build-coherence.md) | — | `flake.nix`, `.woodpecker.yml`, `.cargo/config.toml`, `screencast-portal-mock/Cargo.toml`, `.gitignore` | 01,02,03,04,05,08 | Medium |
| 08 | [08-docs-and-grooming.md](./08-docs-and-grooming.md) | — | `DESIGN.md`, `docs/`, `README.md`, `screencast-portal-mock/README.md` | 01,02,03,04,05,06,07 | Low |

All phases edit disjoint files, so the dependencies below are **logical /
verification gates**, not file conflicts. Patch-header grooming (authorship,
`Signed-off-by`, SPDX, Subject) is folded into each patch phase (01–05) so no
phase has to re-touch another phase's patch file.

## Parallelism layer (execution waves)

**Wave 0 — fan out immediately from the current tree (7 phases, file-disjoint):**
`01, 02, 03, 04, 05, 07, 08`.
- Code edits in all seven do not collide. Open up to seven fresh Codex sessions.
- **Gate A:** Phase 02's *end-to-end* rescue smoke test (OBS → xdp → COSMIC)
  requires Phase 01's `restore_match_rules` forwarding fix to have landed. The
  COSMIC *code* edits proceed in parallel; only 02's final e2e acceptance item
  is blocked on 01. Phase 02's unit-level acceptance (deadlock, any_app gating)
  is checkable without 01.
- **Gate B:** Phase 07 adds a CI check that runs the integration suite; the
  "integration suite is gated in CI" criterion is finalized after Phase 06 adds
  its new tests. 07's flake/toolchain/dep work is independent and lands in
  Wave 0.

**Wave 1 — after Phase 02 and Phase 03 land their contract decisions:**
`06`.
- The mock must model the *corrected* contract: where `source_unavailable`
  surfaces (decided in 02), and the OBS default-Skip behavior (decided in 03).
  Dispatching 06 before those decisions risks encoding the wrong contract again.

**Wave 2 — whole-set verification (single session, after all phases pass):**
- Rebuild all five patched packages + `nix flake check` (now including the new
  dbus integration check from 07 and the new mock tests from 06).
- Walk `docs/manual-restore-alias-checklist.md` on a real patched COSMIC + OBS
  build (the audit notes the mock cannot prove desktop-level behavior).
- Re-confirm Gate A (rescue reaches COSMIC) and Gate B (CI gates everything).
- Prompt `verify` against this plan dir to audit acceptance criteria.

## Whole-set acceptance criteria

- [ ] `nix flake check` passes, including all `*-patch-applies` checks,
      `mock-clippy`, and the new dbus-backed integration check (Phase 07).
- [ ] All five patched packages build: `nix build .#xdg-desktop-portal
      .#xdg-desktop-portal-cosmic .#obs-studio .#xdg-desktop-portal-hyprland
      .#libportal`.
- [ ] The two xdg-desktop-portal memory-safety bugs are fixed: no
      `g_strfreev` over a `^a&s` borrowed array, and `xdp_session_close` is
      ref-balanced on the skip/error path (Phase 01).
- [ ] `restore_match_rules` reaches COSMIC end-to-end — proven by a test/trace
      asserting OBS-sent ⊆ xdp-allow-list ⊆ COSMIC-read for every v6 key
      (Phase 01 + 06).
- [ ] An upgraded OBS source carrying a stale `restore_token` and the default
      policy resolves to **Skip** and does **not** pop a picker (Phase 03).
- [ ] An `any_app` regex alias cannot silently hand an unrelated app's window
      to a recorder on the unattended rescue path (Phase 02).
- [ ] No ABBA lock-order inversion between `toplevels` and `pending_rescues`;
      the misleading lock-order comment is corrected (Phase 02).
- [ ] The mock models `source_unavailable` at the **Start** phase (not
      SelectSources), parses + asserts `restore_match_rules`, and has a test for
      the cross-app confidentiality case (Phase 06).
- [ ] `DESIGN.md` documents `restore_match_rules`, rescue modes, and COSMIC
      token versions v1/v2/v3; `README.md` scenario examples compile (Phase 08).
- [ ] A clean-clone `cargo` build in the working tree is not broken by a
      nightly-only `.cargo/config.toml` while `rust-toolchain.toml` pins stable
      (Phase 07).

## Reference

- Originating diagnosis: the multi-agent audit (88 verified findings; 116
  agents; verifiers cross-checked against upstream xdg-desktop-portal 1.20.x and
  the pinned COSMIC source from the Nix store). Each phase file embeds its own
  findings inline so it is standalone.
- [DESIGN.md](../../../DESIGN.md) — v6 protocol design (stale re the rescue
  subsystem; Phase 08 refreshes it).
- [docs/restore-match-rules.md](../../restore-match-rules.md),
  [docs/manual-restore-alias-checklist.md](../../manual-restore-alias-checklist.md).
- Run the phases yourself in fresh Codex sessions; prompt **`verify`** when done
  to audit acceptance criteria against the repo.
