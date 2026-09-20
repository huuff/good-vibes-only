{
  pkgs,
  daemonNodejs,
  source,
}:
let
  inherit (pkgs) lib;
  nodejs = pkgs.nodejs_24;
  # Node 24.19.0 regressed cleanup-hook handling used by better-sqlite3's
  # legacy ObjectWrap implementation. The caller supplies 24.18.0 from the
  # dedicated, revision-pinned nixpkgs input so both node-gyp and bin/od use
  # the identical runtime and ABI.
  _daemonNodeVersion = lib.assertMsg (daemonNodejs.version == "24.18.0") ''
    OpenDesign daemon requires Node.js 24.18.0, got ${daemonNodejs.version}
  '';
  inherit (lib.importJSON "${source}/package.json") version;

  filterProjectSource =
    includePaths:
    lib.cleanSourceWith {
      src = source;
      filter =
        path: type:
        let
          root = toString source;
          pathStr = toString path;
          rel = lib.removePrefix (root + "/") pathStr;
          matches =
            includePath:
            rel == includePath
            || lib.hasPrefix (includePath + "/") rel
            || (type == "directory" && lib.hasPrefix (rel + "/") includePath);
        in
        rel == "" || builtins.any matches includePaths;
    };

  workspacePackageManifests =
    workspacePaths: map (workspacePath: "${workspacePath}/package.json") workspacePaths;

  daemonWorkspacePaths = [
    "packages/release"
    "packages/platform"
    "packages/contracts"
    "packages/registry-protocol"
    "packages/agui-adapter"
    "packages/plugin-runtime"
    "packages/sidecar-proto"
    "packages/launcher-proto"
    "packages/sidecar"
    "packages/diagnostics"
    "apps/daemon"
  ];

  webWorkspacePaths = [
    "packages/release"
    "packages/components"
    "packages/contracts"
    "packages/host"
    "packages/platform"
    "packages/sidecar-proto"
    "packages/sidecar"
    "apps/web"
  ];

  commonSourcePaths = [
    "package.json"
    "pnpm-lock.yaml"
    "pnpm-workspace.yaml"
    "tsconfig.json"
  ];

  daemonSrc = filterProjectSource (
    commonSourcePaths
    ++ [
      "assets"
      "plugins"
      "skills"
      "design-systems"
      "design-templates"
      "craft"
      "prompt-templates"
    ]
    ++ daemonWorkspacePaths
  );

  webSrc = filterProjectSource (commonSourcePaths ++ webWorkspacePaths);
  daemonPnpmDepsSrc = filterProjectSource (
    [
      "package.json"
      "pnpm-lock.yaml"
      "pnpm-workspace.yaml"
    ]
    ++ workspacePackageManifests daemonWorkspacePaths
  );
  webPnpmDepsSrc = filterProjectSource (
    [
      "package.json"
      "pnpm-lock.yaml"
      "pnpm-workspace.yaml"
    ]
    ++ workspacePackageManifests webWorkspacePaths
  );

  pnpm_10 = pkgs.pnpm_10.overrideAttrs (_old: rec {
    version = "10.33.2";
    src = pkgs.fetchurl {
      url = "https://registry.npmjs.org/pnpm/-/pnpm-${version}.tgz";
      hash = "sha256-envPE9f2zrOUbAOXg3PZm+n94cr8MAC9/tTE95EWdhA=";
    };
  });

  daemon =
    assert _daemonNodeVersion;
    pkgs.callPackage ./package-daemon.nix {
      inherit pnpm_10 version;
      nodejs = daemonNodejs;
      src = daemonSrc;
      pnpmDepsSrc = daemonPnpmDepsSrc;
      workspacePaths = daemonWorkspacePaths;
    };

  web = pkgs.callPackage ./package-web.nix {
    inherit
      nodejs
      pnpm_10
      version
      ;
    src = webSrc;
    pnpmDepsSrc = webPnpmDepsSrc;
    workspacePaths = webWorkspacePaths;
  };
in
{
  inherit daemon web;
}
