{pkgs, ...}:
pkgs.libportal.overrideAttrs (old: {
  patches =
    (old.patches or [])
    ++ [
      ../patches/libportal/0001-remote-add-source_label-and-restore_policy-to-screencast-API.patch
    ];
})
