const { app, BrowserWindow } = require("electron");
const path = require("node:path");

const isDevelopment = !app.isPackaged;
const DEV_PATH = "/index";
const DEV_URL_RETRY_MS = 500;
const DEV_PORT_CANDIDATES = [5173, 5174, 5175, 5176, 5177, 5178, 5179];

async function canLoad(url) {
  try {
    const res = await fetch(url, { method: "HEAD" });
    return res.status < 500;
  } catch {
    return false;
  }
}

async function resolveDevUrl() {
  for (const port of DEV_PORT_CANDIDATES) {
    const candidateUrl = `http://127.0.0.1:${port}${DEV_PATH}`;
    if (await canLoad(candidateUrl)) {
      return candidateUrl;
    }
  }

  return null;
}

async function loadDevUrlWithRetry(window) {
  if (window.isDestroyed()) {
    return;
  }

  const url = await resolveDevUrl();
  if (!url) {
    setTimeout(() => {
      loadDevUrlWithRetry(window);
    }, DEV_URL_RETRY_MS);
    return;
  }

  if (window.isDestroyed()) {
    return;
  }

  try {
    await window.loadURL(url);
  } catch {
    // 如果当前候选端口读取失败，继续等待并重试
    if (!window.isDestroyed()) {
      const nextWindow = window;
      setTimeout(() => {
        loadDevUrlWithRetry(nextWindow);
      }, DEV_URL_RETRY_MS);
    }
  }
}

function createMainWindow() {
  const mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    title: "Taichu Desktop",
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  if (isDevelopment) {
    loadDevUrlWithRetry(mainWindow);
  } else {
    mainWindow.loadFile(path.join(__dirname, "../dist/index.html"));
  }
}

app.whenReady().then(() => {
  createMainWindow();

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createMainWindow();
    }
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});
