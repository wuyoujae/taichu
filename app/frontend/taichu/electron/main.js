const { app, BrowserWindow, ipcMain } = require("electron");
const path = require("node:path");

const isDevelopment = !app.isPackaged;
const DEV_URL = process.env.TAICHU_DEV_URL || "http://localhost:5174/index";
const SPLASH_PATH = path.join(__dirname, "splash.html");
const APP_PATH = path.join(__dirname, "../dist/index.html");

let mainWindow = null;
const isDarwin = process.platform === "darwin";

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function loadWithRetry(win, targetUrl) {
  const maxAttempts = 20;
  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    try {
      await win.loadURL(targetUrl);
      return;
    } catch (error) {
      if (attempt === maxAttempts) {
        throw error;
      }
      await sleep(250);
    }
  }
}

function createMainWindow() {
  mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    title: "Taichu Desktop",
    frame: isDarwin,
    titleBarStyle: isDarwin ? "hiddenInset" : "default",
    titleBarOverlay: false,
    autoHideMenuBar: true,
    show: false,
    backgroundColor: "#ffffff",
    webPreferences: {
      preload: path.join(__dirname, "preload.js"),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  mainWindow.on("ready-to-show", () => {
    mainWindow.show();
  });

  mainWindow.on("closed", () => {
    mainWindow = null;
  });

  if (isDevelopment) {
    mainWindow.webContents.on("did-fail-load", (_, errorCode, errorDescription, url) => {
      console.log(`[Taichu Dev] failed to load ${url}: ${errorCode} ${errorDescription}`);
    });
    mainWindow.webContents.on("did-finish-load", () => {
      console.log("[Taichu Dev] load finished");
    });
    mainWindow.webContents.on("did-start-loading", () => {
      console.log("[Taichu Dev] start loading app page");
    });
  }

  mainWindow.loadFile(SPLASH_PATH);

  return mainWindow;
}

ipcMain.handle("taichu-window-control", async (event, action) => {
  const senderWindow = BrowserWindow.fromWebContents(event.sender);
  const targetWindow = senderWindow || mainWindow;

  if (!targetWindow || targetWindow.isDestroyed()) {
    return { ok: false };
  }

  if (action === "minimize") {
    targetWindow.minimize();
    return { ok: true };
  }

  if (action === "maximize") {
    if (targetWindow.isMaximized()) {
      targetWindow.unmaximize();
    } else {
      targetWindow.maximize();
    }
    return { ok: true };
  }

  if (action === "close") {
    targetWindow.close();
    return { ok: true };
  }

  return { ok: false };
});

app.whenReady().then(async () => {
  createMainWindow();
  if (!mainWindow) return;

  try {
    if (isDevelopment) {
      await loadWithRetry(mainWindow, DEV_URL);
    } else {
      await mainWindow.loadFile(APP_PATH);
    }
  } catch (error) {
    console.log(`[Taichu] start page load failed: ${error.message}`);
  }

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createMainWindow();
      if (isDevelopment) {
        loadWithRetry(mainWindow, DEV_URL).catch((error) => {
          console.log(`[Taichu] reopen app load failed: ${error.message}`);
        });
      } else {
        mainWindow.loadFile(APP_PATH);
      }
    }
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});
