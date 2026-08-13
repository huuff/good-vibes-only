{
  lib,
  stdenvNoCC,
  fetchurl,
  git,
  makeWrapper,
}:

stdenvNoCC.mkDerivation (finalAttrs: {
  pname = "open-code-review";
  version = "1.9.1";

  src =
    let
      platform = stdenvNoCC.hostPlatform;
      source =
        {
          aarch64-darwin = {
            artifact = "opencodereview-darwin-arm64";
            hash = "sha256-XP/kXvAGuA3L6V5nEYByYYUBCNY5DOcIzawOcssmHR0=";
          };
          aarch64-linux = {
            artifact = "opencodereview-linux-arm64";
            hash = "sha256-rZSfXc/4tmRcXD1H0FtYEpWb0QUFxE/+QcfVWaj+yqM=";
          };
          x86_64-linux = {
            artifact = "opencodereview-linux-amd64";
            hash = "sha256-nLVG5PKTieO312i+zDShjPKqpmNWEEWfplp+oypsi+w=";
          };
        }
        .${platform.system};
    in
    fetchurl {
      url = "https://github.com/alibaba/open-code-review/releases/download/v${finalAttrs.version}/${source.artifact}";
      inherit (source) hash;
    };

  nativeBuildInputs = [ makeWrapper ];

  dontUnpack = true;

  installPhase = ''
    runHook preInstall
    install -Dm755 $src $out/bin/ocr
    wrapProgram $out/bin/ocr --prefix PATH : ${lib.makeBinPath [ git ]}
    runHook postInstall
  '';

  meta = {
    description = "AI-powered code review CLI";
    homepage = "https://github.com/alibaba/open-code-review";
    license = lib.licenses.asl20;
    mainProgram = "ocr";
    platforms = [
      "aarch64-darwin"
      "aarch64-linux"
      "x86_64-linux"
    ];
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
  };
})
