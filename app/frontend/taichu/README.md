# Taichu Desktop Frontend

这是基于 Electron + React + Vite 的桌面端基础模板。

## 运行

```bash
npm install
npm run dev
```

`npm run dev` 会启动 Vite 开发服务，并自动打开 Electron 窗口。

如果需要让前端连接本地后端，请先启动 `app/backend`，并在前端环境中配置：

```bash
VITE_BACKEND_BASE=http://127.0.0.1:4000
```

启动后进入主界面，点击左侧侧边栏的 `Agents` tab，即可打开 AI 实例配置页。该页面支持创建、编辑、删除和测试 AI 实例；API Key 在列表中只显示是否已配置，表单中使用密码框输入，后端读取接口不会回传明文。

## 目录说明

- `electron/`：Electron 主进程与预加载脚本
- `src/`：React 渲染端源码
- `vite.config.ts`：Vite 配置
