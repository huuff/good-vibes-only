"""Unit checks for profile precedence, argv, secret isolation and exec semantics."""
import importlib.util
import os
from pathlib import Path
import sys
sys.dont_write_bytecode = True
import tempfile
import types
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("launch", Path(__file__).with_name("launch.py"))
launch = importlib.util.module_from_spec(spec)
spec.loader.exec_module(launch)


class LaunchTests(unittest.TestCase):
    def test_settings_and_existing_preferences(self):
        with tempfile.TemporaryDirectory() as directory:
            prefs = Path(directory) / "user.js"
            original = 'user_pref("webgl.force-enabled", false);\n'
            prefs.write_text(original)
            def settings(**kwargs):
                self.assertNotIn("TEST_SECRET", kwargs["env"])
                self.assertTrue(kwargs["headless"])
                self.assertFalse(kwargs["geoip"])
                return dict(args=["-extra"], env={"CAMOU_CONFIG_1": "{}"},
                            firefox_user_prefs={"webgl.force-enabled": True})
            modules = {"camoufox": types.ModuleType("camoufox"),
                       "camoufox.addons": types.SimpleNamespace(DefaultAddons=["excluded"]),
                       "camoufox.utils": types.SimpleNamespace(launch_options=settings)}
            argv = ["wrapper", "-headless", "-profile", directory, "-juggler-pipe"]
            with patch.dict(sys.modules, modules), patch.object(sys, "argv", argv), \
                    patch.dict(os.environ, {"TEST_SECRET": "sensitive"}), \
                    patch.object(os, "execve") as execute:
                launch.main()
            executable, args, env = execute.call_args.args
            self.assertEqual(args, [executable, "-extra", *argv[1:]])
            self.assertEqual(env["TEST_SECRET"], "sensitive")
            self.assertNotIn("sensitive", prefs.read_text())
            self.assertTrue(prefs.read_text().endswith(original))

    def test_version_skips_sdk(self):
        with patch.object(sys, "argv", ["wrapper", "--version"]), \
                patch.object(os, "execv", side_effect=SystemExit) as execute:
            with self.assertRaises(SystemExit):
                launch.main()
            execute.assert_called_once_with(launch.BROWSER, [launch.BROWSER, "--version"])

    def test_missing_profile(self):
        with patch.object(sys, "argv", ["wrapper"]):
            with self.assertRaises(ValueError):
                launch.main()


if __name__ == "__main__":
    unittest.main()
