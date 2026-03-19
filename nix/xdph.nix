{pkgs, ...}:
pkgs.xdg-desktop-portal-hyprland.overrideAttrs (old: {
  patches =
    (old.patches or [])
    ++ [
      ../patches/xdg-desktop-portal-hyprland/0001-screencast-display-source_label-in-share-picker.patch
    ];
})
