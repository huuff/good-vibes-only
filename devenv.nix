# Add `lib` etc. to the lambda args when first needed (deadnix rejects
# unused args, statix rejects empty `{ ... }` patterns — use `_:` if no
# args remain).
{ pkgs, config, ... }:

{
  # rust-overlay channel instead of nixpkgs' rustc: extra targets (the
  # nixpkgs toolchain only ships wasm32 std, and Android needs its own).
  languages.rust = {
    enable = true;
    channel = "stable";
    targets = [
      "wasm32-unknown-unknown"
      "aarch64-linux-android"
    ];
  };
  languages.nix.enable = true;

  # Android SDK/NDK for `dx build --platform android` (crates/tally).
  # Emulator off: the target is a physical phone.
  android = {
    enable = true;
    ndk.enable = true;
    emulator.enable = false;
    platforms.version = [ "34" ];
    buildTools.version = [ "34.0.0" ];
  };

  # dx/Gradle look for the NDK under these names; devenv only exports
  # ANDROID_NDK_ROOT.
  env.ANDROID_NDK_HOME = config.env.ANDROID_NDK_ROOT;
  env.NDK_HOME = config.env.ANDROID_NDK_ROOT;

  # dx builds/serves the Dioxus web crates (crates/tally); linking wasm
  # needs lld.
  packages = [
    pkgs.dioxus-cli
    pkgs.lld
    pkgs.nodejs
  ];

  # Build the tally APK and sideload it onto a USB-connected phone
  # (USB debugging on). The explicit --target matters: without it dx
  # assumes an emulator (x86_64) and tries to rustup-install that
  # target, which the nix toolchain can't do.
  tasks."tally:android:install" = {
    description = "Build the tally APK and install it on a USB-connected phone";
    exec = ''
      cd "$DEVENV_ROOT/crates/tally"
      dx build --platform android --release --target aarch64-linux-android
      android_app="$DEVENV_ROOT/target/dx/tally/release/android/app"
      cp -R android/res/. "$android_app/app/src/main/res/"
      "$android_app/gradlew" -p "$android_app" assembleDebug
      apk="$android_app/app/build/outputs/apk/debug/app-debug.apk"
      adb install -r "$apk"
    '';
  };

  git-hooks.hooks = {
    # --- secrets: never commit credentials ---
    ripsecrets.enable = true; # scans staged changes for API keys/tokens
    detect-private-keys.enable = true;

    # --- hygiene ---
    check-added-large-files.enable = true;
    check-merge-conflicts.enable = true;
    end-of-file-fixer.enable = true;
    trim-trailing-whitespace = {
      enable = true;
      # Unified diffs use a single space to represent blank context lines.
      excludes = [ "\\.patch$" ];
    };

    # --- static analysis: nix ---
    nixfmt.enable = true; # RFC 166 style; nixfmt >= 1.0 (nixfmt-rfc-style is the deprecated alias)
    statix.enable = true;
    deadnix.enable = true;

    # --- static analysis: rust ---
    clippy.enable = true;
    rustfmt.enable = true;

    # --- static analysis: shell ---
    shellcheck.enable = true;
    shfmt.enable = true;

    # --- tests: native Jellyfin credential injection ---
    jellyfin-native-login = {
      enable = true;
      name = "native Jellyfin login test";
      entry = "${pkgs.nodejs}/bin/node --test home-media-system/jellyfin-native-login.test.js";
      always_run = true;
      pass_filenames = false;
    };

    # --- commit messages: Conventional Commits (feat:, fix:, chore:, ...) ---
    commitizen.enable = true;
  };

  enterTest = ''
    nix flake check --no-build
  '';
}
