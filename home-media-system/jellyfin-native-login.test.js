const assert = require("node:assert/strict");
const test = require("node:test");
const { runJellyfinNativeLogin } = require("./jellyfin-native-login");

class Input {
  constructor(form, { id = "", type = "text", autocomplete = "" } = {}) {
    this.form = form;
    this.id = id;
    this.type = type;
    this.autocomplete = autocomplete;
    this.visible = false;
    this.currentValue = "";
  }

  get value() { return this.currentValue; }
  set value(value) { this.currentValue = value; }
  closest() { return this.form; }
  dispatchEvent() {}
  getClientRects() { return this.visible ? [{}] : []; }
}

test("native Jellyfin login supports the legacy login form after it becomes visible", () => {
  const submit = {
    disabled: false,
    clicks: 0,
    click() { this.clicks += 1; },
  };
  const form = { querySelector: () => submit };
  const username = new Input(form);
  const password = new Input(form);
  const timers = [];
  const environment = {
    Event: class Event {},
    document: {
      querySelector(selector) {
        if (selector === "#txtManualName") return username;
        if (selector === "#txtManualPassword") return password;
        return null;
      },
      querySelectorAll: () => [],
    },
    setTimeout: (callback) => timers.push(callback),
  };

  runJellyfinNativeLogin({
    username: "media-user",
    password: "test-password",
  }, environment);

  assert.equal(username.value, "");
  assert.equal(password.value, "");
  assert.equal(submit.clicks, 0);

  // Jellyfin resolves its public-user request after constructing the hidden
  // form. With no public users, it clears the username and reveals the form.
  username.value = "";
  username.visible = true;
  password.visible = true;
  timers.shift()();

  assert.equal(username.value, "media-user");
  assert.equal(password.value, "test-password");
  assert.equal(submit.clicks, 0);

  timers.shift()();
  assert.equal(submit.clicks, 1);
});

test("native Jellyfin login connects through Desktop 2.0 then auto-logs in", () => {
  const connect = {
    disabled: false,
    clicks: 0,
    getClientRects() { return [{}]; },
    click() {
      this.clicks += 1;
      server.visible = false;
      username.visible = true;
      password.visible = true;
    },
  };
  const login = {
    disabled: false,
    clicks: 0,
    click() { this.clicks += 1; },
  };
  const connectForm = {
    querySelector(selector) {
      return selector.split(",").map((part) => part.trim()).includes("#connect-button")
        ? connect
        : null;
    },
  };
  const loginForm = {
    querySelector(selector) {
      return selector.split(",").map((part) => part.trim()).includes("#login-button")
        ? login
        : null;
    },
  };
  const server = new Input(connectForm, { id: "address", type: "text" });
  const username = new Input(loginForm, { id: "login-username" });
  const password = new Input(loginForm, { id: "login-password", type: "password" });
  server.visible = true;
  const timers = [];
  const environment = {
    Event: class Event {},
    document: {
      querySelector(selector) {
        if (selector === `#${server.id}`) return server;
        if (selector === `#${username.id}`) return username;
        if (selector === `#${password.id}`) return password;
        return null;
      },
      querySelectorAll: () => [],
    },
    setTimeout: (callback) => timers.push(callback),
  };

  runJellyfinNativeLogin({
    url: "https://jellyfin.example.net",
    username: "media-user",
    password: "test-password",
  }, environment);

  assert.equal(server.type, "text");
  assert.equal(server.value, "https://jellyfin.example.net");
  assert.equal(connect.clicks, 0);
  timers.shift()();
  assert.equal(connect.clicks, 1);

  timers.shift()();
  assert.equal(username.value, "media-user");
  assert.equal(password.value, "test-password");
  assert.equal(login.clicks, 0);

  timers.shift()();
  assert.equal(login.clicks, 1);
});

test("native Jellyfin login remains compatible with the old connect screen", () => {
  const submit = {
    disabled: false,
    clicks: 0,
    getClientRects() { return [{}]; },
    click() { this.clicks += 1; },
  };
  const form = {
    querySelector(selector) {
      return selector.includes("button[type='submit']") ? submit : null;
    },
  };
  const server = new Input(form, { id: "txtServer", type: "url" });
  server.visible = true;
  const timers = [];
  const environment = {
    Event: class Event {},
    document: {
      querySelector(selector) {
        return selector === "#txtServer" ? server : null;
      },
    },
    setTimeout: (callback) => timers.push(callback),
  };

  runJellyfinNativeLogin({ url: "https://old-jellyfin.example.net" }, environment);

  assert.equal(server.value, "https://old-jellyfin.example.net");
  timers.shift()();
  assert.equal(submit.clicks, 1);
});

test("native Jellyfin login does not race Desktop's saved-server auto-connect", () => {
  const submit = {
    disabled: false,
    visible: true,
    clicks: 0,
    click() { this.clicks += 1; },
    getClientRects() { return this.visible ? [{}] : []; },
  };
  const form = {
    querySelector(selector) {
      return selector.includes("#connect-button") ? submit : null;
    },
  };
  const server = new Input(form, { id: "address", type: "text" });
  server.visible = true;
  const timers = [];
  const environment = {
    Event: class Event {},
    document: {
      querySelector(selector) {
        return selector === "#address" ? server : null;
      },
      querySelectorAll: () => [],
    },
    setTimeout: (callback) => timers.push(callback),
  };

  runJellyfinNativeLogin({ url: "https://jellyfin.example.net" }, environment);

  // Desktop's saved-server auto-connect starts while HMS is waiting to
  // submit, disabling and hiding the native form.
  server.disabled = true;
  submit.visible = false;
  timers.shift()();

  assert.equal(submit.clicks, 0);
  assert.equal(timers.length, 1);
});
