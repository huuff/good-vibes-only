# Import from the consumer with its existing Chromium package and sandbox value.
{
  pkgs,
  camoufox-playwright,
  chromiumExecutable,
  chromiumSandbox,
}:
{
  chromium = pkgs.writeText "playwright-chromium.json" (
    builtins.toJSON {
      browser = {
        browserName = "chromium";
        launchOptions = {
          executablePath = chromiumExecutable;
          inherit chromiumSandbox;
        };
      };
    }
  );
  camoufox = pkgs.writeText "playwright-camoufox.json" (
    builtins.toJSON {
      browser = {
        browserName = "firefox";
        launchOptions.executablePath = "${camoufox-playwright}/bin/camoufox-playwright";
        contextOptions.viewport = null;
      };
    }
  );
}
