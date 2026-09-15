"""Exercise installed packages with playwright-cli; no web server is needed.

Usage: python3 validate.py /nix/store/.../bin/camoufox-playwright [chromium-executable]
Creates an isolated HOME beneath a temporary directory; reports its path.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

root = Path(tempfile.mkdtemp(prefix="camoufox-validation-"))
print(f"Artifacts: {root}", flush=True)
env = dict(os.environ, HOME=str(root), XDG_CACHE_HOME=str(root / "cache"))
env.pop("PLAYWRIGHT_MCP_EXECUTABLE_PATH", None)
env["PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD"] = "1"
cli = shutil.which("playwright-cli")
assert cli
(root / "first.html").write_text('''<title>First</title><label>Name<input id="name"></label>
<button onclick="document.querySelector('output').textContent=document.querySelector('input').value">Save</button>
<output></output><a href="second.html">Next</a>''')
(root / "second.html").write_text('<title>Second</title><h1>Second page</h1>')


def command(session, *args):
    proc = subprocess.run([cli, f"-s={session}", *args], env=env, cwd=root,
                          text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=90)
    print(proc.stdout, flush=True)
    if proc.returncode or "### Error" in proc.stdout:
        raise RuntimeError(f"CLI failed: {args[0]}")
    return proc.stdout


sessions = []
try:
    for session, executable, browser in [
        ("camoufox-test", sys.argv[1], "firefox"),
        *([("chromium-test", sys.argv[2], "chromium")] if len(sys.argv) > 2 else []),
    ]:
        config = root / f"{session}.json"
        config.write_text(json.dumps({"browser": {"browserName": browser,
            "launchOptions": {"executablePath": executable, "headless": True, "timeout": 20000},
            "contextOptions": {"viewport": None}}}))
        sessions.append(session)
        command(session, "open", (root / "first.html").as_uri(), f"--config={config}",
                f"--profile={root / session}")
        command(session, "snapshot")
        command(session, "fill", "#name", "Camoufox works")
        command(session, "click", "button")
        assert "Camoufox works" in command(session, "eval", "document.querySelector('output').textContent")
        command(session, "click", "a")
        assert "Second" in command(session, "eval", "document.title")
        command(session, "go-back")
        assert "First" in command(session, "eval", "document.title")
        command(session, "go-forward")
        assert "Second" in command(session, "eval", "document.title")
        command(session, "tab-new", (root / "first.html").as_uri())
        command(session, "tab-list")
        command(session, "eval", "localStorage.setItem('persistent', 'yes')")
        command(session, "screenshot", f"--filename={root / (session + '.png')}")
        command(session, "eval", "JSON.stringify({ua:navigator.userAgent,screen:[screen.width,screen.height],window:[innerWidth,innerHeight]})")
    for session in sessions:
        command(session, "close")
        command(session, "open", (root / "first.html").as_uri(),
                f"--config={root / (session + '.json')}", f"--profile={root / session}")
        assert 'yes' in command(session, "eval", "localStorage.getItem('persistent')")
finally:
    for session in sessions:
        command(session, "close")
print("PASS", flush=True)
