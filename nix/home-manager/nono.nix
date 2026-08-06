{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.nono;
  jsonFormat = pkgs.formats.json { };

  wrapperBin =
    name: wrapper:
    pkgs.writeShellScriptBin name ''
      ${lib.optionalString wrapper.allowGitCommonDir ''
        gitFlags=()
        if gitCommonDir=$(${lib.getExe pkgs.git} rev-parse --path-format=absolute --git-common-dir 2>/dev/null); then
          gitFlags=(--allow "$gitCommonDir")

          # Cargo searches every ancestor of the current directory for
          # .cargo/config.toml. For a worktree nested inside its main
          # checkout, that search enters the otherwise-blocked main checkout
          # before reaching the filesystem root. Make that ancestor readable,
          # while retaining read+write access only for Git's shared metadata.
          if gitWorktree=$(${lib.getExe pkgs.git} rev-parse --path-format=absolute --show-toplevel 2>/dev/null); then
            gitMainWorktree="''${gitCommonDir%/*}"
            if [[ "''${gitCommonDir##*/}" == .git && "$gitMainWorktree" != / && "$gitWorktree" != "$gitMainWorktree" ]]; then
              case "$gitWorktree/" in
                "$gitMainWorktree/"*) gitFlags+=(--read "$gitMainWorktree") ;;
              esac
            fi
          fi
        fi
      ''}
      exec ${lib.getExe cfg.package} run \
        --profile ${lib.escapeShellArg wrapper.profile} \
        ${lib.escapeShellArgs wrapper.extraFlags} ${lib.optionalString wrapper.allowGitCommonDir ''"''${gitFlags[@]}"''} \
        -- ${wrapper.command} "$@"
    '';
in
{
  options.programs.nono = {
    enable = lib.mkEnableOption "nono, a kernel-enforced sandbox for AI agents";

    package = lib.mkPackageOption pkgs "nono" { };

    profiles = lib.mkOption {
      type = lib.types.attrsOf jsonFormat.type;
      default = { };
      example = lib.literalExpression ''
        {
          rust-dev = {
            extends = "claude-code";
            filesystem.read = [ "~/references" ];
            policy.add_deny_access = [ "~/.config/sops" ];
          };
        }
      '';
      description = ''
        Sandbox profiles, written to
        {file}`$XDG_CONFIG_HOME/nono/profiles/<name>.json`. Each value is
        serialized verbatim — the schema is nono's, not this module's, so
        fields added upstream need no module changes. `meta.name` defaults
        to the attribute name. Prefer extending a built-in profile via
        `extends` over redefining one from scratch; the built-in baselines
        change frequently between nono releases.
      '';
    };

    wrappers = lib.mkOption {
      type = lib.types.attrsOf (
        lib.types.submodule {
          options = {
            profile = lib.mkOption {
              type = lib.types.str;
              default = "claude-code";
              description = ''
                Profile to sandbox with: built-in, or one declared in
                {option}`programs.nono.profiles`.
              '';
            };

            command = lib.mkOption {
              type = lib.types.str;
              example = "claude --dangerously-skip-permissions";
              description = ''
                Command line executed inside the sandbox. Interpolated into
                the wrapper script verbatim, with the wrapper's own
                arguments appended.
              '';
            };

            allowGitCommonDir = lib.mkOption {
              type = lib.types.bool;
              default = false;
              description = ''
                Grant read+write access to `git rev-parse --git-common-dir`
                at launch, so agents running in a linked worktree can reach
                the shared `.git` directory. If the linked worktree is nested
                inside the main checkout, also grant read-only access to the
                main checkout so ancestor-based config discovery (such as
                Cargo's) does not fail. No-op outside a git repository; in a
                main worktree the common dir is already inside the CWD.
              '';
            };

            extraFlags = lib.mkOption {
              type = lib.types.listOf lib.types.str;
              default = [ ];
              example = [
                "--allow-cwd"
                "--allow-file"
                "/nix/var/nix/daemon-socket/socket"
              ];
              description = "Extra flags passed to {command}`nono run`.";
            };
          };
        }
      );
      default = { };
      example = lib.literalExpression ''
        {
          claude-sandboxed = {
            command = "claude --dangerously-skip-permissions";
            extraFlags = [ "--allow-cwd" ];
          };
        }
      '';
      description = ''
        Convenience scripts, one binary per attribute, that exec their
        command inside a nono sandbox.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ] ++ lib.mapAttrsToList wrapperBin cfg.wrappers;

    xdg.configFile = lib.mapAttrs' (
      name: profile:
      lib.nameValuePair "nono/profiles/${name}.json" {
        source = jsonFormat.generate "nono-profile-${name}.json" (
          lib.recursiveUpdate { meta.name = name; } profile
        );
      }
    ) cfg.profiles;
  };
}
