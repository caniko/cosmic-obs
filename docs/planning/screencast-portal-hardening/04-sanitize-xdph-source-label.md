# Phase 04 — Sanitize source_label in the XDPH share picker

> **Recommended Codex model: GPT 5.5 medium** (`reasoning_effort: medium`)
>
> Moderate complexity. The fix is well-understood: mirror COSMIC's existing
> `sanitize_label` (strip a defined control-character set, truncate at a
> codepoint boundary) before the untrusted label crosses into the picker
> subprocess and the log. The threat model is clear and the change is localized
> to one small C++ patch with one clear reference implementation to copy. No
> design novelty, so `medium`, not `high`. A `low` model might forget the
> log-before-sanitize site or strip the wrong character set.

## Working tree

`/data/nvme0/can/Projects/cosmic-obs`. Edits are to
`patches/xdg-desktop-portal-hyprland/0001-screencast-display-source_label-in-share-picker.patch`.
Verification builds via `nix build .#xdg-desktop-portal-hyprland`. No other
phase touches this file.

## Goal

The untrusted `source_label` is sanitized (control/bidi characters stripped,
length-bounded, codepoint-safe) **before** it is forwarded into the share-picker
subprocess environment and **before** it is logged, so a hostile label cannot
inject markup/control sequences into the privileged Qt picker or corrupt the
debug log. `nix build .#xdg-desktop-portal-hyprland` succeeds.

## Why this matters now

Audit findings in this patch file. The current code forwards the label verbatim:

```cpp
} else if (key == "source_label") {
    sourceLabel = val.get<std::string>();                          // L30: throws on wrong type
    Debug::log(LOG, "[screencopy] option source_label: {}", sourceLabel);  // L31: logs raw, unsanitized
}
...
if (!sourceLabel.empty())
    proc.addEnv("XDPH_SOURCE_LABEL", sourceLabel);                 // L61-62: into picker env, no sanitization
```

- **Medium — unsanitized label into the picker.** The only guard is
  `if (!sourceLabel.empty())`. [DESIGN.md](../../../DESIGN.md) states the label
  "must be plain text and must not be interpreted as markup," and names XDPH as
  the layer that "forwards the label to the picker environment." The Qt picker
  may render it as markup; control / bidi (U+202A–U+202E, U+2066–U+2069) / NUL /
  terminal-escape characters are passed straight through.
- **Low — logged verbatim before any stripping** (`Debug::log(LOG, …,
  sourceLabel)` at L31) → log injection / terminal-escape via the debug log.
- **Low — `val.get<std::string>()` throws on a type-mismatched value**, which
  can abort the request (consistent with the existing `persist_mode` pattern,
  but worth guarding).

## Out of scope

- Do **not** introduce a new IPC channel for the label — keep using the
  `XDPH_SOURCE_LABEL` env var, just sanitized.
- Do **not** touch other patches. Sanitation *ownership* across layers is
  documented in Phase 08; this phase just makes XDPH defensively sanitize what it
  forwards.
- Do **not** change the picker UI itself (that's the hyprland picker repo, out
  of this RFC's scope).

## Plan

1. **Read COSMIC's `sanitize_label`** in
   `patches/xdg-desktop-portal-cosmic/0001-…patch` to learn the exact
   control-character set and truncation length the project treats as canonical
   (the audit references a set like U+0000–U+001F, U+007F, U+200E–U+200F,
   U+202A–U+202E, U+2066–U+2069, truncated to ≤256 bytes at a codepoint
   boundary). Match it so the layers agree.
2. **Add a small sanitizer** in the XDPH patch (a local helper or inline) that
   strips that set and truncates at a UTF-8 codepoint boundary to the agreed
   byte limit.
3. **Sanitize before use and before log.** Apply it immediately after reading
   the value, so both the `Debug::log` at L31 and the `addEnv` at L61–62 receive
   the sanitized string. Log the sanitized form only.
4. **Guard the type read.** Wrap `val.get<std::string>()` so a wrong-typed value
   is ignored (label dropped) rather than throwing out of `onSelectSources`, if
   the surrounding code doesn't already catch it.
5. **Build + gate.** `nix build .#xdg-desktop-portal-hyprland` then
   `nix flake check` (`xdph-patch-applies`).

## Acceptance criteria

- [ ] `source_label` is stripped of the agreed control/bidi character set and
      truncated at a codepoint boundary to the agreed byte limit before it is
      placed in `XDPH_SOURCE_LABEL`.
- [ ] The debug log receives only the sanitized label (no raw control chars).
- [ ] A type-mismatched `source_label` value does not throw out of
      `onSelectSources` (label is dropped instead).
- [ ] The sanitizer's character set + length match COSMIC's `sanitize_label`
      (cross-layer agreement).
- [ ] `nix build .#xdg-desktop-portal-hyprland` succeeds; `nix flake check`
      passes.
- [ ] The patch's `From:`/`Subject:`/`Signed-off-by:`/SPDX header is normalized
      to the agreed convention. Apply here so Phase 08 never re-touches this file.

## Files likely touched

- `patches/xdg-desktop-portal-hyprland/0001-screencast-display-source_label-in-share-picker.patch`
  (embedded `src/portals/Screencopy.cpp`, `src/shared/ScreencopyShared.cpp`).

## Pitfalls

- **Symptom:** valid non-ASCII labels (CJK, emoji) get mangled. **Cause:**
  truncating mid-codepoint or stripping too broad a Unicode range. **Recovery:**
  strip only the defined control/bidi set; truncate on `char` boundaries via a
  UTF-8-aware step.
- **Symptom:** patch won't apply. **Cause:** the existing hunks are tightly
  anchored. **Recovery:** regenerate against the pinned XDPH source.
- **Symptom:** picker still renders markup. **Cause:** the picker interprets the
  env value as Pango markup; stripping control chars isn't enough. **Recovery:**
  if the picker uses markup, also escape `<`/`&`; note the residual concern for
  the picker repo in Phase 08's cross-layer doc.

## Reference

- Originating diagnosis: multi-agent audit, XDPH source_label injection /
  logging / type-read findings.
- [DESIGN.md](../../../DESIGN.md) "Weaknesses" — uneven `source_label`
  sanitation by layer (Phase 08 documents the authoritative ownership).
- Reference sanitizer: COSMIC `sanitize_label` in the cosmic patch.
