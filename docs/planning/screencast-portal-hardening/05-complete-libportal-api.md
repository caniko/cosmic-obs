# Phase 05 — Complete the libportal public API (GBoxed type + getters)

> **Recommended Codex model: GPT 5.5 medium** (`reasoning_effort: medium`)
>
> Moderate complexity, pattern-following work. Registering a `G_DEFINE_BOXED_TYPE`
> and adding symmetric getters is well-trodden GObject boilerplate, but GObject-
> Introspection correctness (annotations, the `get_type` symbol, header
> declarations) has sharp edges where a wrong annotation silently breaks
> bindings. That justifies `medium` over `low`. It is not `high`: there is no
> novel design — the canonical boxed-type triple already exists in the patch, it
> just lacks its GType. Bump to `high` only if `g-ir-scanner`/introspection
> generation fails in a way that needs real debugging.

## Working tree

`/data/nvme0/can/Projects/cosmic-obs`. Edits are to
`patches/libportal/0001-remote-add-source_label-and-restore_policy-to-screencast-API.patch`.
Verification builds via `nix build .#libportal`. No other phase touches this
file.

## Goal

`XdpRestorePolicy` is a complete, introspectable public boxed type: it has a
registered `GType`, symmetric setters **and** getters, and clean GObject-
Introspection so language bindings work. `nix build .#libportal` succeeds with
introspection enabled.

## Why this matters now

Audit findings in this patch file:

1. **Medium — no GBoxed `GType`, breaking GI bindings.** `XdpRestorePolicy` is an
   opaque, heap-allocated public type exposed with the canonical boxed triple
   (`_new()`/`_copy()`/`_free()`), and the new `_full()` entry point carries GI
   annotations (e.g. `@restore_policy: (nullable):`). But the patch never
   registers a `GType` — no `G_DEFINE_BOXED_TYPE`, no
   `xdp_restore_policy_get_type()` declaration. Without it, GObject-Introspection
   cannot describe the type and bindings (Python/JS/etc.) for the new API break.
2. **Info — setters but no getters** (asymmetric API surface): the policy can be
   built but not read back.
3. **Info — `default_action = PROMPT` is serialized even though PROMPT is the
   protocol default**, adding a redundant key to the wire vardict.
4. **Info — `source_label` truncation** happens here (UTF-8-boundary truncate to
   ≤256 bytes); coordinate the contract with Phase 01's decision (if Phase 01
   keeps a hard 256-byte reject at the base portal, libportal should reject or
   pre-truncate consistently so a label can't pass one layer and violate
   another's assumption).

## Out of scope

- Do **not** touch other patches.
- Do **not** redesign the `XdpRestorePolicy` struct fields or the
  `restore_policy` wire shape.
- Do **not** add new public API beyond the getters needed for symmetry and the
  `get_type` registration.

## Plan

1. **Register the GType.** Add
   `G_DEFINE_BOXED_TYPE(XdpRestorePolicy, xdp_restore_policy,
   xdp_restore_policy_copy, xdp_restore_policy_free)` in the appropriate `.c`
   (e.g. `remote.c`), declare `XDP_PUBLIC GType xdp_restore_policy_get_type
   (void);` and the `XDP_TYPE_RESTORE_POLICY` macro in the public header, next to
   the existing boxed-triple declarations.
2. **Add getters** mirroring the setters (e.g. `xdp_restore_policy_get_default_action`,
   `xdp_restore_policy_get_action_for_reason` / however the setters are shaped)
   with correct GI annotations.
3. **Stop serializing the redundant `default_action = PROMPT`** — only add the
   key to the vardict when it differs from the protocol default, so a
   default-only policy sends an empty/omitted vardict matching "defaults preserve
   current behavior".
4. **Align the `source_label` truncation contract** with Phase 01's decision
   (read [Phase 01](./01-repair-xdp-generic-patch.md) step 5 / its commit
   message). If the base portal hard-rejects > 256 bytes, make libportal reject
   (or pre-truncate to a value the base portal accepts) rather than silently
   produce a label the base portal will reject.
5. **Build with introspection.** `nix build .#libportal` (ensure the package
   build has GI enabled so a broken `get_type` surfaces), then `nix flake check`
   (`libportal-patch-applies`).

## Acceptance criteria

- [ ] `xdp_restore_policy_get_type()` is registered via `G_DEFINE_BOXED_TYPE`
      and declared `XDP_PUBLIC` in the header, with an `XDP_TYPE_RESTORE_POLICY`
      macro.
- [ ] Getters exist for every setter, with correct GI annotations.
- [ ] A default-only `XdpRestorePolicy` does not serialize a redundant
      `default_action = PROMPT` key.
- [ ] `source_label` length handling is consistent with Phase 01's contract
      decision (no label can pass libportal that the base portal would reject).
- [ ] `nix build .#libportal` succeeds with introspection enabled;
      `nix flake check` passes.
- [ ] The patch's `From:`/`Subject:`/`Signed-off-by:`/SPDX header is normalized
      to the agreed convention. Apply here so Phase 08 never re-touches this file.

## Files likely touched

- `patches/libportal/0001-remote-add-source_label-and-restore_policy-to-screencast-API.patch`
  (embedded `libportal/remote.c` + public header hunks).

## Pitfalls

- **Symptom:** `g-ir-scanner` warns or the build fails on the new type.
  **Cause:** missing/incorrect annotations or `get_type` not exported.
  **Recovery:** ensure `XDP_PUBLIC` on the `get_type` decl and that the
  `G_DEFINE_BOXED_TYPE` name prefix matches the function names exactly.
- **Symptom:** ABI/symbol mismatch. **Cause:** declaring the boxed macro in the
  wrong header section. **Recovery:** place it alongside the existing
  `_new/_copy/_free` declarations.
- **Symptom:** omitting `default_action` changes behavior for callers that
  *want* explicit Prompt. **Cause:** over-aggressive omission. **Recovery:** only
  omit when the value equals the protocol default AND no per-reason actions are
  set.

## Reference

- Originating diagnosis: multi-agent audit, libportal GBoxed/getters/redundant-
  default/truncation findings.
- [Phase 01](./01-repair-xdp-generic-patch.md) step 5 — the `source_label`
  length contract this phase must align with.
- libportal upstream boxed-type conventions (compare an existing libportal boxed
  type's `G_DEFINE_BOXED_TYPE` usage).
