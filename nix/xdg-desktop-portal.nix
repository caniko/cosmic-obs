{pkgs, ...}:
pkgs.xdg-desktop-portal.overrideAttrs (old: {
  patches =
    (old.patches or [])
    ++ [
      ../patches/xdg-desktop-portal/0001-screencast-add-source_label-and-restore_fail_mode.patch
    ];
})
