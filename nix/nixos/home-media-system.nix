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
    osd = {
      client = "${pkgs.swayosd}/bin/swayosd-client";
      inherit (cfg) homeHint homeHintDurationMs;
    };
    audio.client = "${pkgs.wireplumber}/bin/wpctl";
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
  mediaAction = pkgs.writeShellScript "home-media-system-media-action" ''
    media_uid="$(${pkgs.coreutils}/bin/id -u ${lib.escapeShellArg cfg.user})"
    runtime_dir="/run/user/$media_uid"
    test -S "$runtime_dir/bus" || exit 0
    exec ${pkgs.util-linux}/bin/runuser -u ${lib.escapeShellArg cfg.user} -- \
      ${pkgs.coreutils}/bin/env \
        HOME=${lib.escapeShellArg homeDirectory} \
        XDG_RUNTIME_DIR="$runtime_dir" \
        DBUS_SESSION_BUS_ADDRESS="unix:path=$runtime_dir/bus" \
        PATH=${lib.escapeShellArg (lib.makeBinPath [ pkgs.playerctl ])} \
        ${pkgs.swayosd}/bin/swayosd-client "$@"
  '';
  osdStyle = pkgs.writeText "home-media-system-swayosd.css" ''
    window#osd {
      border: 1px solid #5c6075;
      border-radius: 9px;
      background: rgba(37, 39, 47, 0.96);
      box-shadow: 0 16px 42px rgba(0, 0, 0, 0.38);
      color: #ececf3;
      font-family: Inter, sans-serif;
      font-size: 18px;
      font-weight: 500;
    }

    window#osd #container {
      margin: 18px 22px;
    }

    window#osd image,
    window#osd label {
      color: #ececf3;
    }

    window#osd progressbar:disabled,
    window#osd image:disabled {
      opacity: 0.5;
    }

    window#osd progressbar,
    window#osd segmentedprogress {
      min-height: 7px;
      border: none;
      border-radius: 4px;
      background: transparent;
    }

    window#osd trough,
    window#osd segment {
      min-height: inherit;
      border: none;
      border-radius: inherit;
      background: #404354;
    }

    window#osd progress,
    window#osd segment.active {
      min-height: inherit;
      border: none;
      border-radius: inherit;
      background: #aa8cf3;
    }

    window#osd segment {
      margin-left: 8px;
    }

    window#osd segment:first-child {
      margin-left: 0;
    }
  '';
  launcher = pkgs.writeShellScript "home-media-system-launcher" ''
    ${pkgs.swayosd}/bin/swayosd-server --style ${osdStyle} &
    osd_pid=$!
    trap 'kill "$osd_pid" 2>/dev/null || true' EXIT
    ${lib.getExe cfg.package} --config ${settingsFile}
    ${pkgs.sway}/bin/swaymsg exit
  '';
  swayConfig = pkgs.writeText "home-media-system-sway.conf" ''
    default_border none
    default_floating_border none
    focus_follows_mouse no
    output * bg "#090b14" solid_color
    seat * hide_cursor 3000
    xwayland enable

    for_window [app_id=".*"] fullscreen enable
    for_window [class=".*"] fullscreen enable

    exec ${launcher}
  '';
  session = pkgs.writeShellScriptBin "home-media-system-session" ''
    export XDG_CONFIG_HOME=${lib.escapeShellArg "${homeDirectory}/.config"}
    export XDG_CACHE_HOME=${lib.escapeShellArg "${homeDirectory}/.cache"}
    export XDG_STATE_HOME=${lib.escapeShellArg "${homeDirectory}/.local/state"}
    exec ${lib.getExe pkgs.sway} --config ${swayConfig}
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

    homeHint = lib.mkOption {
      type = lib.types.nullOr lib.types.str;
      default = "Press Home to go back to Home";
      description = ''
        SwayOSD message shown whenever an application opens. Set this to null
        to disable the hint.
      '';
    };

    homeHintDurationMs = lib.mkOption {
      type = lib.types.ints.between 1000 10000;
      default = 4000;
      description = "Approximate time in milliseconds to keep the Home hint visible.";
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
      pkgs.sway
      pkgs.swayosd
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

      # Triggerhappy reads Linux input events independently of application
      # focus, providing consistent Home and media controls in native and
      # embedded applications. HOMEPAGE covers remotes that expose their Home
      # button under the media key code.
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
          {
            keys = [ "VOLUMEUP" ];
            cmd = "${mediaAction} --output-volume raise";
          }
          {
            keys = [ "VOLUMEDOWN" ];
            cmd = "${mediaAction} --output-volume lower";
          }
          {
            keys = [ "MUTE" ];
            cmd = "${mediaAction} --output-volume mute-toggle";
          }
          {
            keys = [ "MICMUTE" ];
            cmd = "${mediaAction} --input-volume mute-toggle";
          }
          {
            keys = [ "PLAYPAUSE" ];
            cmd = "${mediaAction} --playerctl play-pause";
          }
          {
            keys = [ "NEXTSONG" ];
            cmd = "${mediaAction} --playerctl next";
          }
          {
            keys = [ "PREVIOUSSONG" ];
            cmd = "${mediaAction} --playerctl prev";
          }
          {
            keys = [ "STOPCD" ];
            cmd = "${mediaAction} --playerctl stop";
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
