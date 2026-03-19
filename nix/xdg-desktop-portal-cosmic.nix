{pkgs, ...}:
pkgs.xdg-desktop-portal-cosmic.overrideAttrs (old: {
  patches =
    (old.patches or [])
    ++ [
      ../patches/xdg-desktop-portal-cosmic/0001-screencast-implement-restore_fail_policy-with-rescue.patch
    ];
})
