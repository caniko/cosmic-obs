{
  config,
  lib,
  pkgs,
  ...
}: let
  cfg = config.services.screencast-portal-rfc;
  optionalPatchedComponent = enabled: package:
    lib.optionals (cfg.usePatched && enabled) [package];
in {
  options.services.screencast-portal-rfc = {
    enable =
      lib.mkEnableOption "patched ScreenCast portal stack with source_label and restore_policy";

    usePatched = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Whether to apply RFC patches. Set to false to use unpatched upstream packages.";
    };

    components = {
      xdgDesktopPortal = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Patch xdg-desktop-portal with the generic ScreenCast v6 API.";
      };

      libportal = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Patch libportal with the ScreenCast v6 client API.";
      };

      hyprland = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Patch xdg-desktop-portal-hyprland to display source_label.";
      };

      cosmic = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Patch xdg-desktop-portal-cosmic with restore_policy rescue behavior.";
      };

      obsStudio = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Patch OBS Studio to send source_label and restore_policy.";
      };
    };
  };

  config = let
    anyComponentEnabled =
      cfg.components.xdgDesktopPortal
      || cfg.components.libportal
      || cfg.components.hyprland
      || cfg.components.cosmic
      || cfg.components.obsStudio;
  in
    lib.mkIf cfg.enable {
      nixpkgs.overlays = lib.mkIf (cfg.usePatched && anyComponentEnabled) [
        (final: prev:
          lib.optionalAttrs cfg.components.xdgDesktopPortal {
            xdg-desktop-portal = import ./xdg-desktop-portal.nix {pkgs = prev;};
          }
          // lib.optionalAttrs cfg.components.libportal {
            libportal = import ./libportal.nix {pkgs = prev;};
          }
          // lib.optionalAttrs cfg.components.hyprland {
            xdg-desktop-portal-hyprland = import ./xdph.nix {pkgs = prev;};
          }
          // lib.optionalAttrs cfg.components.cosmic {
            xdg-desktop-portal-cosmic = import ./xdg-desktop-portal-cosmic.nix {pkgs = prev;};
          }
          // lib.optionalAttrs cfg.components.obsStudio {
            obs-studio = import ./obs-studio.nix {pkgs = prev;};
          })
      ];

      xdg.portal.extraPortals =
        optionalPatchedComponent cfg.components.cosmic pkgs.xdg-desktop-portal-cosmic;
    };
}
