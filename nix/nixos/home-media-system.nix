{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.home-media-system;
  jsonFormat = pkgs.formats.json { };

  defaultDescriptions = {
    jellyfin = "Movies & TV from the home server";
    jellyseerr = "Request new movies and shows";
    youtube = "Videos, subscriptions, casts";
    web = "Open web application";
  };

  applicationType = lib.types.submodule (
    { name, config, ... }:
    {
      options = {
        name = lib.mkOption {
          type = lib.types.str;
          default = name;
          description = "Label displayed on the application card.";
        };

        type = lib.mkOption {
          type = lib.types.enum [
            "jellyfin"
            "jellyseerr"
            "youtube"
            "web"
          ];
          default = "web";
          description = "Application kind, used to select its built-in icon.";
        };

        url = lib.mkOption {
          type = lib.types.nullOr (lib.types.strMatching "https?://.*");
          default = null;
          example = "https://jellyfin.example.net";
          description = ''
            URL opened when this card is selected. When null, the launcher
            asks for a server address at runtime and remembers it locally.
            Jellyfin cards use the official client and ignore this option.
          '';
        };

        description = lib.mkOption {
          type = lib.types.str;
          default = defaultDescriptions.${config.type};
          defaultText = lib.literalExpression "a description appropriate for the application type";
          description = "Short text shown below the application name.";
        };

        order = lib.mkOption {
          type = lib.types.int;
          default = 100;
          description = "Card order; lower values appear first.";
        };

        autoLogin = {
          enable = lib.mkEnableOption "runtime form-based login for this application";

          usernameFile = lib.mkOption {
            type = lib.types.nullOr lib.types.str;
            default = null;
            example = "/run/secrets/jellyfin-username";
            description = ''
              Runtime path containing the username. Point this at a sops-nix
              secret path; its contents are never copied into the Nix store.
            '';
          };

          passwordFile = lib.mkOption {
            type = lib.types.nullOr lib.types.str;
            default = null;
            example = "/run/secrets/jellyfin-password";
            description = ''
              Runtime path containing the password. Point this at a sops-nix
              secret path; its contents are never copied into the Nix store.
            '';
          };

          usernameSelector = lib.mkOption {
            type = lib.types.str;
            default = ''input[name="username"], input[name="email"], input[type="email"], #txtManualName'';
            description = "CSS selector used to locate the login name field.";
          };

          passwordSelector = lib.mkOption {
            type = lib.types.str;
            default = ''input[name="password"], input[type="password"], #txtManualPassword'';
            description = "CSS selector used to locate the password field.";
          };

          submitSelector = lib.mkOption {
            type = lib.types.str;
            default = ''button[type="submit"], .btnSubmit, form button'';
            description = "CSS selector used to locate the login button.";
          };
        };
      };
    }
  );

  orderedApplications = builtins.sort (
    left: right:
    if left.value.order == right.value.order then
      left.name < right.name
    else
      left.value.order < right.value.order
  ) (lib.attrsToList cfg.applications);

  renderedApplications = map (
    entry:
    let
      application = entry.value;
    in
    {
      id = entry.name;
      inherit (application)
        name
        type
        url
        description
        order
        ;
      nativeCommand = lib.optionalString (application.type == "jellyfin") (
        lib.getExe cfg.jellyfinPackage
      );
      nativeArgs = lib.optionals (application.type == "jellyfin") [
        "--fullscreen"
        "--tv"
      ];
      autoLogin = {
        inherit (application.autoLogin)
          enable
          usernameFile
          passwordFile
          usernameSelector
          passwordSelector
          submitSelector
          ;
      };
    }
  ) orderedApplications;

  settingsFile = jsonFormat.generate "home-media-system.json" {
    inherit (cfg)
      title
      locale
      clock
      fullscreen
      kiosk
      power
      ;
    applications = renderedApplications;
  };

  homeDirectory = config.users.users.${cfg.user}.home;
  launcherPidFile = "${homeDirectory}/.local/state/home-media-system/launcher.pid";
  returnHome = pkgs.writeShellScript "home-media-system-return-home" ''
    pid_file=${lib.escapeShellArg launcherPidFile}
    test -r "$pid_file" || exit 0
    read -r launcher_pid < "$pid_file"
    case "$launcher_pid" in
      *[!0-9]*) exit 0 ;;
    esac
    test -n "$launcher_pid" || exit 0
    test -d "/proc/$launcher_pid" || exit 0
    test "$(${pkgs.coreutils}/bin/stat -c %U "/proc/$launcher_pid")" = ${lib.escapeShellArg cfg.user} || exit 0
    kill -USR1 "$launcher_pid"
  '';
  session = pkgs.writeShellScriptBin "home-media-system-session" ''
    export XDG_CONFIG_HOME=${lib.escapeShellArg "${homeDirectory}/.config"}
    export XDG_CACHE_HOME=${lib.escapeShellArg "${homeDirectory}/.cache"}
    export XDG_STATE_HOME=${lib.escapeShellArg "${homeDirectory}/.local/state"}
    exec ${lib.getExe pkgs.cage} -- \
      ${lib.getExe cfg.package} --config ${settingsFile}
  '';
