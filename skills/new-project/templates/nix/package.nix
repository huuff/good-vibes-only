# CHANGEME: replace this stub with the right builder for the project's
# language (buildRustPackage, buildGoModule, buildNpmPackage, ...).
# The stub is buildable so `nix flake check` passes from day one.
{
  stdenvNoCC,
  lib,
}:

stdenvNoCC.mkDerivation {
  pname = "CHANGEME_PNAME";
  version = "0.1.0";

  src = lib.cleanSource ../.;

  installPhase = ''
    mkdir -p $out/share/CHANGEME_PNAME
    cp -r . $out/share/CHANGEME_PNAME
  '';

  meta = {
    description = "CHANGEME_DESCRIPTION";
    license = lib.licenses.mit; # CHANGEME
  };
}
