{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.playwright-cli;
  home = config.home.homeDirectory;
  cache = config.xdg.cacheHome;
  configDirectory = "${config.xdg.configHome}/playwright";
  cacheDirectories = [
    "${cache}/ms-playwright"
    "${cache}/fontconfig"
  ]
  ++ lib.optionals cfg.camoufox.enable [
    "${cache}/camoufox"
    "${home}/.camoufox"
  ];
  configurations = import ../camoufox/configurations.nix {
    inherit pkgs;
    camoufox-playwright = cfg.camoufox.package;
    chromiumExecutable = lib.getExe cfg.chromium.package;
    chromiumSandbox = cfg.chromium.sandbox;
  };
in
{
  options.programs.playwright-cli = {
    enable = lib.mkEnableOption "Playwright CLI with declarative browser backends";
    package = lib.mkOption {
      type = lib.types.package;
      description = "Consumer-supplied Playwright CLI package, retaining the consumer's version pin.";
    };
    chromium = {
      enable = lib.mkEnableOption "the Chromium backend";
      package = lib.mkPackageOption pkgs "chromium" { };
      sandbox = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Enable Playwright's Chromium sandbox. NixOS setuid-helper configuration remains consumer-owned.";
      };
    };
    camoufox = {
      enable = lib.mkEnableOption "the experimental Camoufox backend";
      package = lib.mkOption {
        type = lib.types.package;
        default = pkgs.callPackage ../packages/camoufox-playwright.nix { };
        defaultText = lib.literalExpression "good-vibes-only's camoufox-playwright package";
        description = "Direct-launch adapter, which already includes the browser in its closure.";
      };
    };
    filesystem = {
      read = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        readOnly = true;
        default = lib.optionals cfg.enable [
          builtins.storeDir
          "${home}/.playwright"
          configDirectory
        ];
        defaultText = lib.literalExpression "paths derived from the enabled backends and Home Manager directories";
        description = "Required config and Nix store read paths for consumer sandbox profiles. Empty when disabled.";
      };
      write = lib.mkOption {
        type = lib.types.listOf lib.types.str;
        readOnly = true;
        default = lib.optionals cfg.enable (
          cacheDirectories ++ [ "/tmp" ] ++ lib.optional pkgs.stdenv.hostPlatform.isLinux "/dev/shm"
        );
        defaultText = lib.literalExpression "paths derived from the enabled backends and Home Manager directories";
        description = ''
          Required read/write cache, application, temporary and shared-memory paths.
          Consumers must also permit their working directory, any custom profiles,
          and host-specific display/device and Firefox namespace operations.
          This list alone does not establish nono runtime compatibility.
        '';
      };
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.chromium.enable || cfg.camoufox.enable;
        message = "programs.playwright-cli requires at least one enabled browser backend.";
      }
      {
        assertion =
          !cfg.camoufox.enable || lib.meta.availableOn pkgs.stdenv.hostPlatform cfg.camoufox.package;
        message = "programs.playwright-cli.camoufox.package must support the host platform (the bundled adapter supports x86_64-linux).";
      }
    ];

    xdg = {
      enable = lib.mkDefault true;
      configFile = lib.mkMerge [
        (lib.mkIf cfg.chromium.enable { "playwright/chromium.json".source = configurations.chromium; })
        (lib.mkIf cfg.camoufox.enable { "playwright/camoufox.json".source = configurations.camoufox; })
      ];
    };
    home = {
      # Ensure the CLI and SDK use the configured cache location at runtime.
      sessionVariables.XDG_CACHE_HOME = config.xdg.cacheHome;
      packages = [
        cfg.package
      ]
      ++ lib.optional cfg.chromium.enable cfg.chromium.package
      ++ lib.optional cfg.camoufox.enable cfg.camoufox.package;
      file.".playwright/cli.config.json".source =
        if cfg.chromium.enable then configurations.chromium else configurations.camoufox;
      activation.playwrightCliDirectories = lib.hm.dag.entryAfter [ "writeBoundary" ] ''
        (
          umask 077
          run ${pkgs.coreutils}/bin/mkdir -p -m 700 -- ${
            lib.escapeShellArgs (
              [
                "${home}/.playwright"
                configDirectory
              ]
              ++ cacheDirectories
            )
          }
        )
      '';
    };
  };
}