in
{
  options.services.home-media-system = {
    enable = lib.mkEnableOption "a boot-to-launcher home media system";

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.callPackage ../packages/home-media-system.nix { };
      defaultText = lib.literalExpression "good-vibes-only's home-media-system package";
      description = "Launcher package to use.";
    };

    jellyfinPackage = lib.mkOption {
      type = lib.types.package;
      default = pkgs.jellyfin-media-player;
      defaultText = lib.literalExpression "pkgs.jellyfin-media-player";
      description = "Official Jellyfin desktop client launched by Jellyfin cards.";
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "media";
      description = "Unprivileged user automatically logged into the media session.";
    };

    title = lib.mkOption {
      type = lib.types.str;
      default = "Home media";
      description = "Window title.";
    };

    locale = lib.mkOption {
      type = lib.types.str;
      default = "en-GB";
      example = "es-ES";
      description = "Locale used by the date and clock.";
    };

    clock = lib.mkOption {
      type = lib.types.enum [
        "12h"
        "24h"
      ];
      default = "24h";
      description = "Clock format.";
    };

    fullscreen = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Open the launcher fullscreen.";
    };

    kiosk = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Use Chromium kiosk mode, preventing the window from being closed normally.";
    };

    applications = lib.mkOption {
      type = lib.types.attrsOf applicationType;
      default = { };
      example = lib.literalExpression ''
        {
          jellyfin = {
            type = "jellyfin";
            order = 10;
          };
          youtube = {
            type = "youtube";
            url = "https://youtube.com/tv";
            order = 30;
          };
        }
      '';
      description = "Applications displayed on the launcher home screen.";
    };

    power = {
      sleep = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Show the suspend action.";
      };
      restart = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Show the reboot action.";
      };
      poweroff = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Show the power-off action.";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = lib.flatten (
      lib.mapAttrsToList (
        name: application:
        lib.optional application.autoLogin.enable {
          assertion =
            application.autoLogin.usernameFile != null && application.autoLogin.passwordFile != null;
          message = "services.home-media-system.applications.${name}: autoLogin requires usernameFile and passwordFile";
        }
        ++ lib.optional (application.type == "jellyfin" && application.autoLogin.enable) {
          assertion = false;
          message = "services.home-media-system.applications.${name}: Jellyfin uses the official desktop client, which manages its own persistent login; autoLogin is only available for web applications";
        }
      ) cfg.applications
    );

    users.users.${cfg.user} = {
      isNormalUser = true;
      description = "Home media system";
      extraGroups = [
        "audio"
        "video"
      ];
    };

    environment.systemPackages = [
      cfg.package
      cfg.jellyfinPackage
      session
    ];

    programs.dconf.enable = true;
    security.polkit.enable = true;
    security.rtkit.enable = true;
    hardware.graphics.enable = true;

    services = {
      pipewire = {
        enable = true;
        alsa.enable = true;
        pulse.enable = true;
      };

      # Cage intentionally has no global keybinding facility. Triggerhappy reads
      # input events independently of application focus and signals the launcher,
      # so Home works identically in native and embedded applications. HOMEPAGE
      # covers remotes that expose their Home button under the media key code.
      triggerhappy = {
        enable = true;
        user = "root";
        bindings = [
          {
            keys = [ "HOME" ];
            cmd = toString returnHome;
          }
          {
            keys = [ "HOMEPAGE" ];
            cmd = toString returnHome;
          }
        ];
      };

      greetd = {
        enable = true;
        settings.default_session = {
          inherit (cfg) user;
          command = lib.getExe session;
        };
      };
    };
  };
}
