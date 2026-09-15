# Camoufox through playwright-cli

**Experimental: full browser compatibility is not yet verified.** The current
agent sandbox blocks Firefox content processes; see the validation report below.

Target: `x86_64-linux`. Chromium remains the recommended default for development
and testing; select Camoufox for external browsing where fingerprint handling is
useful. These packages do not install a systemd service, REST server, or TCP
listener. Playwright starts the browser with `-juggler-pipe`; the adapter uses
`execve`, retaining the PID, inherited communication descriptors, and signals.
Named sessions use playwright-cli's own session machinery.

## Packages and pins

```sh
nix build github:huuff/good-vibes-only#camoufox
nix build github:huuff/good-vibes-only#camoufox-playwright
```

- Browser: [152.0.4-beta.30](https://github.com/daijro/camoufox/releases/tag/v152.0.4-beta.30),
  fixed SHA-256 release archive; full bundle under `lib/camoufox`, with the
  `version.json` metadata normally created by the SDK installer added at build time.
  Upstream policies disabling browser, system-addon and extension updates are retained.
- SDK: [Python 0.5.6](https://pypi.org/project/camoufox/0.5.6/), fixed source hash
  in `sdk.nix`. Dependencies come from this repository's locked nixpkgs,
  including BrowserForge 1.2.4 and Python Playwright 1.61.0.
- Controller under validation: playwright-cli 0.1.19, bundled JavaScript
  Playwright 1.63.0-alpha-2026-08-31. The Python dependency does **not** control
  the browser or establish compatibility with this controller.

The SDK's `<1.63` requirement is retained. A small patch skips its unnecessary
shared-memory lock when every default extension is excluded. Its launch-settings generator is used
with the real bundle executable, Firefox major 152, no default extensions, and
GeoIP disabled. BrowserForge's packaged fingerprint data and Camoufox's bundled
fonts eliminate runtime asset downloads. Do not run `camoufox fetch`.

## Consumer nix-config changes

Keep the existing pinned playwright-cli package. Consume the new packages from
this flake; do not replace the CLI's Playwright dependency with the Python SDK's
version. Example within the consumer's Home Manager configuration:

```nix
let
  vibes = inputs.good-vibes-only.packages.${pkgs.stdenv.hostPlatform.system};
  configs = import "${inputs.good-vibes-only}/nix/camoufox/configurations.nix" {
    inherit pkgs;
    inherit (vibes) camoufox-playwright;
    chromiumExecutable = "${yourExistingChromium}/bin/chromium";
    chromiumSandbox = yourExistingChromiumSandboxSetting;
  };
in {
  home.packages = [ vibes.camoufox vibes.camoufox-playwright ];
  home.file.".playwright/cli.config.json".source = configs.chromium;
  xdg.configFile."playwright/chromium.json".source = configs.chromium;
  xdg.configFile."playwright/camoufox.json".source = configs.camoufox;
}
```

The Chromium executable and sandbox boolean above are required consumer inputs:
copy the existing values, including its setuid-helper setup. Firefox's sandbox
is separate; the adapter does not set `MOZ_DISABLE_CONTENT_SANDBOX` or reduce
sandbox preferences.

Remove the global `PLAYWRIGHT_MCP_EXECUTABLE_PATH` assignment and put the Chromium
executable into `~/.playwright/cli.config.json` (as above), so plain
`playwright-cli open` still defaults to Chromium. That environment variable takes
precedence over configuration-file executable paths. Until it is removed, use
`env -u PLAYWRIGHT_MCP_EXECUTABLE_PATH` when opening Camoufox.

```sh
playwright-cli -s=chrome open --config="$HOME/.config/playwright/chromium.json"
env -u PLAYWRIGHT_MCP_EXECUTABLE_PATH playwright-cli -s=camoufox open \
  --config="$HOME/.config/playwright/camoufox.json"
playwright-cli -s=camoufox snapshot
playwright-cli -s=camoufox click e12
playwright-cli -s=camoufox close
playwright-cli -s=chrome close
```

Choose the backend when opening a session. For persistent logins, pass a distinct
`--profile=/writable/path` for each backend; never share Firefox and Chromium
profile directories. `viewport: null` leaves window dimensions to the browser
instead of applying Playwright's fixed viewport. The SDK generates a fresh
fingerprint on every launch, including when reusing a persistent profile.

## Preferences and lifecycle

In this exact controller, the Firefox Juggler backend does **not** create
`user.js` before launch. It sends `firefoxUserPrefs` through `Browser.enable`
after launch. The adapter creates `user.js` as necessary, prepends SDK settings,
and preserves existing preferences. Precedence is:

1. SDK defaults, including history/cache enabled.
2. Existing `user.js` preferences.
3. Controller/caller preferences applied through the protocol after startup.

Only the adapter's marked block is replaced on subsequent launches. Files are
replaced atomically with mode 0600. Inherited environment values are passed to
the browser in memory; they are never dumped into a profile or log. Single
`--version`, `-version`, and `-v` requests bypass SDK initialization. Headless
mode follows Playwright's arguments; headed mode requires a working display.

## Writable paths and nono

Grant access to:

- The Nix store (read/execute), including the complete browser and Python closure.
- The selected profile directories (read/write).
- `$XDG_CACHE_HOME/camoufox`, default `~/.cache/camoufox` (read/write): generated
  fontconfig, with absolute references to the store's bundled fonts.
- `~/.camoufox`: create before launch. The SDK probes this even with `-profile`;
  an existing directory can be read-only.
- `$XDG_CACHE_HOME/fontconfig` for font caches.
- Temporary files, shared memory, and playwright-cli's session/socket paths;
  `.playwright-cli` in the working directory for snapshots and screenshots.
- Display sockets/runtime paths and graphics devices for headed rendering as
  required by the existing nono profile.

Do not reuse Chromium's sandbox-helper assumptions for Firefox. Keep namespace
and sandbox failures visible. An outer nono sandbox cannot be relaxed by a
nested nono command; validation inside this agent may inherit additional limits.

## Reproduce validation

```sh
python3 nix/camoufox/test-launch.py
adapter=$(nix build .#camoufox-playwright --no-link --print-out-paths)
python3 nix/camoufox/validate.py "$adapter/bin/camoufox-playwright" \
  /path/to/existing/chromium
nix flake check --no-build
```

`validate.py` uses the installed CLI, an empty temporary HOME/cache, local HTML
pages, and distinct persistent profiles. It exercises snapshots, clicks, filling,
evaluation, history, tabs, screenshots, simultaneous backends, reopening, and
close. Artifacts remain in the printed temporary directory for inspection.

## Repository responsibilities

- **good-vibes-only**: packages, adapter, examples, validation and limitations.
- **nix-config**: consume outputs, install configs, migrate the global executable
  override, and grant required nono access.
- **agent-skills** (`github:huuff/agent-skills`): document Chromium as default,
  explicit Camoufox selection, named sessions, and closing sessions when done.

No consumer or agent-skills files are changed by this implementation.

## Validation report (2026-09-15)

| Check | Result |
| --- | --- |
| Exported browser and adapter Nix builds | Pass |
| All-system flake evaluation | Pass |
| Offline SDK generation under an empty HOME/cache | Pass; network connections forbidden in test |
| Fingerprint keys and bundled font paths | Pass in generated settings; rendered output unverified |
| Profile merge, repeat launch, mode 0600, secret isolation | Pass |
| Browser `--version` | Pass; no runtime files created |
| Exact 1.63 alpha controller connects over Juggler pipe | Pass in nonpersistent launch probe |
| Page creation and persistent CLI launch | Blocked by outer sandbox, not a compatibility pass |
| Navigation, snapshots, clicks, forms, screenshot, history, tabs | Unverified |
| Persistent login reuse and simultaneous Chromium/Camoufox | Unverified |
| Headed mode and `viewport: null` dimensions | Unverified |
| Graceful close after failed page creation | Browser exited 0; temporary profile cleaned |
| Persistent CLI startup timeout cleanup | Pass; no browser process remained |
| General signals/exit-code matrix | Unverified; adapter uses direct exec |
| Outside nono | Not available from this agent environment |

The browser logs `Sandbox: writing /proc/self/uid_map: EACCES`; content processes
crash and `browser.newPage` reports `Target crashed`. Persistent CLI startup
instead waits for a page until timeout. This is an inherited outer sandbox
restriction, so adding permissions to a nested nono process cannot repair it.
Retest in a host shell and then in the consumer's nono profile with Firefox's
required namespace operations available. No Firefox sandbox-disabling
workaround was used. The validation configuration has a 20-second browser
startup timeout so failed launches are cleaned up by Playwright.

The upstream archive also lacks `glxtest`; Firefox logs a failed graphics probe.
The complete archive is retained, including `vulkantest`. Direct ELF dependencies
were patched successfully, and runtime paths include GL, VA-API, PulseAudio,
PipeWire and FFmpeg. Actual accelerated graphics and media playback remain
unverified. Do not infer these capabilities from a successful Nix build.

These pins are candidates with a successful protocol handshake, **not a supported
browser/SDK/controller combination yet**. The remaining checks above must pass
before enabling Camoufox for routine agent use. Chromium's existing package and
sandbox configuration are consumer-owned and unchanged.
