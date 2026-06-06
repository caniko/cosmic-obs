{
  description = "ScreenCast portal RFC: source_label + restore_policy across the stack";

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

      flake = let
        screencastPortalRfcModule = import ./nix/nixos-module.nix;
      in {
        nixosModules = {
          default = screencastPortalRfcModule;
          screencast-portal-rfc = screencastPortalRfcModule;
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

          # Keep the integration suite out of the package build because it
          # spawns D-Bus daemons, but gate it explicitly in `nix flake check`.
          # `dbus-run-session` provides a predictable session bus for CI while
          # the tests continue to create their own private buses.
          mock-tests = craneLib.cargoTest (commonArgs
            // {
              inherit cargoArtifacts;
              nativeBuildInputs = commonArgs.nativeBuildInputs ++ [pkgs.dbus];
              preCheck = ''
                export DBUS_FATAL_WARNINGS=0

                # The test helper intentionally invokes `dbus-daemon --session`
                # directly. In the Nix sandbox there is no /etc/dbus-1, so put
                # a tiny wrapper first on PATH that points dbus-daemon at its
                # immutable Nix-store session configuration.
                mkdir -p "$TMPDIR/dbus-bin"
                cat >"$TMPDIR/dbus-bin/dbus-daemon" <<'EOF'
                #!${pkgs.bash}/bin/bash
                set -euo pipefail

                args=()
                for arg in "$@"; do
                  if [[ "$arg" != "--session" ]]; then
                    args+=("$arg")
                  fi
                done

                exec ${pkgs.dbus}/bin/dbus-daemon --config-file=${pkgs.dbus}/share/dbus-1/session.conf "''${args[@]}"
                EOF
                chmod +x "$TMPDIR/dbus-bin/dbus-daemon"
                export PATH="$TMPDIR/dbus-bin:$PATH"
              '';
              checkPhaseCargoCommand = "dbus-run-session -- cargoWithProfile test --locked --all-targets";
            });

          # Cross-layer contract check for the ScreenCast v6 option path:
          # patched OBS sends the keys, xdg-desktop-portal allows them through
          # SelectSources validation, and COSMIC reads them from SelectSources.
          v6-key-path = pkgs.runCommand "screencast-v6-key-path" {} ''
            set -euo pipefail

            xdp_patch=${./patches/xdg-desktop-portal/0001-screencast-add-source_label-and-restore_policy.patch}
            cosmic_patch=${./patches/xdg-desktop-portal-cosmic/0001-screencast-implement-restore_policy-with-rescue.patch}
            obs_patch=${./patches/obs-studio/0001-linux-pipewire-send-source_label-and-restore_policy.patch}
            mock_records=${./screencast-portal-mock/src/records.rs}
            mock_obs=${./screencast-portal-mock/src/obs.rs}

            require() {
              local pattern="$1"
              local file="$2"
              local label="$3"
              if ! grep -Fq "$pattern" "$file"; then
                echo "missing $label: $pattern in $file" >&2
                exit 1
              fi
            }

            # OBS sends every v6 key when portal version >= 6 and a restore token exists.
            require 'get_screencast_version() >= 6' "$obs_patch" 'OBS v6 gate'
            require '"source_label"' "$obs_patch" 'OBS source_label send'
            require '"restore_policy"' "$obs_patch" 'OBS restore_policy send'
            require '"restore_match_rules"' "$obs_patch" 'OBS restore_match_rules send'
            require 'capture->restore_token && *capture->restore_token' "$obs_patch" 'OBS restore-token gate'

            # xdg-desktop-portal keeps every v6 key in the SelectSources allow-list.
            require '{ "source_label", G_VARIANT_TYPE_STRING, validate_source_label }' "$xdp_patch" 'xdp source_label allow-list'
            require '{ "restore_policy", G_VARIANT_TYPE_VARDICT, validate_restore_policy }' "$xdp_patch" 'xdp restore_policy allow-list'
            require '{ "restore_match_rules", (const GVariantType *) "aa{sv}", validate_restore_match_rules }' "$xdp_patch" 'xdp restore_match_rules allow-list'

            # COSMIC reads every v6 key from SelectSources options.
            require 'source_label: Option<String>' "$cosmic_patch" 'COSMIC source_label option'
            require 'restore_policy: Option<HashMap<String, zvariant::OwnedValue>>' "$cosmic_patch" 'COSMIC restore_policy option'
            require 'restore_match_rules: Option<Vec<HashMap<String, zvariant::OwnedValue>>>' "$cosmic_patch" 'COSMIC restore_match_rules option'
            require 'parse_restore_match_rules(options.restore_match_rules)' "$cosmic_patch" 'COSMIC restore_match_rules parse'

            # The in-process OBS model and mock stay aligned with the same key set.
            require '"source_label"' "$mock_records" 'mock source_label accepted key'
            require '"restore_policy"' "$mock_records" 'mock restore_policy accepted key'
            require '"restore_match_rules"' "$mock_records" 'mock restore_match_rules accepted key'
            require 'representative_v6_keys' "$mock_obs" 'mock OBS representative key set'

            mkdir -p "$out"
            cat >"$out/evidence.txt" <<'EOF'
            ScreenCast v6 key-path evidence:
            - OBS sends source_label, restore_policy, and restore_match_rules for portal v6 restore calls.
            - xdg-desktop-portal SelectSources allows source_label, restore_policy, and restore_match_rules through validation.
            - COSMIC SelectSources reads source_label, restore_policy, and restore_match_rules, including restore_match_rules parsing.
            - The Rust mock accepted key set and in-process OBS model cover the same v6 keys.
            EOF
          '';

          # Verify that RFC patches still apply cleanly to upstream packages.
          # Each of these builds the patched package; a failing patch = a failing check.
          xdp-patch-applies = import ./nix/xdg-desktop-portal.nix {inherit pkgs;};
          libportal-patch-applies = import ./nix/libportal.nix {inherit pkgs;};
          xdph-patch-applies = import ./nix/xdph.nix {inherit pkgs;};
          cosmic-patch-applies = import ./nix/xdg-desktop-portal-cosmic.nix {inherit pkgs;};
          obs-patch-applies = import ./nix/obs-studio.nix {inherit pkgs;};
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
