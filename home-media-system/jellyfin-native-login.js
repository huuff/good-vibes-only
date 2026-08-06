function runJellyfinNativeLogin(credentials, environment = globalThis) {
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
    const username = document.querySelector("#txtManualName, input[autocomplete='username']");
    const password = document.querySelector("#txtManualPassword, input[autocomplete='current-password']");

    if (isVisible(username) && isVisible(password)) {
      setValue(username, credentials.username);
      setValue(password, credentials.password);
      environment.setTimeout(() => {
        const submit = username.closest("form")?.querySelector("button[type='submit']");
        if (submit && !submit.disabled) submit.click();
      }, 250);
      return;
    }

    const cards = [...document.querySelectorAll("#divUsers .card")];
    const account = cards.find((element) => {
      const name = element.querySelector("[data-username]")?.dataset.username;
      return name?.toLocaleLowerCase() === credentials.username.toLocaleLowerCase();
    });
    const manual = document.querySelector(".btnManual");
    if (isVisible(account)) account.click();
    else if (cards.length && isVisible(manual)) manual.click();

    if (attempts < 120) environment.setTimeout(fill, 250);
  };

  fill();
}

if (typeof window !== "undefined" && window.jmpInfo?.hmsAutoLogin) {
  const credentials = window.jmpInfo.hmsAutoLogin;
  delete window.jmpInfo.hmsAutoLogin;
  runJellyfinNativeLogin(credentials);
}

if (typeof module !== "undefined") module.exports = { runJellyfinNativeLogin };
