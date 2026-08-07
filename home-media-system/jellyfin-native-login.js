function runJellyfinNativeLogin(options, environment = globalThis) {
  const { document, Event } = environment;
  let attempts = 0;

  const isVisible = (element) => element && element.getClientRects().length > 0;
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
    const server = document.querySelector(
      "#address, #txtServer, input[autocomplete='url'], input[type='url']"
    );
    if (options.url && isVisible(server)) {
      setValue(server, options.url);
      environment.setTimeout(() => {
        const submit = server.closest("form")?.querySelector(
          "#connect-button, button[type='submit']"
        ) || document.querySelector("#connect-button, .btnConnect");
        if (submit && !submit.disabled) {
          submit.click();
          options.url = undefined;
          environment.setTimeout(fill, 250);
        }
      }, 250);
      return;
    }

    if (!options.username || !options.password) {
      if (attempts < 120) environment.setTimeout(fill, 250);
      return;
    }

    const username = document.querySelector("#txtManualName, input[autocomplete='username']");
    const password = document.querySelector("#txtManualPassword, input[autocomplete='current-password']");

    if (isVisible(username) && isVisible(password)) {
      setValue(username, options.username);
      setValue(password, options.password);
      environment.setTimeout(() => {
        const submit = username.closest("form")?.querySelector("button[type='submit']");
        if (submit && !submit.disabled) submit.click();
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
  delete window.jmpInfo.hmsServerUrl;
  delete window.jmpInfo.hmsAutoLogin;
  runJellyfinNativeLogin(options);
}

if (typeof module !== "undefined") module.exports = { runJellyfinNativeLogin };
