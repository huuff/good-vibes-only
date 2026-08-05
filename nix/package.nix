{
  rustPlatform,
  lib,
  crate,
}:

rustPlatform.buildRustPackage {
  pname = crate;
  version = "0.1.0";

  # Only the cargo workspace files — commits touching nix/, docs, etc.
  # don't invalidate the build.
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../crates
    ];
  };

  cargoLock.lockFile = ../Cargo.lock;

  cargoBuildFlags = [
    "-p"
    crate
  ];
  cargoTestFlags = [
    "-p"
    crate
  ];

  meta = {
    description = "${crate} from the good-vibes-only workspace";
    license = lib.licenses.mit;
    mainProgram = crate;
  };
}
