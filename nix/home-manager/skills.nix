{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.agent-skills;
  skillNames = builtins.attrNames (
    lib.filterAttrs (_: type: type == "directory") (builtins.readDir ../../skills)
  );

  mkHarnessOption =
    name: defaultDirectory:
    lib.mkOption {
      default = { };
      description = "Skill installation for ${name}.";
      type = lib.types.submodule {
        options = {
          enable = lib.mkEnableOption "installing skills for ${name}";

          directory = lib.mkOption {
            type = lib.types.str;
            default = defaultDirectory;
            description = "Skill directory relative to the home directory.";
          };
        };
      };
    };

  enabledDirectories = map (harness: harness.directory) (
    lib.filter (harness: harness.enable) [
      cfg.claude-code
      cfg.codex
      cfg.opencode
    ]
  );
in
{
  options.programs.agent-skills = {
    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.callPackage ../packages/skills.nix { };
      defaultText = lib.literalExpression "good-vibes-only's skills package";
      description = "Package containing the skills to install.";
    };

    claude-code = mkHarnessOption "Claude Code" ".claude/skills";
    codex = mkHarnessOption "Codex" ".agents/skills";
    opencode = mkHarnessOption "OpenCode" ".config/opencode/skills";
  };

  config.home.file = lib.listToAttrs (
    lib.concatMap (
      directory:
      map (name: {
        name = "${directory}/${name}";
        value.source = "${cfg.package}/share/skills/${name}";
      }) skillNames
    ) enabledDirectories
  );
}
