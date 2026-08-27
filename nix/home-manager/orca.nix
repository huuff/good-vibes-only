{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.orca;

  # Byte-exact copies of the managed hook commands Orca installs
  # (stablyai/orca, src/main/agent-hooks + src/main/claude +
  # src/main/codex). Orca's status check compares command bytes, so
  # matching them makes Orca read these declarative entries as its own
  # installed hooks instead of trying to rewrite the Home
  # Manager-managed config files (a write it cannot perform through the
  # store symlinks). The ~/.orca/agent-hooks/*.sh scripts stay
  # Orca-owned: the app writes and refreshes them, and until it does the
  # commands fall back to draining stdin (plus a neutral '{}' for
  # Claude, whose permission hooks fail closed on empty stdout).
  # ponytail: coupled to Orca's current command format; re-derive from
  # upstream if an Orca update reports the hooks as not installed.
  drain = "{ command -p cat 2>/dev/null || cat; } >/dev/null 2>&1 || :";
  neutralFallback = "${drain}; printf '{}\\n'";

  claudeHookCommand =
    let
      posixScript = "\"\${HOME-}/.orca/agent-hooks/claude-hook.sh\"";
      winScript = "\"\${HOME-}/.orca/agent-hooks/claude-hook.cmd\"";
      powershell = "\"\${SYSTEMROOT-}/System32/WindowsPowerShell/v1.0/powershell.exe\"";
      # base64(utf16le("$ProgressPreference='SilentlyContinue'; " + the
      # PowerShell launcher for ~/.orca/agent-hooks/claude-hook.cmd)).
      encodedPowershellCommand = "JABQAHIAbwBnAHIAZQBzAHMAUAByAGUAZgBlAHIAZQBuAGMAZQA9ACcAUwBpAGwAZQBuAHQAbAB5AEMAbwBuAHQAaQBuAHUAZQAnADsAIAAkAGgAbwBtAGUAUABhAHQAaAAgAD0AIAAkAGUAbgB2ADoASABPAE0ARQAgAC0AcgBlAHAAbABhAGMAZQAgACcAXgAvACgAWwBBAC0AWgBhAC0AegBdACkALwAnACwAIAAnACQAMQA6AC8AJwA7ACAAJABzAGMAcgBpAHAAdABQAGEAdABoACAAPQAgAEoAbwBpAG4ALQBQAGEAdABoACAAJABoAG8AbQBlAFAAYQB0AGgAIAAnAC4AbwByAGMAYQBcAGEAZwBlAG4AdAAtAGgAbwBvAGsAcwBcAGMAbABhAHUAZABlAC0AaABvAG8AawAuAGMAbQBkACcAOwAgAGkAZgAgACgAVABlAHMAdAAtAFAAYQB0AGgAIAAtAEwAaQB0AGUAcgBhAGwAUABhAHQAaAAgACQAcwBjAHIAaQBwAHQAUABhAHQAaAAgAC0AUABhAHQAaABUAHkAcABlACAATABlAGEAZgApACAAewAgACYAIAAkAHMAYwByAGkAcAB0AFAAYQB0AGgAOwAgAGUAeABpAHQAIAAkAEwAQQBTAFQARQBYAEkAVABDAE8ARABFACAAfQA7ACAAWwBDAG8AbgBzAG8AbABlAF0AOgA6AEkAbgAuAFIAZQBhAGQAVABvAEUAbgBkACgAKQAgAHwAIABPAHUAdAAtAE4AdQBsAGwAOwAgAFcAcgBpAHQAZQAtAE8AdQB0AHAAdQB0ACAAJwB7AH0AJwA7ACAAZQB4AGkAdAAgADAA";
      powershellInvocation = "${powershell} -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -EncodedCommand ${encodedPowershellCommand}";
      encodedWinBranch = "if [ -f ${powershell} ]; then ${powershellInvocation}; else ${neutralFallback}; fi";
      winBranch = "if [ -f ${winScript} ]; then case \"\${HOME-}\" in *\\&*|*\\^*|*\\(*|*\\)*|*\\;*|*,*|*=*|*%*|*\\!*) ${encodedWinBranch} ;; *) ${winScript} ;; esac; else ${neutralFallback}; fi";
      posixBranch = "if [ -f ${posixScript} ] && [ -r ${posixScript} ] && [ -x ${posixScript} ]; then /bin/sh ${posixScript}; else ${neutralFallback}; fi";
    in
    "if [ -z \"\${HOME-}\" ]; then ${neutralFallback}; else case \"\${OSTYPE-}\" in msys*|cygwin*|win32*) ${winBranch} ;; *) ${posixBranch} ;; esac; fi";

  codexScript = "${config.home.homeDirectory}/.orca/agent-hooks/codex-hook.sh";
  codexHookCommand = "if [ -f '${codexScript}' ] && [ -r '${codexScript}' ] && [ -x '${codexScript}' ]; then /bin/sh '${codexScript}'; else ${drain}; fi";

  mkManagedHook = command: {
    type = "command";
    inherit command;
    timeout = 10;
  };

  # Event -> whether Orca registers it with matcher "*".
  claudeEvents = {
    SessionStart = false;
    UserPromptSubmit = false;
    Stop = false;
    StopFailure = false;
    SubagentStart = false;
    SubagentStop = false;
    TeammateIdle = false;
    PreToolUse = true;
    PostToolUse = true;
    PostToolUseFailure = true;
    PermissionRequest = true;
    PostCompact = false;
  };

  codexEvents = [
    "SessionStart"
    "UserPromptSubmit"
    "PreToolUse"
    "PermissionRequest"
    "PostToolUse"
    "SubagentStart"
    "SubagentStop"
    "Stop"
  ];

  mkIntegrationOption = name: {
    enable = lib.mkEnableOption "Orca's ${name} integration" // {
      description = ''
        Whether to register Orca's ${name} status hooks declaratively in
        the harness configuration managed by Home Manager. The hook
        commands invoke Orca-owned launcher scripts under
        {file}`~/.orca/agent-hooks/` and are silent no-ops until the
        Orca app has written them.
      '';
    };
  };
in
{
  options.programs.orca = {
    enable = lib.mkEnableOption "Orca, the agent development environment";

    package = lib.mkOption {
      type = lib.types.nullOr lib.types.package;
      default = pkgs.callPackage ../packages/orca.nix { };
      defaultText = lib.literalExpression "pkgs.callPackage ../packages/orca.nix { }";
      description = "Orca package to install. Set to null to configure integrations without installing Orca.";
    };

    integrations = {
      claude = mkIntegrationOption "Claude Code";
      codex = mkIntegrationOption "Codex";
    };
  };

  config = lib.mkIf cfg.enable {
    home.packages = lib.optional (cfg.package != null) cfg.package;

    programs.claude-code.settings.hooks = lib.mkIf cfg.integrations.claude.enable (
      lib.mapAttrs (
        _: hasMatcher:
        lib.mkAfter [
          (
            lib.optionalAttrs hasMatcher { matcher = "*"; }
            // {
              hooks = [ (mkManagedHook claudeHookCommand) ];
            }
          )
        ]
      ) claudeEvents
    );

    programs.codex = lib.mkIf cfg.integrations.codex.enable {
      settings.features.hooks = true;
      hooks = lib.genAttrs codexEvents (
        _: lib.mkAfter [ { hooks = [ (mkManagedHook codexHookCommand) ]; } ]
      );
    };
  };
}
