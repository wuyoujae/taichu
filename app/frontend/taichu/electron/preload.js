const { contextBridge, ipcRenderer } = require("electron");

contextBridge.exposeInMainWorld("taichuAPI", {
  appName: "Taichu Desktop",
  version: () => process.versions.electron,
  platform: process.platform,
  windowControls: {
    minimize: () => ipcRenderer.invoke("taichu-window-control", "minimize"),
    maximize: () => ipcRenderer.invoke("taichu-window-control", "maximize"),
    close: () => ipcRenderer.invoke("taichu-window-control", "close"),
  },
});
