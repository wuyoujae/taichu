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

用于确认元灵基础模块可用，当前返回状态中包含 AI 模块配置摘要与参数支持。

- `GET /yuanling/ai/config`

返回 AI 模块当前配置与参数支持。

- `POST /yuanling/ai/compose`

用于按供应商规则构造一次请求体，支持全部 AI 参数映射与兼容转换。

## 参数支持（AI 模块）

| 参数 | OpenAI 兼容 | Anthropic |
| --- | --- | --- |
| model | ✅ | ✅ |
| max_tokens | ✅ | ✅ |
| messages | ✅ | ✅ |
| system | ✅（注入系统消息） | ✅（顶层字段） |
| stream | ✅ | ✅ |
| tools | ✅ | ✅ |
| tool_choice | ✅ | ✅ |
| temperature | ✅ | ✅ |
| top_p | ✅ | ❌ |
| frequency_penalty | ✅ | ❌ |
| presence_penalty | ✅ | ❌ |
| stop | ✅ | ✅（转 `stop_sequences`） |
| reasoning_effort | ✅（推理模型优先） | ❌ |

### 特殊处理

- OpenAI：`gpt-5*` 自动使用 `max_completion_tokens`。
- OpenAI：推理模型（如 `o1/o3/o4/grok-3-mini/qwq/thinking`）会清理采样参数。
- Anthropic：自动清理不兼容字段（`frequency_penalty`/`presence_penalty`/`top_p`/`reasoning_effort` 等），并将 `stop` 映射为 `stop_sequences`。

### 典型请求示例

```bash
curl -X POST http://127.0.0.1:4000/yuanling/ai/compose \
  -H "Content-Type: application/json" \
  -d '{
    "model":"gpt-4o-mini",
    "max_tokens":1024,
    "messages":[{"role":"user","content":[{"type":"text","text":"请总结以下内容"}]}],
    "system":"You are YUANLING",
    "stream":true,
    "tools":[{"name":"note_summary","description":"摘要工具","input_schema":{"type":"object","properties":{}}}],
    "tool_choice":"auto",
    "temperature":0.7,
    "top_p":0.95,
    "frequency_penalty":0.0,
    "presence_penalty":0.0,
    "stop":["\n\n"],
    "reasoning_effort":"medium"
  }'
```

## 环境变量

- `YUANLING_AI_ENABLED`：是否启用 AI 模块。
- `YUANLING_AI_PROVIDER`：`openai-compatible` 或 `anthropic-compatible`。
- `YUANLING_AI_ENDPOINT`：覆盖 endpoint。
- `YUANLING_AI_REQUEST_PATH`：覆盖请求路径。
- `YUANLING_AI_API_KEY`：鉴权密钥（不会返回到接口）。
- `YUANLING_AI_MODEL`：模型名。
- `YUANLING_AI_PROMPT_TEMPLATE`：系统提示词模板（仅用于消息为空时补齐）。
- `YUANLING_AI_TIMEOUT_MS`：请求超时时间。
- `YUANLING_AI_AUTH_HEADER`：鉴权头名称（如 `x-api-key`）。
