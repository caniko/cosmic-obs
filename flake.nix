{
  description = "ScreenCast portal RFC: source_label + restore_fail_policy across the stack";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    crane.url = "github:ipetkov/crane";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ {flake-parts, ...}:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux" "aarch64-linux"];

      imports = [
        inputs.treefmt-nix.flakeModule
      ];

      flake = {
        nixosModules.default = {
          config,
          lib,
          ...
        }: let
          cfg = config.services.screencast-portal-rfc;
        in {
          options.services.screencast-portal-rfc = {
            enable =
              lib.mkEnableOption "patched ScreenCast portal stack with source_label and restore_fail_policy";

            usePatched = lib.mkOption {
              type = lib.types.bool;
              default = true;
              description = "Whether to apply RFC patches. Set to false to use unpatched upstream packages.";
            };
          };

          config = lib.mkIf cfg.enable {
            nixpkgs.overlays = lib.mkIf cfg.usePatched [
              (final: prev: {
                xdg-desktop-portal = import ./nix/xdg-desktop-portal.nix {pkgs = prev;};
                libportal = import ./nix/libportal.nix {pkgs = prev;};
                xdg-desktop-portal-hyprland = import ./nix/xdph.nix {pkgs = prev;};
                xdg-desktop-portal-cosmic = import ./nix/xdg-desktop-portal-cosmic.nix {pkgs = prev;};
                obs-studio = import ./nix/obs-studio.nix {pkgs = prev;};
              })
            ];
          };
        };
      };

      perSystem = {
        pkgs,
        system,
        ...
      }: let
        fenixPkgs = inputs.fenix.packages.${system};
        toolchain = fenixPkgs.stable.withComponents [
          "rustc"
          "cargo"
          "clippy"
          "rustfmt"
          "rust-src"
          "rust-analyzer"
        ];
        craneLib = (inputs.crane.mkLib pkgs).overrideToolchain toolchain;

        mockSrc = craneLib.cleanCargoSource ./screencast-portal-mock;
        commonArgs = {
          src = mockSrc;
          pname = "screencast-portal-mock";
          version = "0.1.0";
          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
          buildInputs = with pkgs; [
            dbus
            glib
          ];
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
      in {
        treefmt.config = {
          projectRootFile = "flake.nix";
          programs.rustfmt.enable = true;
          programs.alejandra.enable = true;
        };

        packages = {
          screencast-portal-mock = craneLib.buildPackage (commonArgs
            // {
              inherit cargoArtifacts;
              # Integration tests need a running dbus-daemon which is unavailable in the nix sandbox
              doCheck = false;
            });

          xdg-desktop-portal = import ./nix/xdg-desktop-portal.nix {inherit pkgs;};
          libportal = import ./nix/libportal.nix {inherit pkgs;};
          xdg-desktop-portal-hyprland = import ./nix/xdph.nix {inherit pkgs;};
          xdg-desktop-portal-cosmic = import ./nix/xdg-desktop-portal-cosmic.nix {inherit pkgs;};
          obs-studio = import ./nix/obs-studio.nix {inherit pkgs;};
        };

        checks = {
          mock-clippy = craneLib.cargoClippy (commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "-- -D warnings";
            });
        };

        devShells.default = craneLib.devShell {
          inputsFrom = [commonArgs];
          packages = with pkgs; [
            cargo-nextest
            dbus
            glib
            pipewire
          ];
        };
      };
    };
}
