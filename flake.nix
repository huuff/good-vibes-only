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
    in
    {
      packages = forAllSystems (pkgs: {
        default = pkgs.callPackage ./nix/package.nix { };
      });

      overlays.default = final: _prev: {
        good-vibes-only = final.callPackage ./nix/package.nix { };
      };

      # Every nix/home-manager/<name>.nix is exported as
      # homeManagerModules.<name>, mirroring how crates/* feeds the cargo
      # workspace: drop a file in, no flake edits needed.
      homeManagerModules =
        lib.mapAttrs'
          (
            fileName: _:
            lib.nameValuePair (lib.removeSuffix ".nix" fileName) (./nix/home-manager + "/${fileName}")
          )
          (
            lib.filterAttrs (n: t: t == "regular" && lib.hasSuffix ".nix" n) (
              builtins.readDir ./nix/home-manager
            )
          );

      homeModules = self.homeManagerModules;

      checks = forAllSystems (pkgs: {
        package = self.packages.${pkgs.stdenv.hostPlatform.system}.default;

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
      });
    };
}
