{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.ccstatusline;
  jsonFormat = pkgs.formats.json { };
in
{
  options.programs.ccstatusline = {
    enable = lib.mkEnableOption "ccstatusline, a status line formatter for Claude Code";

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.callPackage ../packages/ccstatusline.nix { };
      defaultText = lib.literalExpression "good-vibes-only's ccstatusline package";
      description = "The ccstatusline package to use.";
    };

    settings = lib.mkOption {
      inherit (jsonFormat) type;
      default = { };
      example = lib.literalExpression ''
        {
          lines = [
            [
              { id = "model"; type = "model"; }
              { id = "branch"; type = "git-branch"; }
              { id = "ctx"; type = "context-length"; }
            ]
          ];
          flexMode = "full-minus-40";
        }
      '';
      description = ''
        Written verbatim to
        {file}`$XDG_CONFIG_HOME/ccstatusline/settings.json` — the schema is
        ccstatusline's, not this module's, so fields added upstream need no
        module changes. Partial settings are safe: ccstatusline fills every
        missing field with its built-in default when it loads the file, so
        overriding one key never breaks the rest. Leave empty to let the
        `ccstatusline` TUI manage the file instead (a declarative file is
        read-only, so the TUI can't save over it).
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = [ cfg.package ];

    xdg.configFile."ccstatusline/settings.json" = lib.mkIf (cfg.settings != { }) {
      # A file without `version` is treated as a legacy config: ccstatusline
      # migrates it and writes the result over the symlink. 3 matches the
      # packaged 2.2.27; bump alongside nix/packages/ccstatusline.nix.
      source = jsonFormat.generate "ccstatusline-settings.json" (
        lib.recursiveUpdate { version = 3; } cfg.settings
      );
    };

    # Lands in ~/.claude/settings.json only when programs.claude-code is
    # enabled; otherwise point Claude Code at `command` yourself.
    programs.claude-code.settings.statusLine = {
      type = "command";
      command = lib.getExe cfg.package;
      padding = 0;
    };
  };
}
