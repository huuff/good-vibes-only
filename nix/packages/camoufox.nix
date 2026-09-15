{
  lib,
  stdenv,
  fetchurl,
  unzip,
  autoPatchelfHook,
  patchelfUnstable,
  gtk3,
  alsa-lib,
  dbus-glib,
  libxtst,
  curl,
  pciutils,
  libva,
  pipewire,
  libGL,
  ffmpeg,
  libpulseaudio,
}:
stdenv.mkDerivation {
  pname = "camoufox";
  version = "152.0.4-beta.30";
  src = fetchurl {
    url = "https://github.com/daijro/camoufox/releases/download/v152.0.4-beta.30/camoufox-152.0.4-beta.30-lin.x86_64.zip";
    hash = "sha256-VyDUW4lM4XcFQ94CTG8Q1RSzi+Vg+i3DIms9hYbK9nI=";
  };
  nativeBuildInputs = [
    unzip
    autoPatchelfHook
    patchelfUnstable
  ];
  buildInputs = [
    gtk3
    alsa-lib
    dbus-glib
    libxtst
  ];
  runtimeDependencies = [
    curl
    pciutils
    libva.out
    libGL
    libpulseaudio
  ];
  appendRunpaths = [
    "${pipewire}/lib"
    "${ffmpeg}/lib"
  ];
  # Preserve Firefox's fixed-offset relrhack relocations.
  patchelfFlags = [ "--no-clobber-old-sections" ];
  unpackPhase = ''
    mkdir bundle
    cd bundle
    unzip -q "$src"
  '';
  installPhase = ''
    mkdir -p "$out/lib/camoufox" "$out/bin"
    cp -a . "$out/lib/camoufox/"
    # Upstream's SDK normally creates this after downloading the archive.
    cat > "$out/lib/camoufox/version.json" <<'JSON'
    {"version":"152.0.4","build":"beta.30"}
    JSON
    ln -s "$out/lib/camoufox/camoufox" "$out/bin/camoufox"
  '';
  dontStrip = true;
  meta = {
    description = "Camoufox browser bundle for direct Playwright launch";
    homepage = "https://github.com/daijro/camoufox";
    license = lib.licenses.mpl20;
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
    platforms = [ "x86_64-linux" ];
    mainProgram = "camoufox";
  };
}
