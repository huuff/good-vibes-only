{
  pkgs,
  home-manager,
  module,
}:
let
  inherit (pkgs) lib;
  cli = pkgs.writeShellScriptBin "playwright-cli" "exit 0";
  chromium = pkgs.writeShellScriptBin "chromium" "exit 0";
  nativeCamoufox = pkgs.stdenv.hostPlatform.system == "x86_64-linux";
  adapter =
    if nativeCamoufox then
      pkgs.callPackage ../packages/camoufox-playwright.nix { }
    else
      pkgs.writeShellScriptBin "camoufox-playwright" "exit 0";
  mkHome =
    chrome: camou:
    home-manager.lib.homeManagerConfiguration {
      inherit pkgs;
      modules = [
        module
        {
          home = {
            username = "playwright-test";
            homeDirectory = "/build/playwright-${toString chrome}-${toString camou}/home user's";
            stateVersion = "25.11";
          };
          xdg = {
            cacheHome = "/build/playwright-${toString chrome}-${toString camou}/custom cache";
            configHome = "/build/playwright-${toString chrome}-${toString camou}/custom config";
          };
          programs.playwright-cli = {
            enable = true;
            package = cli;
            chromium = {
              enable = chrome;
              package = chromium;
              sandbox = true;
            };
            camoufox = {
              enable = camou;
            }
            // lib.optionalAttrs (!nativeCamoufox) { package = adapter; };
          };
        }
      ];
    };
  check =
    name: chrome: camou:
    let
      hm = mkHome chrome camou;
      c = hm.config;
      cfg = c.programs.playwright-cli;
      home = c.home.homeDirectory;
      cache = c.xdg.cacheHome;
      configDir = "${c.xdg.configHome}/playwright";
      writes = [
        "${cache}/ms-playwright"
        "${cache}/fontconfig"
      ]
      ++ lib.optionals camou [
        "${cache}/camoufox"
        "${home}/.camoufox"
      ];
      activation = pkgs.writeText "playwright-activation" c.home.activation.playwrightCliDirectories.data;
      configs = pkgs.linkFarm "playwright-test-configs-${name}" (
        [
          {
            name = "default.json";
            path = c.home.file.".playwright/cli.config.json".source;
          }
        ]
        ++ lib.optional chrome {
          name = "chromium.json";
          path = c.xdg.configFile."playwright/chromium.json".source;
        }
        ++ lib.optional camou {
          name = "camoufox.json";
          path = c.xdg.configFile."playwright/camoufox.json".source;
        }
      );
    in
    assert lib.all (a: a.assertion) c.assertions;
    assert builtins.isString hm.activationPackage.drvPath;
    assert lib.elem cli c.home.packages;
    assert (lib.elem chromium c.home.packages) == chrome;
    assert (lib.elem adapter c.home.packages) == camou;
    assert
      !nativeCamoufox || !(lib.elem (pkgs.callPackage ../packages/camoufox.nix { }) c.home.packages);
    assert (builtins.hasAttr "playwright/chromium.json" c.xdg.configFile) == chrome;
    assert (builtins.hasAttr "playwright/camoufox.json" c.xdg.configFile) == camou;
    assert !(c.home.sessionVariables ? PLAYWRIGHT_MCP_EXECUTABLE_PATH);
    assert c.home.sessionVariables.XDG_CACHE_HOME == cache;
    assert
      cfg.filesystem.read == [
        builtins.storeDir
        "${home}/.playwright"
        configDir
      ];
    assert
      cfg.filesystem.write
      == writes ++ [ "/tmp" ] ++ lib.optional pkgs.stdenv.hostPlatform.isLinux "/dev/shm";
    pkgs.runCommand "hm-playwright-${name}" { nativeBuildInputs = [ pkgs.python3 ]; } ''
      # Exercise the activation entry, including its use of HM's dry-run helper.
      run() { if [[ "$dry" == false ]]; then "$@"; fi; }
      dry=true
      source ${activation}
      for directory in ${
        lib.escapeShellArgs (
          [
            "${home}/.playwright"
            configDir
          ]
          ++ writes
        )
      }; do
        test ! -e "$directory"
      done
      dry=false
      source ${activation}
      source ${activation}
      for directory in ${
        lib.escapeShellArgs (
          [
            "${home}/.playwright"
            configDir
          ]
          ++ writes
        )
      }; do
        test -d "$directory"
        test "$(stat -c %a "$directory")" = 700
      done
      chmod 750 ${lib.escapeShellArg "${cache}/ms-playwright"}
      source ${activation}
      test "$(stat -c %a ${lib.escapeShellArg "${cache}/ms-playwright"})" = 750
      ${lib.optionalString (!camou) "test ! -e ${lib.escapeShellArg "${home}/.camoufox"}"}
      python - ${configs} <<'PY'
      import json
      import pathlib
      import sys
      configs = pathlib.Path(sys.argv[1])
      default = json.loads((configs / 'default.json').read_text())
      assert default['browser']['browserName'] == '${if chrome then "chromium" else "firefox"}'
      ${lib.optionalString chrome ''
        chromium = json.loads((configs / 'chromium.json').read_text())
        assert chromium['browser']['launchOptions'] == {
            'executablePath': '${chromium}/bin/chromium', 'chromiumSandbox': True
        }
        assert default == chromium
      ''}
      ${lib.optionalString camou ''
        camoufox = json.loads((configs / 'camoufox.json').read_text())
        assert camoufox['browser']['browserName'] == 'firefox'
        assert camoufox['browser']['launchOptions'] == {
            'executablePath': '${adapter}/bin/camoufox-playwright'
        }
        assert camoufox['browser']['contextOptions']['viewport'] is None
        ${lib.optionalString (!chrome) "assert default == camoufox"}
      ''}
      PY
      cp -r ${configs} "$out"
    '';
  neither = mkHome false false;
  disabled = home-manager.lib.homeManagerConfiguration {
    inherit pkgs;
    modules = [
      module
      {
        home = {
          username = "disabled";
          homeDirectory = "/build/disabled";
          stateVersion = "25.11";
        };
      }
    ];
  };
in
{
  chromium = check "chromium" true false;
  camoufox = check "camoufox" false true;
  both = check "both" true true;
  neither =
    assert disabled.config.programs.playwright-cli.filesystem.read == [ ];
    assert disabled.config.programs.playwright-cli.filesystem.write == [ ];
    assert !(disabled.config.home.activation ? playwrightCliDirectories);
    assert !(builtins.tryEval neither.activationPackage.drvPath).success;
    pkgs.runCommand "hm-playwright-neither-rejected" { } ''touch "$out"'';
}
