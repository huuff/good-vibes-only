function runJellyfinNativeLogin(options, environment = globalThis) {
  const { document, Event } = environment;
  let attempts = 0;

  const log = (message) => environment.console?.info(`[HMS Jellyfin] ${message}`);
  const isVisible = (element) => {
    if (!element || element.hidden || element.getClientRects().length === 0) return false;
    const style = environment.getComputedStyle?.(element);
    return !style || (style.display !== "none" && style.visibility !== "hidden");
  };
  const queryFirst = (selectors) => {
    let firstMatch;
    for (const selector of selectors) {
      const element = document.querySelector(selector);
      if (element && !firstMatch) firstMatch = element;
      if (isVisible(element)) return element;
    }
    return firstMatch;
  };
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
    const server = queryFirst([
      "#address",
      "#txtServer",
      "input[autocomplete='url']",
      "input[type='url']",
    ]);
    if (options.url && isVisible(server)) {
      setValue(server, options.url);
      environment.setTimeout(() => {
        const submit = server.closest("form")?.querySelector(
          "#connect-button, button[type='submit']"
        ) || document.querySelector("#connect-button, .btnConnect");
        if (isVisible(server) && !server.disabled && isVisible(submit) && !submit.disabled) {
          log("submitting configured server URL");
          submit.click();
          options.url = undefined;
        }
        environment.setTimeout(fill, 250);
      }, 250);
      return;
    }

    if (!options.username || !options.password) {
      if (attempts < 120) environment.setTimeout(fill, 250);
      return;
    }

    const username = queryFirst([
      "#txtManualName",
      "#login-username",
      "input[autocomplete='username']",
      "input[name='username']",
    ]);
    const password = queryFirst([
      "#txtManualPassword",
      "#login-password",
      "input[autocomplete='current-password']",
      "input[name='password']",
    ]);

    if (isVisible(username) && isVisible(password)) {
      log("filling username and password");
      setValue(username, options.username);
      setValue(password, options.password);
      environment.setTimeout(() => {
        const submit = username.closest("form")?.querySelector(
          "#login-button, button[type='submit'], .btnSubmit"
        ) || document.querySelector("#login-button, .btnSubmit");
        if (submit && !submit.disabled) {
          log("submitting login form");
          submit.click();
          options.username = undefined;
          options.password = undefined;
        }
      }, 250);
      return;
    }

    const cards = [...document.querySelectorAll("#divUsers .card")];
    const account = cards.find((element) => {
      const name = element.querySelector("[data-username]")?.dataset.username;
      return name?.toLocaleLowerCase() === options.username.toLocaleLowerCase();
    });
    const manual = document.querySelector(".btnManual");
    if (isVisible(account)) account.click();
    else if (cards.length && isVisible(manual)) manual.click();

    if (attempts < 120) environment.setTimeout(fill, 250);
  };

  fill();
}

if (typeof window !== "undefined" && (window.jmpInfo?.hmsServerUrl || window.jmpInfo?.hmsAutoLogin)) {
  const options = {
    url: window.jmpInfo.hmsServerUrl,
    ...window.jmpInfo.hmsAutoLogin,
  };
  window.console?.info(
    `[HMS Jellyfin] bootstrap: server=${Boolean(options.url)} autoLogin=${Boolean(options.username && options.password)}`
  );
  delete window.jmpInfo.hmsServerUrl;
  delete window.jmpInfo.hmsAutoLogin;
  runJellyfinNativeLogin(options);
}

if (typeof module !== "undefined") module.exports = { runJellyfinNativeLogin };
