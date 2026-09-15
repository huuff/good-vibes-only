{
  callPackage,
  python3,
  writeTextFile,
}:
let
  browser = callPackage ./camoufox.nix { };
  sdk = callPackage ../camoufox/sdk.nix { };
  python = python3.withPackages (_: [ sdk ]);
in
writeTextFile {
  name = "camoufox-playwright-152.0.4-beta.30";
  destination = "/bin/camoufox-playwright";
  executable = true;
  text =
    "#!${python}/bin/python3\n"
    + builtins.replaceStrings [ "@browser@" ] [ "${browser}/lib/camoufox/camoufox" ] (
      builtins.readFile ../camoufox/launch.py
    );
  passthru = { inherit python sdk; };
  meta = {
    description = "Direct Camoufox launch adapter for Playwright Firefox";
    platforms = [ "x86_64-linux" ];
    mainProgram = "camoufox-playwright";
  };
}
