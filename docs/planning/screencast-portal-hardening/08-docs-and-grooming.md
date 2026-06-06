# Phase 08 — Refresh docs and define the patch-grooming convention

> **Recommended Codex model: GPT 5.5 low** (`reasoning_effort: low`)
>
> Mostly mechanical documentation and convention-setting: refresh `DESIGN.md` to
> cover the shipped rescue subsystem, fix README scenario examples so they match
> the real types, and define the patch-header convention (authorship,
> `Signed-off-by`, SPDX, Subject) that Phases 01–05 apply to their own patch
> files. The hard technical content already exists in the audit and the phase
> docs; this phase transcribes and organizes it. `low` is right — reserve effort
> for the phases doing the actual fixes. Bump to `medium` only if accurately
> summarizing the token v1/v2/v3 evolution requires reading the COSMIC patch in
> depth.

## Working tree

`/data/nvme0/can/Projects/cosmic-obs`. Edits are to `DESIGN.md`, files under
`docs/`, the top-level `README.md` (if present) and
`screencast-portal-mock/README.md`. This phase does **not** edit any `.patch`
file — the patch-header grooming is *applied by* Phases 01–05 to their own
files; this phase only **defines the convention** they follow. No file conflict
with any other phase.

## Goal

The docs match the shipped code: `DESIGN.md` documents the
`restore_match_rules` / regex-alias rescue subsystem and COSMIC token versions
v1/v2/v3; the mock `README.md` scenario examples compile against the real
field-bearing types; and a single written patch-grooming convention exists for
Phases 01–05 to apply (consistent authorship, `Signed-off-by`, SPDX, accurate
Subjects), supporting the stated upstreaming goal.

## Why this matters now

Audit findings:

1. **Medium/docs — `DESIGN.md` is stale.** It is labeled "consolidated design
   note as of 2026-04-23" and claims to fold in the patch headers and mock
   README, but it contains **zero** mention of `restore_match_rules`, regex title
   aliases, rescue modes (exact/app/title), or COSMIC token versions v1/v2/v3 —
   the bulk of the most recent and riskiest shipped work (confidentiality break,
   unbounded regex). Reviewers relying on it misjudge scope and risk.
2. **Low/docs — README scenario examples don't match the types.** The mock
   `README.md` presents scenarios as drop-in unit values
   (`RestoreTokenValid`, `RestoreTokenFails`, `MultiSource`, `OldPortal`,
   `SlowResponse`) but the real types require fields (e.g. `RestoreTokenValid {
   token, … }`), so the examples don't compile.
3. **Upstream-readiness — inconsistent patch metadata.** The five patches carry
   inconsistent authorship (`agent <agent@local>` vs a real identity), no
   `Signed-off-by`, no SPDX on new files, and Subject lines that misdescribe
   content (the COSMIC patch Subject omits restore_policy/source_label). These
   would bounce in upstream review, undermining DESIGN's stated primary path.

## Out of scope

- Do **not** edit any `.patch` file's *content* — Phases 01–05 own those, and
  they apply the grooming convention to their own headers as part of their work.
- Do **not** split the COSMIC patch into generic-vs-backend here — record it as a
  recommended **future** upstreaming step (per DESIGN's "minimal upstream path"),
  not an action in this plan.
- Do **not** invent new design — document what the code actually does (read the
  patches/mock to confirm).

## Plan

1. **Define the patch-grooming convention** (a short section in `DESIGN.md` or a
   new `docs/patch-conventions.md`) that Phases 01–05 reference: the canonical
   `From:` author/email, a `Signed-off-by:` line (DCO), SPDX headers on
   net-new files, Subject-line format (`<component>: <imperative summary>`
   matching actual content), and a note to widen minimal-context hunks that are
   apply-fragile. Keep it short and prescriptive.
2. **Refresh `DESIGN.md`.** Add a section covering: the `restore_match_rules`
   SelectSources option (shape, `kind=title_regex`, `pattern`, `scope=same_app|
   any_app`); the rescue modes (exact / app / title) and matching order; COSMIC
   token versions v1/v2/v3 and their compat (v1/v2 deserialize with empty alias
   lists); and — importantly — the **security posture** of `any_app` rescue
   (the confidentiality risk and how Phase 02 gates it) and the regex bounds.
   Also state the authoritative `source_label` sanitation ownership across layers
   (which layer validates length, which truncates, which strips control chars),
   resolving the "uneven by layer" weakness, consistent with Phases 01/04/05.
3. **Fix the mock README examples** so every scenario snippet compiles against
   the real types in `screencast-portal-mock/src/scenario.rs` (add the required
   fields, or mark snippets as illustrative pseudocode if that is the intent —
   prefer making them real).
4. **Record future upstreaming steps** (COSMIC patch split; any residual picker-
   markup-escaping concern noted in Phase 04) as a short "Next steps / not in
   this plan" list so they aren't lost.
5. **Verify** docs internally: links resolve; the DESIGN section names match the
   code symbols; the README snippets compile (paste into a scratch test or
   `cargo build --example` if an example covers them).

## Acceptance criteria

- [ ] `DESIGN.md` documents `restore_match_rules`, rescue modes + matching
      order, token v1/v2/v3 compat, the `any_app` confidentiality posture +
      regex bounds, and the authoritative `source_label` sanitation ownership.
- [ ] A written patch-grooming convention exists (author, `Signed-off-by`, SPDX,
      Subject format) that Phases 01–05 can and do follow.
- [ ] The mock `README.md` scenario examples match the real field-bearing types
      (they compile, or are explicitly marked pseudocode).
- [ ] A "future upstreaming steps" note records the COSMIC patch split and any
      residual picker-escaping concern.
- [ ] No `.patch` file content was modified by this phase.

## Files likely touched

- `DESIGN.md`
- `docs/patch-conventions.md` (new, optional) and/or a `DESIGN.md` section.
- `screencast-portal-mock/README.md`
- Top-level `README.md` if one exists.

## Pitfalls

- **Symptom:** DESIGN describes behavior that the patches don't actually
  implement. **Cause:** documenting intent instead of code. **Recovery:** read
  the COSMIC patch (`parse_restore_match_rules`, the `RESCUE_MODE_*` constants,
  the v3 `TryFrom`) and `docs/restore-match-rules.md`; describe what is there.
- **Symptom:** the grooming convention conflicts with what Phases 01–05 already
  applied. **Cause:** this phase ran after they groomed. **Recovery:** since all
  phases run in Wave 0, agree the convention early (this phase's step 1 is the
  source of truth); if 01–05 already committed headers, reconcile to this
  convention and note any divergence.
- **Symptom:** README snippets still don't compile after edits. **Cause:** the
  scenario type signatures changed in Phase 06. **Recovery:** if Phase 06 has
  landed, match its types; otherwise match current `scenario.rs` and note that
  Phase 06 may adjust them.

## Reference

- Originating diagnosis: multi-agent audit, findings: stale DESIGN.md, README
  scenario drift, patch authorship/Subject/SPDX/sign-off inconsistencies.
- [DESIGN.md](../../../DESIGN.md) (current, stale),
  [docs/restore-match-rules.md](../../restore-match-rules.md) (the rescue
  semantics to fold into DESIGN).
- The patch-grooming convention here is referenced by the final acceptance
  criterion of every Phase 01–05 file.
