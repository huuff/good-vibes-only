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
  enabledIntegrations = lib.filter (name: cfg.integrations.${name}.enable) integrationNames;
  integrationsFile = "${config.xdg.stateHome}/herdr/home-manager-integrations";
in
{
  options.programs.herdr.integrations = lib.genAttrs integrationNames (name: {
    enable = lib.mkEnableOption "Herdr's ${name} integration" // {
      description = ''
        Whether to install Herdr's ${name} integration during Home Manager
        activation. Herdr writes its hook or plugin into the agent's
        configuration directory, which must already exist. Disabling an
        integration uninstalls it if it was previously managed by this module;
        integrations installed by other means are left alone.
      '';
    };
  });

  config = lib.mkIf cfg.enable {
    home.activation.herdrIntegrations = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
      integrationsFile=${lib.escapeShellArg integrationsFile}
      currentIntegrations=${lib.escapeShellArg " ${lib.concatStringsSep " " enabledIntegrations} "}
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
      '') enabledIntegrations}

      newIntegrationsFile="$(mktemp "''${integrationsFile}.XXXXXX")"
      ${lib.concatMapStringsSep "\n" (integration: ''
        printf '%s\n' ${lib.escapeShellArg integration} >> "$newIntegrationsFile"
      '') enabledIntegrations}
      mv "$newIntegrationsFile" "$integrationsFile"
    '';
  };
}
