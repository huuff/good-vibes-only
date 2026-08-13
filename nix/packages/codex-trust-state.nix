{
  codexUpstream,
  fetchurl,
  rustPlatform,
  stdenv,
}:

let
  rustyV8 =
    if stdenv.hostPlatform.isx86_64 && stdenv.hostPlatform.isLinux then
      fetchurl {
        url = "https://github.com/denoland/rusty_v8/releases/download/v149.2.0/librusty_v8_release_x86_64-unknown-linux-gnu.a.gz";
        hash = "sha256-iu2YY323533Iv7i7R1nsW95HLQv3lD9Y4OYqNQlFxVk=";
      }
    else if stdenv.hostPlatform.isAarch64 && stdenv.hostPlatform.isLinux then
      fetchurl {
        url = "https://github.com/denoland/rusty_v8/releases/download/v149.2.0/librusty_v8_release_aarch64-unknown-linux-gnu.a.gz";
        hash = "sha256-+XdRJ8pk3MSjZi0BpSGizvuluY+DOUOog9hHc7Kv88U=";
      }
    else
      fetchurl {
        url = "https://github.com/denoland/rusty_v8/releases/download/v149.2.0/librusty_v8_release_aarch64-apple-darwin.a.gz";
        hash = "sha256-+rsuyNO6Wm3qY9uaNalg3FypheujLzQrm6Sqocc0sv4=";
      };
in

codexUpstream.overrideAttrs (old: {
  pname = "codex-trust-state";

  patches = (old.patches or [ ]) ++ [
    ../patches/codex-project-trust-file.patch
  ];
  patchFlags = (old.patchFlags or [ ]) ++ [ "-p2" ];
  cargoBuildFlags = [
    "-p"
    "codex-cli"
    "-p"
    "codex-code-mode-host"
  ];
  RUSTY_V8_ARCHIVE = rustyV8;

  # The release flake carries stale hashes for two moving Git repositories.
  cargoDeps = rustPlatform.importCargoLock {
    lockFile = "${old.src}/Cargo.lock";
    outputHashes = {
      "ratatui-0.29.0" = "sha256-HBvT5c8GsiCxMffNjJGLmHnvG77A6cqEL+1ARurBXho=";
      "crossterm-0.28.1" = "sha256-6qCtfSMuXACKFb9ATID39XyFDIEMFDmbx6SSmNe+728=";
      "nucleo-0.5.0" = "sha256-Hm4SxtTSBrcWpXrtSqeO0TACbUxq3gizg1zD/6Yw/sI=";
      "nucleo-matcher-0.3.1" = "sha256-Hm4SxtTSBrcWpXrtSqeO0TACbUxq3gizg1zD/6Yw/sI=";
      "runfiles-0.1.0" = "sha256-uJpVLcQh8wWZA3GPv9D8Nt43EOirajfDJ7eq/FB+tek=";
      "tokio-tungstenite-0.28.0" = "sha256-V1xmnrfRWOcZZogelZEA4vvyMj2awCfHVA5/glQ6KAI=";
      "tungstenite-0.27.0" = "sha256-VVHhk7l9J/sEmG3q/UuV/sQ3f+fGsmq5vumSy8vbMvw=";
    };
  };

  meta = old.meta // {
    description = "Codex CLI with project trust stored outside config.toml";
  };
})
