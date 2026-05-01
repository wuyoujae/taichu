# 元灵模块基础架构

本阶段先搭建“元灵”的基础骨架，不实现具体业务逻辑。  
目标是先把结构统一起来，后续再按子模块逐个实现。

## 目录映射

- 后端代码：`app/backend/src/yuanling`
- 文档：`docs/yuanling`
- 模块构成：`ai`、`tools`、`skills`、`mcp`、`memory`、`agent`
- `context`（会话级上下文与 session 管理）

## 当前状态

每个子模块均已建立最小占位文件，包含：
- 统一的配置结构或状态结构
- 默认配置/默认注册项
- 预留的扩展点（注册、执行、接口）
- `context` 增加了基础配置项：会话上下文保留策略、会话 TTL、系统提示词保留策略。

## Context 与 Memory 的职责边界

- `context`：按 session 管理上下文，关注一次对话窗口、生命周期和历史裁剪。
- `memory`：全局记忆配置，关注跨会话持久化策略（当前版本为占位能力）。

## Context 模块（会话上下文）

Context 模块是 Yuanling 的会话级上下文管理层。上层传入 `session_id` 后，context 负责加载该 session 的历史消息、追加新消息、构建本轮模型请求可用的上下文，并在上下文超过阈值时进行 compact。

### 核心数据结构

- `ContextSession`：会话容器，包含 `session_id`、创建/更新时间、消息列表、压缩记录和模型信息。
- `ContextMessage`：会话消息，包含角色、内容块和可选 token 使用信息。
- `ContextBlock`：消息内容块，支持 `text`、`tool_use`、`tool_result`。
- `ContextCompaction`：压缩记录，包含压缩次数、移除消息数、摘要和压缩时间。
- `ContextPromptEntry`：用户 prompt 历史记录。
- `ContextFork`：session fork 来源记录。
- `ContextUsageSummary`：session 级 token 使用汇总。

### 持久化格式

- 默认存储目录：`{BACKEND_DATA_DIR}/yuanling/context/sessions`
- 文件名：`{session_id}.jsonl`
- JSONL 记录类型：
  - `session_meta`
  - `message`
  - `compaction`
  - `prompt_history`

### Compact 策略

- token 估算先使用本地规则：`字符数 / 4 + 1`。
- 超过 `YUANLING_CONTEXT_COMPACT_THRESHOLD_TOKENS` 时触发压缩。
- 压缩后保留最近 `YUANLING_CONTEXT_PRESERVE_RECENT_MESSAGES` 条消息。
- 旧消息优先通过主 AI 配置生成语义摘要；AI compact 失败时回退到本地 deterministic 摘要。
- 压缩后的摘要会写成一条 synthetic `system` summary，放在上下文开头。
- compact 边界会避免切断 `tool_use` 与 `tool_result` 的配对。
- compact 后会执行本地 health check，确认摘要、最近消息、工具配对和 token 下降情况正常。

### Session 生命周期

- `tail_turns` 模式会按 `YUANLING_CONTEXT_MAX_TURNS` 保留最近会话轮次。
- `tail_tokens` 模式优先按 token 阈值 compact。
- session TTL 到期后默认归档旧 JSONL 文件，并返回新的空 session。
- session 文件超过 `YUANLING_CONTEXT_ROTATE_AFTER_BYTES` 时会轮转，最多保留 `YUANLING_CONTEXT_MAX_ROTATED_FILES` 个历史文件。

### 内部调用方式

- `load_session(session_id, config)`：加载 session，不存在则返回空 session。
- `save_session(session, config)`：保存完整 session。
- `append_message(session_id, message, config)`：追加消息并持久化。
- `append_prompt_entry(session_id, text, config)`：追加用户 prompt 历史。
- `fork_session(parent_session_id, new_session_id, branch_name, config)`：复制一个 session 分支。
- `build_context(session_id, config)`：加载 session，并在必要时 compact 后返回可用于模型请求的上下文。
- `compact_session(session, config)`：对已有 session 执行本地 compact。
- `compact_session_with_ai(session, config, ai_config)`：通过 AI 模块生成语义 summary，失败时回退本地 compact。

## AI 模块（参数能力）

AI 模块支持 OpenAI 兼容与 Anthropic 兼容端点，并按供应商能力处理参数：

### 核心参数

