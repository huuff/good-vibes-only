# The new Sentry CLI (github.com/getsentry/cli), NOT nixpkgs' sentry-cli
# (that's the old Rust tool, github.com/getsentry/sentry-cli).
#
# HACK: Bun single-executable binaries embed JS at the end of the ELF.
# dontStrip prevents the nix strip phase from removing the embedded code,
# and we patch with plain patchelf (not autoPatchelfHook, whose extra
# rewrites can shift the embedded code offset). --set-rpath is verified
# to keep the Bun blob intact.
{
  lib,
  stdenv,
  fetchurl,
  patchelf,
  glibc,
}:

stdenv.mkDerivation (finalAttrs: {
  pname = "sentry-cli";
  version = "0.42.2";

  src =
    let
      source =
        {
          x86_64-linux = {
            artifact = "sentry-linux-x64";
            hash = "sha256-rxZ944ZkcAWEt5pa+U1J0zuNnJYG+LbKVO4xsaim5VE=";
          };
          aarch64-linux = {
            artifact = "sentry-linux-arm64";
            hash = "sha256-aX82abJ/lDvPZI+jKAlEqK3HKrl0AJgJp5mHxYtq2qw=";
          };
          aarch64-darwin = {
            artifact = "sentry-darwin-arm64";
            hash = "sha256-7EKQXp/8dNO4JKGvQIMzZ9IPuAmKSnYQfcB+H+mXIj0=";
          };
        }
        .${stdenv.hostPlatform.system};
    in
    fetchurl {
      url = "https://github.com/getsentry/cli/releases/download/${finalAttrs.version}/${source.artifact}";
      inherit (source) hash;
    };

  dontUnpack = true;
  dontStrip = true;

  nativeBuildInputs = lib.optionals stdenv.hostPlatform.isLinux [ patchelf ];

  installPhase = ''
    runHook preInstall
    install -Dm755 $src $out/bin/sentry
    ${lib.optionalString stdenv.hostPlatform.isLinux ''
      patchelf \
        --set-interpreter ${glibc}/lib/ld-linux-*.so.* \
        --set-rpath ${lib.makeLibraryPath [ stdenv.cc.cc.lib ]} \
        $out/bin/sentry
    ''}
    runHook postInstall
  '';

  meta = {
    description = "Sentry command line interface";
    homepage = "https://github.com/getsentry/cli";
    license = lib.licenses.mit;
    mainProgram = "sentry";
    platforms = [
      "x86_64-linux"
      "aarch64-linux"
      "aarch64-darwin"
    ];
  };
})
