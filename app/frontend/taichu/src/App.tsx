import { OlympusWorkspace } from "./pages/OlympusWorkspace";

const route = window.location.pathname.toLowerCase();

export function App() {
  if (route === "/index" || route === "/index/" || route === "/" || route === "/index.html") {
    return <OlympusWorkspace />;
  }

  return (
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
}
