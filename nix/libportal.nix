{pkgs, ...}:
pkgs.libportal.overrideAttrs (old: {
  patches =
    (old.patches or [])
    ++ [
      ../patches/libportal/0001-remote-add-source_label-and-restore_fail_policy-to-s.patch
    ];
})
