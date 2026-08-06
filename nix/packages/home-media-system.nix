{
  lib,
  stdenvNoCC,
  makeWrapper,
  electron,
  nodejs,
  systemd,
}:

stdenvNoCC.mkDerivation {
  pname = "home-media-system";
  version = "0.1.0";

  src = ../../home-media-system;

  nativeBuildInputs = [ makeWrapper ];

  doCheck = true;
  nativeCheckInputs = [ nodejs ];

  checkPhase = ''
    runHook preCheck
    node --test jellyfin-native-login.test.js
    runHook postCheck
  '';

  installPhase = ''
    runHook preInstall

    mkdir -p $out/share/home-media-system $out/bin
    cp -r . $out/share/home-media-system/

    makeWrapper ${lib.getExe electron} $out/bin/home-media-system \
      --prefix PATH : ${lib.makeBinPath [ systemd ]} \
      --add-flags "--ozone-platform-hint=auto" \
      --add-flags "--enable-features=WaylandWindowDecorations" \
      --add-flags $out/share/home-media-system

    runHook postInstall
  '';

  meta = {
    description = "Ten-foot home media launcher";
    license = lib.licenses.mit;
    platforms = lib.platforms.linux;
    mainProgram = "home-media-system";
  };
}
