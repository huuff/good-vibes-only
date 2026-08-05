const icons = {
  jellyfin: `<svg viewBox="0 0 64 64" aria-hidden="true"><defs><linearGradient id="jf" x1="0" y1="1" x2="1" y2="0"><stop stop-color="#20b7d8"/><stop offset="1" stop-color="#aa5cc3"/></linearGradient></defs><path fill="url(#jf)" d="M32 7c-6 0-23 29-20 38 3 9 37 9 40 0C55 36 38 7 32 7Zm0 11c3 0 14 20 12 24-2 4-22 4-24 0-2-4 9-24 12-24Zm0 8c-2 0-8 12-7 14 1 2 13 2 14 0 1-2-5-14-7-14Z"/></svg>`,
  jellyseerr: `<svg viewBox="0 0 64 64" aria-hidden="true"><defs><linearGradient id="js" x1="0" y1="1" x2="1" y2="0"><stop stop-color="#69d5f2"/><stop offset="1" stop-color="#ad63fa"/></linearGradient></defs><path fill="url(#js)" d="M17 13c8-8 26-5 30 6 4 12-1 28-13 34-10 5-23-2-23-14 0-8 2-20 6-26Z"/><circle cx="26" cy="27" r="7" fill="#e8dcff"/><circle cx="27" cy="27" r="3" fill="#443372"/><path d="M19 39c9 4 17 3 27-2" fill="none" stroke="#443372" stroke-width="3"/></svg>`,
  youtube: `<svg viewBox="0 0 64 64" aria-hidden="true"><rect x="6" y="15" width="52" height="34" rx="10" fill="#ff101b"/><path d="m27 23 17 9-17 9Z" fill="white"/></svg>`,
  web: `<svg viewBox="0 0 64 64" aria-hidden="true"><circle cx="32" cy="32" r="23" fill="none" stroke="#aa8cf3" stroke-width="5"/><path d="M10 32h44M32 9c8 8 8 38 0 46M32 9c-8 8-8 38 0 46" fill="none" stroke="#aa8cf3" stroke-width="4"/></svg>`,
};

const powerIcons = {
  sleep: `<svg viewBox="0 0 24 24"><path d="M20 15.5A8 8 0 0 1 8.5 4 8 8 0 1 0 20 15.5Z"/></svg>`,
  restart: `<svg viewBox="0 0 24 24"><path d="M20 7v5h-5M19 12a7 7 0 1 0-1 5"/></svg>`,
  poweroff: `<svg viewBox="0 0 24 24"><path d="M12 2v10m6.36-6.36a9 9 0 1 1-12.72 0"/></svg>`,
};

function updateClock(settings) {
  const now = new Date();
  document.querySelector("#time").textContent = new Intl.DateTimeFormat(settings.locale, {
    hour: "2-digit", minute: "2-digit", hour12: settings.clock === "12h",
  }).format(now);
  document.querySelector("#date").textContent = new Intl.DateTimeFormat(settings.locale, {
    weekday: "long", month: "long", day: "numeric",
  }).format(now);
}

function addApplication(application) {
  const button = document.createElement("button");
  button.className = "app-card";
  button.innerHTML = `<span class="icon">${icons[application.type] || icons.web}</span>
    <span class="app-copy"><span class="app-name">${escapeHtml(application.name)}</span>
    <span class="app-description">${escapeHtml(application.description)}</span></span>`;
  button.addEventListener("click", () => {
    if (application.nativeCommand || application.url) {
      window.homeMedia.open(application.id);
      return;
    }
    showConnect(application);
  });
  document.querySelector("#applications").append(button);
}

function normalizeServerUrl(value) {
  const candidate = /^https?:\/\//i.test(value) ? value : `http://${value}`;
  try {
    const parsed = new URL(candidate);
    return ["http:", "https:"].includes(parsed.protocol) && parsed.hostname ? parsed.href : null;
  } catch (_) {
    return null;
  }
}

