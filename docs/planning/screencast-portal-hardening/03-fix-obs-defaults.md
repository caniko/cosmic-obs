# Phase 03 — Fix OBS client defaults and option consistency

> **Recommended Codex model: GPT 5.5 high** (`reasoning_effort: high`)
>
> Complex but bounded. The core bug is a non-obvious libobs data-model semantic
> (`obs_data_has_user_value` is false for a registered *default*, so the fallback
> path silently yields `Prompt` instead of `Skip`), which defeats the entire
> reason the RFC exists. Getting it right requires understanding OBS settings
> lifecycle (defaults vs user values, create vs update, deferred video_tick
> updates), not just editing three lines. That is a real design call, so route
> `high`; it is not frontier (single C patch file, well-understood once the
> data-model semantics are clear), so it is not `max`. A `medium` model risks
> "fixing" it by registering a default for the orphaned `RestoreFailMode` key
> instead of removing the broken fallback.

## Working tree

`/data/nvme0/can/Projects/cosmic-obs`. Edits are to
`patches/obs-studio/0001-linux-pipewire-send-source_label-and-restore_policy.patch`.
Verification builds via `nix build .#obs-studio`. No other phase touches this
file.

## Goal

An OBS source upgraded from a pre-feature build, carrying a stale
`restore_token` and the default policy, resolves its restore action to **Skip**
(not Prompt) and therefore does **not** pop an unexpected picker — fulfilling
the RFC's central promise. The create-time and update-time code agree, the
orphaned `RestoreFailMode` key is gone, and `nix build .#obs-studio` succeeds.

## Why this matters now

Audit findings in this patch file:

1. **High — upgraded OBS pops a picker (defeats the RFC's whole point).** All
   three `create()` functions compute the action as:
   ```c
   capture->restore_action = obs_data_has_user_value(settings, "RestorePolicyAction")
       ? (uint32_t)obs_data_get_int(settings, "RestorePolicyAction")
       : (uint32_t)obs_data_get_int(settings, "RestoreFailMode");
   ```
   (patch L149–151, L156–158, L163–165). For a source upgraded from a build
   without these keys, the saved JSON has neither key, so
   `obs_data_has_user_value("RestorePolicyAction")` is **false** (a default set
   via `obs_data_set_default_int` is *not* a user value), and the code reads
   `RestoreFailMode` — which is **never written, defaulted, or surfaced
   anywhere** (verified: it appears only in these fallbacks) → returns `0` =
   `Prompt`. The registered `Skip` default
   (`obs_data_set_default_int(settings, "RestorePolicyAction", 1)`, L174) is
   bypassed on the create path. `select_source()` then sends
   `restore_policy{default_action=0}` whenever a `restore_token` exists, so the
   upgrade scenario [DESIGN.md](../../../DESIGN.md) lines 81–84 promise to
   prevent instead pops the picker.

2. **Consistency — create vs update diverge.** `screencast_portal_capture_update()`
   reads it correctly and unconditionally:
   `capture->restore_action = (uint32_t)obs_data_get_int(settings,
   "RestorePolicyAction");` (L193), honoring the registered `Skip` default. The
   create path's fallback is the inconsistent one.

3. **Info/consistency — option gating.** `restore_match_rules` is sent by OBS
   unconditionally (independent of `restore_token`), unlike `restore_policy`
   which is token-gated; and the Skip default is applied even with no token —
   audit notes this diverges slightly from "defaults preserve current
   behavior". Decide and document the intended gating.

4. **Info/docs — `RestoreMatchRules` tooltip** omits the `same:`/`same_app:`/
   `any:`/`any_app:` prefixes the parser accepts (per
   [docs/restore-match-rules.md](../../restore-match-rules.md)).

## Out of scope

- Do **not** touch the COSMIC patch (Phase 02) or the generic portal (Phase 01).
- Do **not** redesign the OBS UI property model beyond removing the orphaned key
  and aligning create/update.
- Do **not** change the wire format of `restore_policy` / `restore_match_rules`.

## Plan

1. **Remove the broken fallback.** In all three `create()` functions, replace the
   `has_user_value ? RestorePolicyAction : RestoreFailMode` expression with the
   same unconditional read the update path uses:
   `capture->restore_action = (uint32_t)obs_data_get_int(settings,
   "RestorePolicyAction");`. The registered default `1` (Skip) then applies for
   upgraded sources (verified: when a key has no user value, `obs_data_get_int`
   returns the registered default). Factor the four identical sites into one
   small helper if it reduces duplication without disturbing patch context.
2. **Confirm the default registration** at L174 stays `1` (Skip) and that
   `get_defaults` runs before the create callback.
3. **Decide option gating (step 3 above).** Make `restore_match_rules` sending
   consistent with the intended contract (gate on `restore_token` presence if
   that matches "defaults preserve current behavior", or document why it is
   sent unconditionally). Keep the change minimal.
4. **Fix the tooltip** to mention the accepted scope prefixes.
5. **Build + gate.** `nix build .#obs-studio` then `nix flake check`
   (`obs-patch-applies`).

## Acceptance criteria

- [ ] All three `create()` functions read `RestorePolicyAction` unconditionally;
      no reference to `RestoreFailMode` remains anywhere in the patch.
- [ ] An upgraded source (saved JSON lacking `RestorePolicyAction`) resolves
      `restore_action == 1` (Skip) at create time — demonstrate by tracing the
      data-model path or an OBS-side assertion in the commit message.
- [ ] Create-path and update-path use identical logic to populate
      `restore_action`.
- [ ] The `RestoreMatchRules` property tooltip documents the
      `same:`/`same_app:`/`any:`/`any_app:` prefixes.
- [ ] `nix build .#obs-studio` succeeds; `nix flake check` passes.
- [ ] The patch's `From:`/`Subject:`/`Signed-off-by:`/SPDX header is normalized
      to the agreed convention. Apply here so Phase 08 never re-touches this file.

## Files likely touched

- `patches/obs-studio/0001-linux-pipewire-send-source_label-and-restore_policy.patch`
  (embedded `linux-pipewire` source hunks + the locale `.ini` tooltip hunk).

## Pitfalls

- **Symptom:** upgraded source still prompts. **Cause:** you kept a
  `has_user_value` guard somewhere, or registered the default after the create
  callback. **Recovery:** read it unconditionally; the getter returns the
  registered default when no user value exists.
- **Symptom:** locale hunk fails to apply. **Cause:** the audit flagged the
  tooltip/locale hunk uses a fragile 2-line context anchor on a frequently
  reordered `en-US.ini`. **Recovery:** widen context or re-anchor; regenerate
  the hunk against the pinned OBS source.
- **Symptom:** `nix build .#obs-studio` is very slow. **Cause:** OBS is a large
  build. **Recovery:** expect minutes; don't assume failure on slowness.

## Reference

- Originating diagnosis: multi-agent audit, finding #4 (Prompt-vs-Skip upgrade
  regression, verified down to libobs `obs-data.c` semantics) + create/update
  inconsistency + tooltip/gating notes.
- [DESIGN.md](../../../DESIGN.md) lines 81–84 (the no-unexpected-picker promise).
- Plan README: [./README.md](./README.md). [Phase 06](./06-mock-fidelity-coverage.md)'s
  OBS-client model should mirror the corrected default-Skip behavior decided here.
