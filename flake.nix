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
    opendesign-src = {
      url = "tarball+https://github.com/nexu-io/open-design/archive/refs/tags/open-design-v0.22.2.tar.gz";
      flake = false;
    };
    # Node 24.19.0 can abort while NAN/ObjectWrap-based native addons run
    # cleanup hooks. Keep OpenDesign and its better-sqlite3 build on the last
    # known-good 24.x runtime without constraining the rest of this flake.
    opendesign-node-nixpkgs.url = "github:NixOS/nixpkgs/624af665418d3c65d544145b4d34ad696439570e";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      home-manager,
      opendesign-src,
      opendesign-node-nixpkgs,
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
        lib.filterAttrs (_: package: lib.meta.availableOn pkgs.stdenv.hostPlatform package) (
          lib.mapAttrs (_: f: pkgs.callPackage f { }) (nixFilesIn ./nix/packages)
        );

      playwrightChecks =
        pkgs:
        import ./nix/tests/playwright-cli.nix {
          inherit pkgs home-manager;
          module = ./nix/home-manager/playwright-cli.nix;
        };

      opendesignPackages =
        pkgs:
        import ./nix/opendesign {
          inherit pkgs;
          daemonNodejs = opendesign-node-nixpkgs.legacyPackages.${pkgs.stdenv.hostPlatform.system}.nodejs_24;
          source = opendesign-src;
        };

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
          opendesign = (opendesignPackages pkgs).daemon;
          opendesign-web = (opendesignPackages pkgs).web;
        }
      );

      overlays.default = final: _prev: cratePackages final // extraPackages final;

      # Every nix/home-manager/<name>.nix is exported as
      # homeManagerModules.<name>.
      homeManagerModules = nixFilesIn ./nix/home-manager // {
        open-design = import ./nix/opendesign/home-manager.nix {
          moduleCommon = import ./nix/opendesign/module-common.nix;
          flake = self;
        };
      };

      homeModules = self.homeManagerModules;

      nixosModules = {
        home-media-system = ./nix/nixos/home-media-system.nix;
        open-design = import ./nix/opendesign/nixos.nix {
          moduleCommon = import ./nix/opendesign/module-common.nix;
          flake = self;
        };
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
        hm-playwright-chromium = (playwrightChecks pkgs).chromium;
        hm-playwright-camoufox = (playwrightChecks pkgs).camoufox;
        hm-playwright-both = (playwrightChecks pkgs).both;
        hm-playwright-neither = (playwrightChecks pkgs).neither;

        # Eval-only smoke tests for both OpenDesign service modules. Use tiny
        # stand-in packages so `nix flake check --no-build` validates the
        # generated service definitions without pulling the application build
        # into the module tests.
        hm-open-design =
          (home-manager.lib.homeManagerConfiguration {
            inherit pkgs;
            modules = [
              self.homeManagerModules.open-design
              {
                home = {
                  username = "vibes";
                  homeDirectory = "/home/vibes";
                  stateVersion = "25.11";
                };
                services.open-design = {
                  enable = true;
                  package = pkgs.hello;
                  autoStart = true;
                  extraEnv.OD_CODEX_DISABLE_PLUGINS = "1";
                  extraBinPaths = [ "/opt/agents/bin" ];
                  webFrontend = {
                    enable = true;
                    package = pkgs.emptyDirectory;
                  };
                };
              }
            ];
          }).activationPackage;

        nixos-open-design =
          (lib.nixosSystem {
            system = pkgs.stdenv.hostPlatform.system;
            modules = [
              self.nixosModules.open-design
              {
                boot.isContainer = true;
                system.stateVersion = "25.11";
                services.open-design = {
                  enable = true;
                  package = pkgs.hello;
                  autoStart = true;
                  openFirewall = true;
                  webFrontend = {
                    enable = true;
                    package = pkgs.emptyDirectory;
                  };
                };
              }
            ];
          }).config.system.build.toplevel;

        camoufox-launch-settings =
          pkgs.runCommand "camoufox-launch-settings"
            {
              nativeBuildInputs = [ pkgs.python3 ];
            }
            (
              ''
                export HOME="$TMPDIR/home"
                mkdir -p "$HOME"
                python ${./nix/camoufox}/test-launch.py
              ''
              + lib.optionalString (pkgs.stdenv.hostPlatform.system == "x86_64-linux") (
                let
                  adapter = self.packages.x86_64-linux.camoufox-playwright;
                in
                ''
                  ${adapter.python}/bin/python ${./nix/camoufox/test-settings.py} ${adapter}/bin/camoufox-playwright
                ''
              )
              + ''touch "$out"''
            );

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

        hm-herdr =
          (home-manager.lib.homeManagerConfiguration {
            inherit pkgs;
            modules = [
              self.homeManagerModules.herdr
              {
                home = {
                  username = "vibes";
                  homeDirectory = "/home/vibes";
                  stateVersion = "25.11";
                };
                programs = {
                  herdr = {
                    enable = true;
                    package = pkgs.herdr;
                    settings = {
                      onboarding = false;
                      session.resume_agents_on_restore = true;
                      terminal.default_shell = "nu";
                    };
                    integrations = {
                      claude.enable = true;
                      codex.enable = true;
                      opencode.enable = true;
                    };
                  };
                };
              }
            ];
          }).activationPackage;

        hm-orca =
          (home-manager.lib.homeManagerConfiguration {
            inherit pkgs;
            modules = [
              self.homeManagerModules.orca
              ({ config, ... }: {
                home = {
                  username = "vibes";
                  homeDirectory = "/home/vibes";
                  stateVersion = "25.11";
                };
                # Enable the harnesses so the check evaluates the merged
                # hook settings and verifies that Orca itself is installed.
                programs = {
                  claude-code = {
                    enable = true;
                    package = pkgs.hello;
                  };
                  codex = {
                    enable = true;
                    package = pkgs.hello;
                  };
                  orca = {
                    enable = true;
                    integrations = {
                      claude.enable = true;
                      codex.enable = true;
                    };
                  };
                };
                assertions = [
                  {
                    assertion = lib.any (package: (package.pname or "") == "orca") config.home.packages;
                    message = "programs.orca.enable must install the Orca package";
                  }
                ];
              })
            ];
          }).activationPackage;
      });
    };
}
