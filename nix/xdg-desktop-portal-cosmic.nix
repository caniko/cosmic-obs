{pkgs, ...}: let
  cosmicPatch = ../patches/xdg-desktop-portal-cosmic/0001-screencast-implement-restore_policy-with-rescue.patch;
in
  pkgs.xdg-desktop-portal-cosmic.overrideAttrs (old: {
    cargoDeps = pkgs.runCommand "${old.pname}-${old.version}-vendor-patched" {} ''
      cp -r ${old.cargoDeps} "$out"
      chmod -R u+w "$out"
      sed -i '/^ "serde",$/ {
        N
        s/^ "serde",\n "tempfile",$/ "serde",\n "regex",\n "tempfile",/
      }' "$out/Cargo.lock"
      grep -q '^ "regex",$' "$out/Cargo.lock"
    '';

    patches =
      (old.patches or [])
      ++ [
        cosmicPatch
      ];
  })