function showConnect(application) {
  const modal = document.querySelector("#connect-modal");
  const input = document.querySelector("#server-url");
  document.querySelector("#connect-title").textContent = `Connect to ${application.name}`;
  document.querySelector("#connect-error").hidden = true;
  input.value = localStorage.getItem(`server:${application.id}`) || "";
  modal.dataset.application = application.id;
  modal.hidden = false;
  input.focus();
}

function escapeHtml(value) {
  const span = document.createElement("span");
  span.textContent = value;
  return span.innerHTML;
}

function addPowerAction(action, label) {
  const button = document.createElement("button");
  button.className = "power-action";
  button.innerHTML = `${powerIcons[action]}<span>${label}</span>`;
  button.addEventListener("click", () => window.homeMedia.power(action));
  document.querySelector("#power-actions").append(button);
}

window.homeMedia.settings().then((settings) => {
  document.title = settings.title;
  updateClock(settings);
  window.setInterval(() => updateClock(settings), 1000);
  settings.applications.forEach(addApplication);

  if (settings.power.sleep) addPowerAction("sleep", "Sleep");
  if (settings.power.restart) addPowerAction("restart", "Restart");
  if (settings.power.poweroff) addPowerAction("poweroff", "Shut down");

  const modal = document.querySelector("#modal");
  const connectModal = document.querySelector("#connect-modal");
  const powerButton = document.querySelector("#power-button");
  powerButton.addEventListener("click", () => {
    modal.hidden = false;
    document.querySelector(".power-action")?.focus();
  });
  modal.addEventListener("click", (event) => {
    if (event.target === modal) modal.hidden = true;
  });
  connectModal.addEventListener("click", (event) => {
    if (event.target === connectModal) connectModal.hidden = true;
  });
  document.querySelector("#connect-cancel").addEventListener("click", () => {
    connectModal.hidden = true;
    document.querySelector(".app-card")?.focus();
  });
  document.querySelector("#connect-form").addEventListener("submit", (event) => {
    event.preventDefault();
    const id = connectModal.dataset.application;
    const url = normalizeServerUrl(document.querySelector("#server-url").value.trim());
    if (!url) {
      document.querySelector("#connect-error").hidden = false;
      return;
    }
    localStorage.setItem(`server:${id}`, url);
    window.homeMedia.open(id, url);
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !modal.hidden) {
      event.stopPropagation();
      modal.hidden = true;
      powerButton.focus();
    }

    if (event.key === "Escape" && !connectModal.hidden) {
      event.stopPropagation();
      connectModal.hidden = true;
      document.querySelector(".app-card")?.focus();
    }

    if (event.target.matches("input") && event.key.startsWith("Arrow")) return;

    if (modal.hidden && connectModal.hidden) {
      const cards = [...document.querySelectorAll(".app-card")];
      const current = cards.indexOf(document.activeElement);
      let target;

      if (document.activeElement === powerButton) {
        if (event.key === "ArrowLeft" || event.key === "ArrowDown") target = cards.at(-1);
      } else if (current >= 0) {
        if (event.key === "ArrowRight") target = cards[current + 1] || powerButton;
        if (event.key === "ArrowLeft" && current > 0) target = cards[current - 1];
        if (event.key === "ArrowUp") target = powerButton;
      }

      if (target) {
        event.preventDefault();
        target.focus();
      }
      return;
    }

    const selector = !connectModal.hidden
      ? "#server-url, #connect-cancel, #connect-form button[type=submit]"
      : "#power-actions .power-action";
    const items = [...document.querySelectorAll(selector)];
    const current = items.indexOf(document.activeElement);
    let next = current;
    if (event.key === "ArrowRight" || event.key === "ArrowDown") next = Math.min(items.length - 1, current + 1);
    if (event.key === "ArrowLeft" || event.key === "ArrowUp") next = Math.max(0, current - 1);
    if (next !== current && next >= 0) {
      event.preventDefault();
      items[next].focus();
    }
  }, true);

  document.querySelector(".app-card")?.focus();
});
