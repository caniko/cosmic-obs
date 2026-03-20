{pkgs, ...}:
pkgs.obs-studio.overrideAttrs (old: {
  patches =
    (old.patches or [])
    ++ [
      ../patches/obs-studio/0001-linux-pipewire-send-source_label-and-restore_fail_mode.patch
    ];
})
