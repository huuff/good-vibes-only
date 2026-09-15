"""Exercise the real packaged SDK with network operations forbidden."""
import json
import os
from pathlib import Path
import runpy
import socket
import sys
import tempfile
from unittest.mock import patch

adapter = sys.argv[1]
root = Path(tempfile.mkdtemp())
os.environ.update(HOME=str(root), XDG_CACHE_HOME=str(root / "cache"), TEST_SECRET="not-for-files")
profile = root / "profile"
profile.mkdir()
module = runpy.run_path(adapter)
with patch.object(sys, "argv", [adapter, "-headless", "-profile", str(profile), "-juggler-pipe"]), \
        patch.object(socket.socket, "connect", side_effect=AssertionError("network forbidden")), \
        patch.object(os, "execve") as execute:
    module["main"]()
    first = (profile / "user.js").read_text()
    module["main"]()
    assert (profile / "user.js").read_text() == first, "generated blocks accumulated"

browser, args, env = execute.call_args.args
assert Path(browser).is_file()
assert json.loads((Path(browser).parent / "version.json").read_text()) == {
    "version": "152.0.4", "build": "beta.30"
}
assert "-juggler-pipe" in args
assert env["TEST_SECRET"] == "not-for-files"
config = json.loads("".join(env[k] for k in sorted(
    (k for k in env if k.startswith("CAMOU_CONFIG_")), key=lambda k: int(k.rsplit("_", 1)[1]))))
assert config["navigator.userAgent"]
assert config["screen.width"] > 0
assert config["screen.height"] > 0
assert config["fonts"]
assert not config.get("addons")
fontconfig = Path(env["FONTCONFIG_FILE"])
assert fontconfig.is_relative_to(root / "cache")
assert str(Path(browser).parent / "fonts") in fontconfig.read_text()
assert (root / ".camoufox").is_dir()
assert (profile / "user.js").stat().st_mode & 0o777 == 0o600
for file in root.rglob("*"):
    if file.is_file():
        assert b"not-for-files" not in file.read_bytes()
print("PASS: offline settings, fingerprints, font paths, preferences, private files, repeat launch")
