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

test("native Jellyfin login waits until the asynchronous form is visible", () => {
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
        if (selector.startsWith("#txtManualName")) return username;
        if (selector.startsWith("#txtManualPassword")) return password;
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

test("native Jellyfin login connects through Jellyfin Desktop 2.0's text address field", () => {
  const submit = {
    disabled: false,
    clicks: 0,
    click() { this.clicks += 1; },
  };
  const form = {
    querySelector(selector) {
      return selector.split(",").map((part) => part.trim()).includes("#connect-button")
        ? submit
        : null;
    },
  };
  const server = new Input(form, { id: "address", type: "text" });
  server.visible = true;
  const timers = [];
  const environment = {
    Event: class Event {},
    document: {
      querySelector(selector) {
        const selectors = selector.split(",").map((part) => part.trim());
        if (selectors.includes(`#${server.id}`)) return server;
        return null;
      },
    },
    setTimeout: (callback) => timers.push(callback),
  };

  runJellyfinNativeLogin({ url: "https://jellyfin.example.net" }, environment);

  assert.equal(server.type, "text");
  assert.equal(server.value, "https://jellyfin.example.net");
  assert.equal(submit.clicks, 0);
  timers.shift()();
  assert.equal(submit.clicks, 1);
});

test("native Jellyfin login remains compatible with the old connect screen", () => {
  const submit = {
    disabled: false,
    clicks: 0,
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
        const selectors = selector.split(",").map((part) => part.trim());
        return selectors.includes("#txtServer") ? server : null;
      },
    },
    setTimeout: (callback) => timers.push(callback),
  };

  runJellyfinNativeLogin({ url: "https://old-jellyfin.example.net" }, environment);

  assert.equal(server.value, "https://old-jellyfin.example.net");
  timers.shift()();
  assert.equal(submit.clicks, 1);
});
