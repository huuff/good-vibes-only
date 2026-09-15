"""Translate SDK settings into a Playwright-prepared Firefox profile, then exec."""
import json
import os
from pathlib import Path
import sys
import tempfile

BROWSER = "@browser@"
BEGIN = "// BEGIN camoufox-playwright generated preferences\n"
END = "// END camoufox-playwright generated preferences\n"


def main():
    args = sys.argv[1:]
    if args in (["--version"], ["-version"], ["-v"]):
        os.execv(BROWSER, [BROWSER, *args])
    profiles = [i for i, arg in enumerate(args) if arg in ("-profile", "--profile")]
    if len(profiles) != 1 or profiles[0] + 1 >= len(args):
        raise ValueError("expected one Playwright -profile <directory>")
    prefs = Path(args[profiles[0] + 1]) / "user.js"
    original = prefs.read_text() if prefs.exists() else ""
    if not prefs.parent.is_dir():
        raise ValueError("Playwright profile directory does not exist")

    if original.startswith(BEGIN) and END in original:
        original = original.split(END, 1)[1]

    from camoufox.addons import DefaultAddons
    from camoufox.utils import launch_options

    options = launch_options(
        executable_path=BROWSER,
        ff_version=152,
        headless=any(arg in ("-headless", "--headless") for arg in args),
        exclude_addons=list(DefaultAddons),
        geoip=False,
        enable_cache=True,
        i_know_what_im_doing=True,
        env={key: os.environ[key] for key in ("HOME", "DISPLAY", "WAYLAND_DISPLAY")
             if key in os.environ},
    )
    generated = "".join(
        f"user_pref({json.dumps(key)}, {json.dumps(value)});\n"
        for key, value in sorted(options["firefox_user_prefs"].items())
    )
    # Firefox uses the last value: Playwright and caller preferences win.
    with tempfile.NamedTemporaryFile(mode="w", dir=prefs.parent, delete=False) as staged:
        staged.write(BEGIN + generated + END + original)
    try:
        os.replace(staged.name, prefs)
    finally:
        Path(staged.name).unlink(missing_ok=True)
    environment = dict(os.environ)
    environment.update(options["env"])
    os.execve(BROWSER, [BROWSER, *options["args"], *args], environment)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError) as error:
        print(f"camoufox-playwright: launch preparation failed ({type(error).__name__})",
              file=sys.stderr)
        sys.exit(1)
