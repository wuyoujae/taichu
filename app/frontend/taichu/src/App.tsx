import { OlympusWorkspace } from "./pages/OlympusWorkspace";
import { Minus, Square, X } from "lucide-react";
import { GlobalMessageProvider } from "./components/message";

const taichuApi = (window as Window & { taichuAPI?: { platform?: string; windowControls?: { minimize?: () => void; maximize?: () => void; close?: () => void } } }).taichuAPI;
const platform = (taichuApi?.platform || "unknown").toLowerCase();
const useCustomTitleBar = platform === "win32" || platform === "linux";
const useDarwinTitlebarSpace = platform === "darwin";

const route = window.location.pathname.toLowerCase();

export function App() {
  const isWorkspace =
    route === "/index" || route === "/index/" || route === "/" || route === "/index.html";
  const content = isWorkspace ? (
    <OlympusWorkspace />
  ) : (
    <main style={{ padding: 40, fontFamily: "system-ui, sans-serif" }}>
      <h1>Taichu 本地桌面应用</h1>
      <p>当前已从 Next.js 切换为 Electron + React（Vite）骨架。</p>
      <ul>
        <li>前端窗口：Electron 主进程启动</li>
        <li>渲染框架：React + Vite</li>
        <li>运行命令：npm run dev</li>
      </ul>
    </main>
  );

  return (
    <div className="app-shell">
      <GlobalMessageProvider />
      {useDarwinTitlebarSpace ? <div className="darwin-titlebar-drag" aria-hidden="true" /> : null}
      {useCustomTitleBar ? (
        <header className="custom-titlebar">
          <div className="titlebar-drag" />
          <div className="window-controls">
            <button
              className="window-control minimize"
              type="button"
              aria-label="最小化"
              onClick={() => taichuApi?.windowControls?.minimize?.()}
            >
              <Minus size={14} />
            </button>
            <button
              className="window-control maximize"
              type="button"
              aria-label="最大化"
              onClick={() => taichuApi?.windowControls?.maximize?.()}
            >
              <Square size={12} />
            </button>
            <button
              className="window-control close"
              type="button"
              aria-label="关闭"
              onClick={() => taichuApi?.windowControls?.close?.()}
            >
              <X size={14} />
            </button>
          </div>
        </header>
      ) : null}
      <div className={`app-shell-content ${useCustomTitleBar ? "with-titlebar" : "without-titlebar"} ${useDarwinTitlebarSpace ? "with-darwin-titlebar" : ""}`}>
        {content}
      </div>
    </div>
  );
}
