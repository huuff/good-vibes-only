const { app, BrowserWindow, ipcMain } = require("electron");
const fs = require("fs");
const path = require("path");
const { spawn } = require("child_process");

let window;
let settings;
let onHome = true;
let activeApplication;
let activeOrigin;
let nativeChild;
const pidFile = process.env.XDG_STATE_HOME
  ? path.join(process.env.XDG_STATE_HOME, "home-media-system", "launcher.pid")
  : undefined;

if (process.env.XDG_STATE_HOME) {
  app.setPath("userData", path.join(process.env.XDG_STATE_HOME, "home-media-system"));
}

function argument(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

function loadSettings() {
  const configPath = argument("--config");
  if (!configPath) throw new Error("home-media-system requires --config <path>");
  return JSON.parse(fs.readFileSync(configPath, "utf8"));
}

function publicSettings() {
  return {
    title: settings.title,
    locale: settings.locale,
    clock: settings.clock,
    applications: settings.applications.map(({ autoLogin, ...application }) => application),
    power: settings.power,
  };
}

function showHome() {
  onHome = true;
  activeApplication = undefined;
  activeOrigin = undefined;
  window.loadFile(path.join(__dirname, "index.html"));
}

function sentFromHome(event) {
  return event.senderFrame.url === `file://${path.join(__dirname, "index.html")}`;
}

function applicationFor(url) {
  return settings.applications.find((candidate) => {
    try {
      return new URL(candidate.url).origin === new URL(url).origin;
    } catch (_) {
      return false;
    }
  });
}

function isWebUrl(url) {
  try {
    return ["http:", "https:"].includes(new URL(url).protocol);
  } catch (_) {
    return false;
  }
}

function readSecret(file) {
  return fs.readFileSync(file, "utf8").trimEnd();
}

async function attemptAutoLogin(application) {
  const login = application.autoLogin;
  if (!login || !login.enable) return;

  let username;
  let password;
  try {
    username = readSecret(login.usernameFile);
    password = readSecret(login.passwordFile);
  } catch (error) {
    console.error(`Could not read auto-login credentials for ${application.name}:`, error.message);
    return;
  }

  const payload = JSON.stringify({
    type: application.type,
    username,
    password,
    usernameSelector: login.usernameSelector,
    passwordSelector: login.passwordSelector,
    submitSelector: login.submitSelector,
  });

  // Credentials are read at runtime and only injected into the selected login
  // page. They are never part of the generated Nix store configuration.
  await window.webContents.executeJavaScript(`
    (() => {
      const options = ${payload};
      let attempts = 0;
      const setValue = (element, value) => {
        const setter = Object.getOwnPropertyDescriptor(
          Object.getPrototypeOf(element), "value"
        )?.set;
        if (setter) setter.call(element, value); else element.value = value;
        element.dispatchEvent(new Event("input", { bubbles: true }));
        element.dispatchEvent(new Event("change", { bubbles: true }));
      };
      const fill = () => {
        attempts += 1;
        const user = document.querySelector(options.usernameSelector);
        const pass = document.querySelector(options.passwordSelector);
        if (pass && (user || options.type === "jellyfin")) {
          if (user) setValue(user, options.username);
          setValue(pass, options.password);
          window.setTimeout(() => {
            const submit = document.querySelector(options.submitSelector);
            if (submit && !submit.disabled) submit.click();
          }, 250);
          return;
        }
        if (options.type === "jellyfin") {
          const controls = [...document.querySelectorAll("button, .card")];
          const account = controls.find((element) =>
            element.textContent.trim().toLocaleLowerCase() === options.username.toLocaleLowerCase()
          );
          const manual = document.querySelector("#btnManualLogin, .btnManualLogin") || controls.find((element) => {
            const text = element.textContent.toLocaleLowerCase();
            return text.includes("manual") && (text.includes("login") || text.includes("sign in"));
          });
          if (account) account.click(); else if (manual) manual.click();
        }
        if (attempts < 40) window.setTimeout(fill, 250);
      };
      fill();
    })()
  `).catch((error) => console.error("Auto-login injection failed:", error.message));
}

function runPowerAction(action) {
  const allowed = {
    sleep: settings.power.sleep,
    restart: settings.power.restart,
    poweroff: settings.power.poweroff,
  };
  if (!allowed[action]) return;

  const verb = action === "sleep" ? "suspend" : action === "restart" ? "reboot" : "poweroff";
  const child = spawn("systemctl", [verb], { detached: true, stdio: "ignore" });
  child.on("error", (error) => console.error(`systemctl ${verb} failed:`, error.message));
  child.unref();
}

function openNativeApplication(application) {
  if (!application.nativeCommand || nativeChild) return;

  nativeChild = spawn(application.nativeCommand, application.nativeArgs || [], {
    env: process.env,
    stdio: "inherit",
  });
  nativeChild.once("spawn", () => {
    onHome = false;
    window.hide();
  });
  nativeChild.once("error", (error) => {
    console.error(`Could not launch ${application.name}:`, error.message);
    nativeChild = undefined;
    window.show();
  });
  nativeChild.once("exit", () => {
    nativeChild = undefined;
    showHome();
    window.show();
    window.focus();
  });
}

function returnHome() {
  if (nativeChild) {
    nativeChild.kill("SIGTERM");
    return;
  }
  if (!onHome) showHome();
  window.show();
  window.focus();
}

process.on("SIGUSR1", returnHome);

app.whenReady().then(() => {
  settings = loadSettings();
  window = new BrowserWindow({
    width: 1280,
    height: 720,
    fullscreen: settings.fullscreen,
    kiosk: settings.kiosk,
    autoHideMenuBar: true,
    backgroundColor: "#090b14",
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  window.webContents.setWindowOpenHandler(({ url }) => {
    if (!isWebUrl(url)) return { action: "deny" };
    onHome = false;
    window.loadURL(url);
    return { action: "deny" };
  });

  window.webContents.on("before-input-event", (event, input) => {
    const back = input.type === "keyDown" &&
      (input.key === "Escape" || input.key === "BrowserBack" || input.code === "GoBack");
    if (back && !onHome) {
      event.preventDefault();
      showHome();
    }
  });

  window.webContents.on("did-finish-load", () => {
    if (onHome) return;
    const currentUrl = window.webContents.getURL();
    let currentOrigin;
    try {
      currentOrigin = new URL(currentUrl).origin;
    } catch (_) {
      return;
    }
    const selected = activeApplication && currentOrigin === activeOrigin
      ? activeApplication
      : applicationFor(currentUrl);
    if (selected) attemptAutoLogin(selected);
  });

  ipcMain.handle("settings", (event) => sentFromHome(event) ? publicSettings() : null);
  ipcMain.on("open", (event, id, runtimeUrl) => {
    if (!sentFromHome(event)) return;
    const selected = settings.applications.find((candidate) => candidate.id === id);
    if (!selected) return;
    if (selected.nativeCommand) {
      openNativeApplication(selected);
      return;
    }
    const target = selected.url || runtimeUrl;
    let parsed;
    try {
      parsed = new URL(target);
      if (parsed.protocol !== "http:" && parsed.protocol !== "https:") return;
    } catch (_) {
      return;
    }
    activeApplication = selected;
    activeOrigin = parsed.origin;
    onHome = false;
    window.loadURL(target);
  });
  ipcMain.on("power", (event, action) => {
    if (sentFromHome(event)) runPowerAction(action);
  });

  showHome();
  if (pidFile) {
    fs.mkdirSync(path.dirname(pidFile), { recursive: true });
    fs.writeFileSync(pidFile, `${process.pid}\n`, { mode: 0o600 });
  }
});

app.on("window-all-closed", () => app.quit());
app.on("before-quit", () => {
  if (pidFile) fs.rmSync(pidFile, { force: true });
});
