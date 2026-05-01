const { contextBridge } = require("electron");

contextBridge.exposeInMainWorld("taichuAPI", {
  appName: "Taichu Desktop",
  version: () => process.versions.electron,
});

