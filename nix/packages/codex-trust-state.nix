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
        url = "https://github.com/denoland/rusty_v8/releases/download/v150.4.0/librusty_v8_release_x86_64-unknown-linux-gnu.a.gz";
        hash = "sha256-WGn9twcbHyHyAKl86X0gElh34PMc2ALtmd4sU/SIsGw=";
      }
    else if stdenv.hostPlatform.isAarch64 && stdenv.hostPlatform.isLinux then
      fetchurl {
        url = "https://github.com/denoland/rusty_v8/releases/download/v150.4.0/librusty_v8_release_aarch64-unknown-linux-gnu.a.gz";
        hash = "sha256-txd9Uq0zNycv4NO453gjnIIalcJdWVnexiue/WVPfdM=";
      }
    else
      fetchurl {
        url = "https://github.com/denoland/rusty_v8/releases/download/v150.4.0/librusty_v8_release_aarch64-apple-darwin.a.gz";
        hash = "sha256-zNj4FIW4IsWxiuun+d65KaM4LYasZzu/DzZvBod+axA=";
      };

  rustyV8Binding =
    if stdenv.hostPlatform.isLinux then
      fetchurl {
        url = "https://github.com/denoland/rusty_v8/releases/download/v150.4.0/src_binding_release_${stdenv.hostPlatform.rust.rustcTarget}.rs";
        hash = "sha256-dyeCauR5vbZF6Acjn7EtH44uI956bPFvXuWSaQ0dhQY=";
      }
    else
      fetchurl {
        url = "https://github.com/denoland/rusty_v8/releases/download/v150.4.0/src_binding_release_aarch64-apple-darwin.rs";
        hash = "sha256-ylrfDPicmnCtRgrnNkiy/om3SqETs8t/dXtqArdYOU8=";
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
  RUSTY_V8_SRC_BINDING_PATH = rustyV8Binding;

  # The release flake carries stale hashes for two moving Git repositories.
  cargoDeps = rustPlatform.importCargoLock {
    lockFile = "${old.src}/Cargo.lock";
    outputHashes = {
      "crossterm-0.29.0" = "sha256-ewiWWQPEU1lSUHzmZTiO5yes5luIaQ9TrvCNnTWhxpE=";
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
