const { contextBridge, ipcRenderer } = require("electron");

if (location.protocol === "file:") {
  contextBridge.exposeInMainWorld("homeMedia", {
    settings: () => ipcRenderer.invoke("settings"),
    volume: () => ipcRenderer.invoke("volume"),
    open: (id, url) => ipcRenderer.send("open", id, url),
    power: (action) => ipcRenderer.send("power", action),
  });
}
