{
  config,
  lib,
  ...
}:

let
  cfg = config.programs.herdr;
  herdr = if cfg.package == null then "herdr" else lib.getExe cfg.package;
  integrationNames = [
    "pi"
    "omp"
    "claude"
    "codex"
    "copilot"
    "devin"
    "droid"
    "kimi"
    "opencode"
    "kilo"
    "hermes"
    "qodercli"
    "cursor"
    "mastracode"
    "antigravity-cli"
    "grok"
  ];
  integrationType = lib.types.enum integrationNames;
  integrationsFile = "${config.xdg.stateHome}/herdr/home-manager-integrations";
in
{
  options.programs.herdr.integrations = lib.mkOption {
    type = lib.types.listOf integrationType;
    default = [ ];
    example = [
      "claude"
      "codex"
      "opencode"
    ];
    description = ''
      Agent integrations to install with {command}`herdr integration
      install` during Home Manager activation. Herdr writes these hooks or
      plugins into each agent's configuration directory, which must already
      exist. Integrations removed from this list are uninstalled if they
      were previously managed by this module; integrations installed by
      other means are left alone.
    '';
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = lib.length cfg.integrations == lib.length (lib.unique cfg.integrations);
        message = "programs.herdr.integrations must not contain duplicates";
      }
    ];

    home.activation.herdrIntegrations = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
      integrationsFile=${lib.escapeShellArg integrationsFile}
      currentIntegrations=${lib.escapeShellArg " ${lib.concatStringsSep " " cfg.integrations} "}
      mkdir -p "$(dirname "$integrationsFile")"

      if [[ -f "$integrationsFile" ]]; then
        while IFS= read -r integration; do
          [[ -z "$integration" ]] && continue
          case "$currentIntegrations" in
            *" $integration "*) ;;
            *) ${herdr} integration uninstall "$integration" ;;
          esac
        done < "$integrationsFile"
      fi

      ${lib.concatMapStringsSep "\n" (integration: ''
        ${herdr} integration install ${lib.escapeShellArg integration}
      '') cfg.integrations}

      newIntegrationsFile="$(mktemp "''${integrationsFile}.XXXXXX")"
      ${lib.concatMapStringsSep "\n" (integration: ''
        printf '%s\n' ${lib.escapeShellArg integration} >> "$newIntegrationsFile"
      '') cfg.integrations}
      mv "$newIntegrationsFile" "$integrationsFile"
    '';
  };
}
