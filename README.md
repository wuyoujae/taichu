# Taichu

Taichu 目前已切换为**本地桌面应用骨架**，默认使用：

- 前端：Electron + React（Vite）
- 后端：Rust Axum（保留）

## 目录结构

- `app/frontend/taichu`：Electron 桌面前端项目
- `app/backend`：Rust API 服务（文件持久化后端）
- `app/selection`：暂存/试验代码目录
- `app/test`：测试脚本和验证用例目录
- `docs`：项目文档
- `propertypes`：原型与设计素材
- `resources`：资源文件

## 快速开始

### 1. 启动桌面前端

```bash
cd app/frontend/taichu
npm install
npm run dev
```

启动后会同时打开 Electron 窗口。

### 2. 启动后端

```bash
cd app/backend
cp .env.example .env
cargo run
```

默认后端监听：`http://127.0.0.1:4000`

健康检查：
- `GET http://127.0.0.1:4000/health`

## 配置

- 前端环境变量示例：`app/frontend/taichu/.env.example`
- 后端环境变量示例：`app/backend/.env.example`

## 下一步

- 在 `app/backend/src/main.rs` 中添加业务路由
- 在 `app/frontend/taichu/src/App.tsx` 中构建桌面端首屏
- 在 `app/test` 中补充测试和联调脚本

