{
  description = "A cargo workspace hosting small vibe-coded Rust projects";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    codex = {
      url = "github:openai/codex/rust-v0.147.0";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-overlay.follows = "rust-overlay";
    };
    opendesign = {
      url = "path:./forks/opendesign";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.home-manager.follows = "home-manager";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      home-manager,
      codex,
      opendesign,
      ...
    }:
    let
      inherit (nixpkgs) lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = f: lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});

      # <name>.nix files in a directory, as { name = path; }. Feeds the
      # globbed flake outputs below, mirroring how crates/* feeds the cargo
      # workspace: drop a file in, no flake edits needed.
      nixFilesIn =
        dir:
        lib.mapAttrs' (
          fileName: _: lib.nameValuePair (lib.removeSuffix ".nix" fileName) (dir + "/${fileName}")
        ) (lib.filterAttrs (n: t: t == "regular" && lib.hasSuffix ".nix" n) (builtins.readDir dir));

      extraPackages =
        pkgs:
        lib.mapAttrs (
          name: f:
          pkgs.callPackage f (
            lib.optionalAttrs (name == "codex-trust-state") {
              codexUpstream = codex.packages.${pkgs.stdenv.hostPlatform.system}.codex-rs;
            }
          )
        ) (nixFilesIn ./nix/packages);

      # One package per workspace crate, built with `cargo build -p <crate>`.
      crates = lib.attrNames (lib.filterAttrs (_: t: t == "directory") (builtins.readDir ./crates));
      cratePackages =
        pkgs:
        let
          craneLib = crane.mkLib pkgs;
          workspaceSrc = craneLib.cleanCargoSource ./.;

          # Compile third-party dependencies once and reuse the resulting Cargo
          # target directory for every crate package. This derivation depends on
          # Cargo manifests and Cargo.lock, but not on Rust source changes.
          cargoArtifacts = craneLib.buildDepsOnly {
            src = workspaceSrc;
            pname = "good-vibes-only-workspace";
            version = "0.1.0";
            strictDeps = true;
          };
        in
        lib.genAttrs crates (
          crate:
          pkgs.callPackage ./nix/package.nix {
            # Keep non-Rust crate assets (for example tally's fonts), while
            # excluding every other crate so unrelated edits stay cached.
            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                (./crates + "/${crate}")
              ];
            };
            inherit
              cargoArtifacts
              craneLib
              crate
              ;
          }
        );
    in
    {
      packages = forAllSystems (
        pkgs:
        cratePackages pkgs
        // extraPackages pkgs
        // {
          opendesign = opendesign.packages.${pkgs.stdenv.hostPlatform.system}.daemon;
        }
      );

      overlays.default = final: _prev: cratePackages final // extraPackages final;

      # Every nix/home-manager/<name>.nix is exported as
      # homeManagerModules.<name>.
      homeManagerModules = nixFilesIn ./nix/home-manager // {
        open-design = opendesign.homeManagerModules.open-design;
      };

      homeModules = self.homeManagerModules;

      nixosModules = {
        home-media-system = ./nix/nixos/home-media-system.nix;
        open-design = opendesign.nixosModules.open-design;
      };

      nixosConfigurations.home-media-system-vm = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          "${nixpkgs}/nixos/modules/virtualisation/qemu-vm.nix"
          self.nixosModules.home-media-system
          {
            system.stateVersion = "25.11";
            networking.hostName = "home-media-demo";

            services.home-media-system = {
              enable = true;
              locale = "en-GB";
              applications = {
                jellyfin-home = {
                  type = "jellyfin";
                  url = "https://jellyfin.example.net";
                  order = 10;
                  autoLogin = {
                    enable = true;
                    username = "home-user";
                    passwordFile = "/run/secrets/jellyfin-home-password";
                  };
                };
                jellyfin-family = {
                  name = "Family Jellyfin";
                  type = "jellyfin";
                  url = "https://family-jellyfin.example.net";
                  order = 15;
                  autoLogin = {
                    enable = true;
                    username = "family-user";
                    passwordFile = "/run/secrets/jellyfin-family-password";
                  };
                };
                jellyseerr = {
                  type = "jellyseerr";
                  order = 20;
                };
                youtube = {
                  type = "youtube";
                  url = "https://www.youtube.com/tv";
                  order = 30;
                };
              };
            };

            virtualisation = {
              memorySize = 3072;
              cores = 2;
              graphics = true;
            };
          }
        ];
      };

      apps.x86_64-linux.home-media-system-vm = {
        type = "app";
        program = lib.getExe self.nixosConfigurations.home-media-system-vm.config.system.build.vm;
        meta.description = "Run the home media system demo VM";
      };

      checks = forAllSystems (pkgs: {
        packages = pkgs.symlinkJoin {
          name = "all-packages";
          paths = lib.attrValues self.packages.${pkgs.stdenv.hostPlatform.system};
        };

        # Eval-only smoke test for the nono module: a minimal home
        # configuration exercising every option. `nix flake check
        # --no-build` catches module regressions without building anything.
        hm-nono =
          (home-manager.lib.homeManagerConfiguration {
            inherit pkgs;
            modules = [
              self.homeManagerModules.nono
              {
                home = {
                  username = "vibes";
                  homeDirectory = "/home/vibes";
                  stateVersion = "25.11";
                };
                programs.nono = {
                  enable = true;
                  profiles.rust-dev = {
                    extends = "claude-code";
                    filesystem.read = [ "~/references" ];
                  };
                  wrappers.claude-sandboxed = {
                    command = "claude --dangerously-skip-permissions";
                    extraFlags = [ "--allow-cwd" ];
                    allowGitCommonDir = true;
                  };
                };
              }
            ];
          }).activationPackage;

        hm-ccstatusline =
          (home-manager.lib.homeManagerConfiguration {
            inherit pkgs;
            modules = [
              self.homeManagerModules.ccstatusline
              {
                home = {
                  username = "vibes";
                  homeDirectory = "/home/vibes";
                  stateVersion = "25.11";
                };
                # Real package is unfree; any package satisfies the eval-only check.
                programs.claude-code = {
                  enable = true;
                  package = pkgs.hello;
                };
                programs.ccstatusline = {
                  enable = true;
                  settings.lines = [
                    [
                      {
                        id = "model";
                        type = "model";
                      }
                      {
                        id = "branch";
                        type = "git-branch";
                      }
                    ]
                  ];
                };
              }
            ];
          }).activationPackage;

      });
    };
}
