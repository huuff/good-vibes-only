{
  description = "A cargo workspace hosting small vibe-coded Rust projects";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      home-manager,
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

      extraPackages = pkgs: lib.mapAttrs (_: f: pkgs.callPackage f { }) (nixFilesIn ./nix/packages);

      # One package per workspace crate, built with `cargo build -p <crate>`.
      crates = lib.attrNames (lib.filterAttrs (_: t: t == "directory") (builtins.readDir ./crates));
      cratePackages =
        pkgs: lib.genAttrs crates (crate: pkgs.callPackage ./nix/package.nix { inherit crate; });
    in
    {
      packages = forAllSystems (pkgs: cratePackages pkgs // extraPackages pkgs);

      overlays.default = final: _prev: cratePackages final // extraPackages final;

      # Every nix/home-manager/<name>.nix is exported as
      # homeManagerModules.<name>.
      homeManagerModules = nixFilesIn ./nix/home-manager;

      homeModules = self.homeManagerModules;

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