- `model`
- `max_tokens`
- `messages`
- `system`
- `stream`

### 工具参数

- `tools`
- `tool_choice`

### 采样与控制参数

- `temperature`
- `top_p`
- `frequency_penalty`
- `presence_penalty`
- `stop`
- `reasoning_effort`

### 能力对比

- `openai-compatible`：`top_p`、`frequency_penalty`、`presence_penalty`、`reasoning_effort` 全量支持；
- `anthropic-compatible`：`top_p`、`frequency_penalty`、`presence_penalty`、`reasoning_effort` 不支持；`stop` 映射为 `stop_sequences`；
- `system`：OpenAI 通过消息注入，Anthropic 作为顶层字段。

### 特殊处理

- OpenAI：`gpt-5*` 使用 `max_completion_tokens`。
- OpenAI：推理模型会清理采样参数，避免兼容模型报错。
- Anthropic：移除 Beta 兼容字段和 OpenAI-only 字段。
- 支持按供应商配置独立 `base_url` 与 `api_key`，并兼容自定义 endpoint：  
  - `YUANLING_OPENAI_BASE_URL` / `YUANLING_ANTHROPIC_BASE_URL`
  - `YUANLING_OPENAI_API_KEY` / `YUANLING_ANTHROPIC_API_KEY`
  - 以上变量缺失时回退到 `YUANLING_AI_BASE_URL`、`YUANLING_AI_API_KEY`。

### 发送链路验证

- 已验证两类端点都可正常发送：
  - OpenAI 兼容端点：
    - `POST {base_url}/chat/completions`
    - 测试 URL：`https://api.deepseek.com/chat/completions`
  - Anthropic 兼容端点：
    - `POST {base_url}/v1/messages`
    - 测试 URL：`https://api.deepseek.com/anthropic/v1/messages`

### 开发约定（本轮）

- 本轮仅补齐端点发送与参数兼容逻辑，不新增任何新的 API 接口。

### 公开接口

- `GET /yuanling/ai/config`：查询配置与能力。
- `POST /yuanling/ai/compose`：本地验证参数组装结果，返回 `request` 与 `skipped_params`。

### Context 基础配置（env）

- `YUANLING_CONTEXT_ENABLED`：是否启用 context 模块（布尔）
- `YUANLING_CONTEXT_SESSION_ENABLED`：是否启用会话上下文模式（布尔）
- `YUANLING_CONTEXT_SESSION_TTL_MINUTES`：会话有效期（分钟）
- `YUANLING_CONTEXT_RETENTION_MODE`：`tail_turns` 或 `tail_tokens`
- `YUANLING_CONTEXT_MAX_TURNS`：每会话最多保留轮次
- `YUANLING_CONTEXT_MAX_TOKENS`：每会话上下文 token 上限
- `YUANLING_CONTEXT_KEEP_SYSTEM_PROMPT`：是否保留系统提示词到上下文
- `YUANLING_CONTEXT_STORAGE_DIR`：session JSONL 存储目录，默认跟随 `BACKEND_DATA_DIR`
- `YUANLING_CONTEXT_AUTO_COMPACT_ENABLED`：是否自动 compact
- `YUANLING_CONTEXT_COMPACT_THRESHOLD_TOKENS`：触发 compact 的 token 阈值
- `YUANLING_CONTEXT_PRESERVE_RECENT_MESSAGES`：compact 后保留的最近消息数
- `YUANLING_CONTEXT_AI_COMPACT_ENABLED`：是否启用 AI 语义 compact
- `YUANLING_CONTEXT_COMPACT_MAX_OUTPUT_TOKENS`：AI compact summary 最大输出 token
- `YUANLING_CONTEXT_COMPACT_SYSTEM_PROMPT`：AI compact 的系统提示词
- `YUANLING_CONTEXT_INPUT_TOKEN_PRICE_PER_1M`：输入 token 单价，用于本地成本估算
- `YUANLING_CONTEXT_OUTPUT_TOKEN_PRICE_PER_1M`：输出 token 单价，用于本地成本估算
- `YUANLING_CONTEXT_ROTATE_AFTER_BYTES`：session 文件轮转阈值
- `YUANLING_CONTEXT_MAX_ROTATED_FILES`：最多保留的轮转文件数
- `YUANLING_CONTEXT_EXPIRE_ACTION`：过期处理方式，默认 `archive`
