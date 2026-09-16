{
  cargoArtifacts,
  craneLib,
  lib,
  git,
  crate,
  src,
}:

craneLib.buildPackage {
  pname = crate;
  version = "0.1.0";

  inherit cargoArtifacts src;
  strictDeps = true;

  cargoExtraArgs = "-p ${crate}";
  cargoTestExtraArgs = "-p ${crate}";

  # cargo-affected integration tests create and inspect local Git repositories.
  nativeCheckInputs = lib.optionals (crate == "cargo-affected") [ git ];

  meta = {
    description = "${crate} from the good-vibes-only workspace";
    license = lib.licenses.mit;
    mainProgram = crate;
  };
}
