# Home media system

`nixosModules.home-media-system` turns a NixOS machine into a dedicated
ten-foot media appliance. It creates an unprivileged media user and uses greetd
to log directly into a minimal Sway session containing only the launcher.
Sway supplies the layer-shell support used by the on-screen display. Jellyfin
cards launch the official Jellyfin Desktop client; Jellyseerr, YouTube, and
generic web applications run in the launcher's persistent Chromium profile.

Every application opens fullscreen. Pressing the keyboard or remote Home key
closes a native application or leaves an embedded web application and returns
to the launcher. Escape or Browser Back also returns from embedded web apps.
When an application opens, SwayOSD displays a reminder that Home returns to the
launcher. Hardware volume, mute, play/pause, previous, next, and stop keys work
globally and use the same overlay for feedback. The launcher header also shows
the current output volume and mute state. Arrow keys move focus between cards
and power actions.

## Configuration

```nix
{
  inputs.good-vibes-only.url = "path:/path/to/good-vibes-only";
  inputs.sops-nix.url = "github:Mic92/sops-nix";

  outputs =
    inputs@{ nixpkgs, ... }:
    {
      nixosConfigurations.media-box = nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          inputs.sops-nix.nixosModules.sops
          inputs.good-vibes-only.nixosModules.home-media-system
          ./hardware-configuration.nix
          ({ config, ... }:
          {
            system.stateVersion = "25.11";

            sops.defaultSopsFile = ./secrets.yaml;
            sops.age.keyFile = "/var/lib/sops-nix/key.txt";
            sops.secrets = {
              jellyseerr-username.owner = "media";
              jellyseerr-password.owner = "media";
            };

            services.home-media-system = {
              enable = true;
              user = "media";
              locale = "en-GB";
              # Set to null to hide the app-opening reminder.
              homeHint = "Press Home to go back to Home";
              homeHintDurationMs = 4000;

              applications = {
                jellyfin = {
                  type = "jellyfin";
                  order = 10;
                };

                jellyseerr = {
                  type = "jellyseerr";
                  url = "https://requests.example.net";
                  order = 20;
                  autoLogin = {
                    enable = true;
                    usernameFile = config.sops.secrets.jellyseerr-username.path;
                    passwordFile = config.sops.secrets.jellyseerr-password.path;
                  };
                };

                youtube = {
                  type = "youtube";
                  url = "https://www.youtube.com/tv";
                  order = 30;
                };
              };
            };
          })
        ];
      };
    };
}
```

Jellyfin's `url` setting is ignored because the official client provides its
own server selection and authentication. It remembers the selected server and
session in the media user's profile. For web applications such as Jellyseerr,
`url` is optional: without it, the launcher asks for a server address and
remembers it locally. Set `url` declaratively when the appliance should always
use one managed web server.

Only secret *paths* are placed in the generated launcher configuration. The
launcher reads their contents at login time and injects them into the matching
login form; the contents never enter the Nix store. The sops-nix secrets must be
readable by the configured media user, hence the `owner` settings above.

Web login sessions persist under the media user's state directory, so
automation normally runs only on the first launch or after a session expires.
If an installation customizes its web login page, set `usernameSelector`,
`passwordSelector`, and `submitSelector` under that application's `autoLogin`.
Jellyfin Desktop manages its own login and persistent session rather than using
the launcher's form automation.

The module also enables graphics, PipeWire audio, RTKit, Polkit, and SwayOSD.
Normal logind policy lets the active local media user suspend, reboot, and power
off from the power menu.

## Demo VM

On an x86_64 Linux host:

```console
nix run .#home-media-system-vm
```

The first run builds a NixOS QEMU VM, Electron, and Jellyfin Desktop, so it can
take a while. The VM boots straight into the launcher with example cards. It
uses 3 GiB RAM, two virtual CPUs, and QEMU user networking. Power actions affect
only the VM.
