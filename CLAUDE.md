# screencast-portal-rfc

## What this repo is
A mono-repo containing patches for five upstream projects and a new Rust
test harness, all packaged via a single Nix flake. The goal is to ship
two new options in the ScreenCast portal's SelectSources() call:

  source_label (string): human-readable hint from the caller identifying
    which application-level source this D-Bus call corresponds to.
    Displayed by portal backends in the source picker dialog.

  restore_fail_mode (u32): controls what the portal does when a restore
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
