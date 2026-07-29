{
  rustPlatform,
  lib,
}:

rustPlatform.buildRustPackage {
  pname = "good-vibes-only";
  version = "0.1.0";

  src = lib.cleanSource ../.;

  cargoLock.lockFile = ../Cargo.lock;

  meta = {
    description = "A cargo workspace hosting small vibe-coded Rust projects";
    license = lib.licenses.mit;
  };
}
