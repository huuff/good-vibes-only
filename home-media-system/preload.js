const { contextBridge, ipcRenderer } = require("electron");

if (location.protocol === "file:") {
  contextBridge.exposeInMainWorld("homeMedia", {
    settings: () => ipcRenderer.invoke("settings"),
    open: (id, url) => ipcRenderer.send("open", id, url),
    power: (action) => ipcRenderer.send("power", action),
  });
}
