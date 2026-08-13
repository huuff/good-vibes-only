{
  cargoArtifacts,
  craneLib,
  lib,
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

  meta = {
    description = "${crate} from the good-vibes-only workspace";
    license = lib.licenses.mit;
    mainProgram = crate;
  };
}
