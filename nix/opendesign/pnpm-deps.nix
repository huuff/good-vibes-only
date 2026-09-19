{
  # Vendored pnpm store hashes for the workspace packages built by the flake.
  # Generated lock artifact; do not hand-edit outside intentional Nix maintenance.
  #
  # The daemon and web derivations now build from different filtered source
  # trees, so each fetchPnpmDeps invocation needs its own fixed-output hash.
  # Refresh a hash whenever pnpm-lock.yaml or that derivation's source filter
  # changes:
  # 1. Temporarily set the consuming `hash = lib.fakeHash;`
  # 2. Run the relevant nix build/flake check
  # 3. Copy the expected hash printed by Nix into the matching field below
  daemonHash = "sha256-uV5CY0TP2n8pBVjxkanA29h6UEfr70a3nJUJ6TJkpbo=";
  webHash = "sha256-SIs+q91GnNvX8P4Vh/515YAXqWlvFV05HLZD1I9MDGw=";
}
