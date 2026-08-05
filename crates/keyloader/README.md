# keyloader

Discover GPG and SSH keys stored in 1Password and load them into
`gpg-agent` and `ssh-agent` — idempotently, without ever writing key
material to disk.

```console
$ keyloader discover   # fetch the key list from 1Password, cache it locally
$ keyloader status     # which of those keys are already usable locally
$ keyloader load       # import/add whatever is missing (safe to re-run)
$ keyloader load --dry-run
```

Secrets only ever move through pipes between `op`, `gpg`, `ssh-add` and
`gpg-preset-passphrase`: never through argv, never through files.

## Local cache

`discover` mirrors what it finds into
`$XDG_CACHE_HOME/keyloader/items.json` (default
`~/.cache/keyloader/items.json`): item ids, titles, vault names and
public fingerprints, plus timestamps for keys loaded by keyloader —
metadata only, no key material. `status` and `load` read that cache
instead of listing items through `op`, so:

- `status` never touches 1Password — it only asks ssh-agent and
  gpg-agent, and works signed out, locked, or offline. For currently
  loaded entries, it reports the estimated remaining time when
  keyloader loaded the entry itself. Agent protocols do not expose
  per-key expiry times, so entries loaded before tracking or refreshed
  by another program have an unknown countdown.
- `load` contacts 1Password only to fetch the secrets of keys that are
  actually missing; when everything is already loaded it makes no `op`
  calls at all.

The cache never refreshes implicitly: re-run `keyloader discover` after
adding, removing or rotating keys in 1Password. Both commands tell you
how stale the cache is (`discovered 2h ago`) and refuse to run before
the first `discover`.

## Signing in

No need to `eval $(op signin)` first: if 1Password reports you're not
signed in and keyloader is running on a terminal, it runs `op signin`
for you (the password prompt is `op`'s own, straight from the tty).

The session token is cached in the kernel **session keyring** — kernel
memory only, never the filesystem — with the same 30-minute idle expiry
`op` applies server-side, so consecutive keyloader runs in the same
login session reuse it instead of prompting again. Processes outside
the session (other logins, SSH sessions, cron) cannot read it, even
under your own uid. Inspect or drop it with `keyctl`:

```console
$ keyctl search @s user keyloader:op-session   # is one cached?
$ keyctl unlink $(keyctl search @s user keyloader:op-session) @s
```

Without a terminal and with no cached session, keyloader keeps its
quiet fail-soft behavior: exit code 2, no prompts. If you use the
1Password desktop app with CLI integration, none of this applies —
authorization goes through the app instead.

## 1Password conventions

keyloader is convention-based. It looks for:

### SSH keys — the native "SSH Key" category

Every item in the **SSH Key** category is picked up automatically; the
`fingerprint` and `public key` fields 1Password maintains are used to
check whether the key is already in the agent, and the private key is
fetched (in OpenSSH format) only when it needs to be added.

Keys are expected to be stored **without a passphrase** — 1Password is
the encryption at rest. Passphrase-protected keys fail with a clear
error instead of hanging on a prompt.

> Alternatively, consider 1Password's built-in SSH agent, which serves
> these keys without them ever entering `ssh-agent`. keyloader exists
> for setups that want the stock OpenSSH agent (and for GPG, which
> 1Password has no agent for).

### GPG keys — items tagged `keyloader/gpg`

Tag any item (Secure Note works well) with `keyloader/gpg` and give it
these fields:

| Field label   | Type      | Required | Content                                    |
|---------------|-----------|----------|--------------------------------------------|
| `secret key`  | concealed | yes      | ASCII-armored secret key (`gpg --export-secret-keys --armor`) |
| `fingerprint` | text      | no       | 40-hex-char key fingerprint, no spaces     |
| `passphrase`  | concealed | no       | the key's passphrase, for gpg-agent preset |

`load` imports the key into the keyring if the fingerprint is missing,
then — if a `passphrase` field exists — presets the passphrase into
gpg-agent for every keygrip (subkey) that isn't already cached.

Field labels are matched case-insensitively. Any category works —
discovery is by tag alone — but the key must live in the `secret key`
*field*: file attachments are never read, so attaching an exported
`.asc` to a Document item does nothing.

To add a key, export it:

```console
$ gpg --export-secret-keys --armor <KEY-ID>
```

then create the item in the 1Password app: tag it `keyloader/gpg` and
paste the whole armored block (`BEGIN` through `END` lines) into a
concealed `secret key` field. Paste into the app rather than passing
the key to `op item create` on the command line — argv is visible to
every process on the machine. The export includes all subkeys, which
is what `load` expects. Finish with `keyloader discover` to pick up
the new item.

The fingerprint is what lets `status` and `load` check the keyring
without fetching secret material from 1Password, but you don't have to
provide it: the first `load` learns it from gpg when it imports the key
and remembers it in the local cache. `discover` carries learned
fingerprints over — unless the item was edited in 1Password since
(rotated key, say), in which case the next `load` re-imports and
re-learns. Declare a `fingerprint` field explicitly only if you want
`status` to be definitive before the first load; keyloader then also
verifies that the imported key actually matches it.

## gpg-agent configuration

Presetting passphrases requires this in `~/.gnupg/gpg-agent.conf`
(then `gpgconf --kill gpg-agent`):

```
allow-preset-passphrase
```

With home-manager:

```nix
services.gpg-agent = {
  enable = true;
  extraConfig = "allow-preset-passphrase";
  maxCacheTtl = 86400; # preset entries expire with this TTL
};
```

Note that preset passphrases live until `max-cache-ttl` expires or the
agent restarts — pick a TTL you're comfortable with.

## Exit codes

| Code | Meaning                                                        |
|------|----------------------------------------------------------------|
| 0    | success                                                        |
| 1    | something actually failed                                      |
| 2    | expected, safe to ignore in scripts: 1Password locked/signed out, or no cache yet (`discover` never ran) |

## Installing

Build the package from the workspace root:

```console
$ cargo install --path crates/keyloader
```

## Development

```console
$ cargo test -p keyloader
$ cargo run -p keyloader -- status
```

Runtime dependencies looked up on PATH: `op`, `gpg`, `gpgconf`,
`gpg-connect-agent`, `ssh-add`. `systemctl` is used as an optional
fallback for inspecting a systemd-managed ssh-agent's retention policy
when the agent process is hidden by a sandboxed `/proc`.
