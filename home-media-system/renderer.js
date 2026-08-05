const icons = {
  jellyfin: `<svg viewBox="0 0 64 64" aria-hidden="true"><defs><linearGradient id="jf" x1="0" y1="1" x2="1" y2="0"><stop stop-color="#20b7d8"/><stop offset="1" stop-color="#aa5cc3"/></linearGradient></defs><path fill="url(#jf)" d="M32 7c-6 0-23 29-20 38 3 9 37 9 40 0C55 36 38 7 32 7Zm0 11c3 0 14 20 12 24-2 4-22 4-24 0-2-4 9-24 12-24Zm0 8c-2 0-8 12-7 14 1 2 13 2 14 0 1-2-5-14-7-14Z"/></svg>`,
  jellyseerr: `<svg viewBox="0 0 64 64" aria-hidden="true"><defs><linearGradient id="js" x1=".15" y1=".15" x2=".85" y2=".9"><stop stop-color="#d49aff"/><stop offset=".55" stop-color="#9969ed"/><stop offset="1" stop-color="#5bcbea"/></linearGradient></defs><path fill="url(#js)" d="M10 27C10 15.4 19.7 6 31.7 6s21.7 9.4 21.7 21c0 8.8-4.7 13.4-11.7 16.1-2.3.9-4.2-.2-5.2-2.2-1.2 2.7-3.5 4.6-6.2 4.6-2.6 0-4.6-1.6-5.6-4-1.3 2.1-3.4 3.4-5.7 3.4-4.5 0-7.1-4.6-5.1-8.5A19.7 19.7 0 0 1 10 27Z"/><path fill="url(#js)" d="M17.5 40.5c-1.7 7.9-1.3 13.8.7 17.5 1-6.8 2.6-11.4 4.8-14m4-1.5c-1.1 8.1-.2 13.7 2.7 16.8.1-6.8 1.1-11.6 3-14.3m4.2-3.7c-.4 7.4.9 12.6 3.9 15.5-.5-6.1.1-10.6 1.8-13.5"/><circle cx="34" cy="24" r="10.5" fill="#e9e8ff"/><circle cx="36" cy="24" r="5" fill="#4554ba"/><circle cx="34.5" cy="22.5" r="1.7" fill="white"/></svg>`,
  youtube: `<svg viewBox="0 0 64 64" aria-hidden="true"><rect x="6" y="15" width="52" height="34" rx="10" fill="#ff101b"/><path d="m27 23 17 9-17 9Z" fill="white"/></svg>`,
  web: `<svg viewBox="0 0 64 64" aria-hidden="true"><circle cx="32" cy="32" r="23" fill="none" stroke="#aa8cf3" stroke-width="5"/><path d="M10 32h44M32 9c8 8 8 38 0 46M32 9c-8 8-8 38 0 46" fill="none" stroke="#aa8cf3" stroke-width="4"/></svg>`,
};

const builtInNames = {
  jellyfin: "Jellyfin",
  jellyseerr: "Jellyseerr",
  youtube: "YouTube",
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
  const weekday = new Intl.DateTimeFormat(settings.locale, { weekday: "long" }).format(now);
  const month = new Intl.DateTimeFormat(settings.locale, { month: "long" }).format(now);
  const day = new Intl.DateTimeFormat(settings.locale, { day: "numeric" }).format(now);
  document.querySelector("#date").textContent = `${weekday}, ${month} ${day}`;
}

function applicationName(application) {
  return application.name === application.id
    ? builtInNames[application.type] || application.name
    : application.name;
}

function addApplication(application) {
  const button = document.createElement("button");
  button.className = "app-card";
  button.setAttribute("aria-label", applicationName(application));
  button.innerHTML = `<span class="icon">${icons[application.type] || icons.web}</span>
    <span class="app-copy"><span class="app-name">${escapeHtml(applicationName(application))}</span></span>`;
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
  document.querySelector("#connect-title").textContent = `Connect to ${applicationName(application)}`;
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
