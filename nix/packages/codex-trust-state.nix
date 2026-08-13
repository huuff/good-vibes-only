{ codexUpstream, rustPlatform }:

codexUpstream.overrideAttrs (old: {
  pname = "codex-trust-state";

  patches = (old.patches or [ ]) ++ [ ../patches/codex-project-trust-file.patch ];
  patchFlags = (old.patchFlags or [ ]) ++ [ "-p2" ];
  cargoBuildFlags = [
    "-p"
    "codex-cli"
  ];

  # Upstream's flake hash lags the git dependency revision in Cargo.lock at
  # the pinned commit.
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
