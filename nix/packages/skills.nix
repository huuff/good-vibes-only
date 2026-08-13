{
  stdenvNoCC,
  lib,
}:

stdenvNoCC.mkDerivation {
  pname = "skills";
  version = "0.1.0";

  src = lib.cleanSource ../../skills;

  dontConfigure = true;
  dontBuild = true;

  installPhase = ''
    runHook preInstall

    mkdir -p $out/share/skills
    cp -R ./. $out/share/skills/

    runHook postInstall
  '';

  meta = {
    description = "Harness-agnostic skills for AI coding agents";
    platforms = lib.platforms.all;
  };
}
