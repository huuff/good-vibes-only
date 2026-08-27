{
  lib,
  stdenvNoCC,
  fetchurl,
  appimageTools,
  undmg,
}:

let
  pname = "orca";
  version = "1.4.190";

  sources = {
    x86_64-linux = {
      url = "https://github.com/stablyai/orca/releases/download/v${version}/orca-linux.AppImage";
      hash = "sha256-9bMhV22ckJ+eaYeqO9IOj/nyFNiBtDxxCSgcvIeHjN4=";
    };
    aarch64-linux = {
      url = "https://github.com/stablyai/orca/releases/download/v${version}/orca-linux-arm64.AppImage";
      hash = "sha256-ikOPMxiJvE+8WgejmmWyfcEkI/MQwtDROIZJ3JOjKMg=";
    };
    aarch64-darwin = {
      url = "https://github.com/stablyai/orca/releases/download/v${version}/orca-macos-arm64.dmg";
      hash = "sha256-qIp0NjhI06Hcthm4LhbOSOpWV3f3ToyRfw0tUHEcNz4=";
    };
  };

  src = fetchurl sources.${stdenvNoCC.hostPlatform.system};

  meta = {
    description = "Agent development environment for running coding agents";
    homepage = "https://www.onorca.dev";
    license = lib.licenses.asl20;
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
    platforms = lib.attrNames sources;
    mainProgram = "orca";
  };
in
if stdenvNoCC.hostPlatform.isDarwin then
  stdenvNoCC.mkDerivation {
    inherit
      pname
      version
      src
      meta
      ;
    nativeBuildInputs = [ undmg ];
    sourceRoot = ".";
    installPhase = ''
      runHook preInstall
      mkdir -p "$out/Applications"
      cp -R Orca.app "$out/Applications/"
      runHook postInstall
    '';
  }
else
  appimageTools.wrapType2 {
    inherit
      pname
      version
      src
      meta
      ;
  }
