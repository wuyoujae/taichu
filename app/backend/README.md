# Taichu Backend

技术栈：Rust + Axum  
用途：提供 API 服务基础入口与文件持久化运行目录，同时承载元灵模块骨架。

## 目录

- `src/main.rs`：服务启动与路由定义（含 `/health`）
- `src/yuanling`：元灵基础模块（`ai`、`tools`、`skills`、`mcp`、`memory`、`agent`）
- `.env.example`：环境变量示例

## 启动

```bash
cd app/backend
cp .env.example .env   # 可选
cargo run
```

默认监听地址：

- `http://127.0.0.1:4000`

## 默认接口

- `GET /health`

响应示例：

```json
{
  "status": "ok",
  "service": "taichu-backend"
}
```

- `GET /yuanling/status`

用于确认元灵基础模块可用。

