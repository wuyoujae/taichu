# 元灵模块基础架构

本阶段先搭建“元灵”的基础骨架，不实现具体业务逻辑。  
目标是先把结构统一起来，后续再按子模块逐个实现。

## 目录映射

- 后端代码：`app/backend/src/yuanling`
- 文档：`docs/yuanling`
- 模块构成：`ai`、`contact`、`tools`、`skills`、`mcp`、`memory`、`agent`
- `context`（元灵长期上下文与 yuanling context 管理）

## 当前状态

每个子模块均已建立最小占位文件，包含：
- 统一的配置结构或状态结构
- 默认配置/默认注册项
- 预留的扩展点（注册、执行、接口）
- `context` 增加了基础配置项：元灵上下文保留策略、TTL、系统提示词保留策略。

## Context 与 Memory 的职责边界

- `context`：按 yuanling context 管理上下文，关注一次对话窗口、生命周期和历史裁剪。
- `contact`：按 `yuanling_id` 管理元灵之间的内部消息、接收队列和接收状态。
- `agent`：消息驱动的元灵运行循环，负责蓄洪/泄洪、AI 调用和工具调用循环。
- `memory`：全局记忆配置，关注跨元灵和全局持久化策略（当前版本为占位能力）。

## AI 模块（默认配置与多实例配置）

AI 模块负责统一管理模型供应商、请求地址、鉴权方式和默认采样参数。当前保留两层配置：

- `.env` 单实例配置：作为系统默认 AI 配置，继续服务 `/yuanling/ai/config`、`/yuanling/ai/compose` 和 `/yuanling/ai/send` 等兼容调用。
- 多 AI 实例配置：通过 `/yuanling/ai/instances` 系列接口创建、编辑、删除和测试多个 AI 实例，前端 `Agents` tab 会使用这套配置。

多实例配置默认保存到 `{BACKEND_DATA_DIR}/yuanling/ai/instances.json`，也可以通过 `YUANLING_AI_INSTANCES_STORAGE_DIR` 指定目录。每个实例包含名称、供应商、base URL、request path、API key、模型名、prompt template、timeout、auth header、stream、max tokens、temperature、top_p、penalty、stop 和 reasoning effort 等字段。API key 会保存到本地文件，但读取列表和详情时只返回 `has_api_key`，不会把明文 key 回传给前端。

### AI 实例接口

- `GET /yuanling/ai/instances`：读取 AI 实例列表。
- `POST /yuanling/ai/instances`：创建 AI 实例。
- `PUT /yuanling/ai/instances/{id}`：更新 AI 实例；不传或传空 `api_key` 时保留旧 key。
- `DELETE /yuanling/ai/instances/{id}`：删除 AI 实例。
- `POST /yuanling/ai/instances/{id}/test`：用该实例发送测试消息，返回连接结果与模型原始回复正文。测试接口固定使用非流式请求，方便前端直接展示完整回复。

## Tools 模块（工具注册、权限与执行）

Tools 模块是 Yuanling 的工具能力管理层。它负责维护可用工具定义、工具来源、权限元数据、白名单过滤、别名解析、搜索、权限确认和内建工具执行器。不同基于元灵派生的智能体，可以通过 allowed tools 白名单暴露不同工具集合。

### 核心能力

- 内置工具注册：`bash`、`read_file`、`write_file`、`edit_file`、`glob_search`、`grep_search`、`WebFetch`、`WebSearch`、`ToolSearch`、`send_message`、`Skill`、`MCP`、`ListMcpResources`、`ReadMcpResource`、`McpAuth`、`Sleep`、`REPL`、`PowerShell`。
- 工具来源：`builtin`、`runtime`、`plugin`。
- 权限元数据：`1=read_only`、`2=workspace_write`、`3=danger_full_access`。
- 别名解析：`read`、`write`、`edit`、`glob`、`grep`。
- 输出 `Vec<ToolDefinition>`，可直接用于 AI 模块的 `tools` 参数。
- 权限确认入口：通过 `ToolPermissionPrompter` trait 注入用户确认/拒绝逻辑，当前不新增 HTTP 审批接口。
- 内建执行器：`BuiltinToolExecutor` 支持文件、搜索、命令、网络获取、网络搜索、Sleep、REPL 和 PowerShell。
- MCP 支持：内置 `MCP` 工具可调用已配置 MCP server 的工具；`ListMcpResources` / `ReadMcpResource` 负责资源发现和读取；`McpAuth` 作为未来远程认证入口预留。
- Skills 支持：内置 `Skill` 工具会按当前 skills 配置加载完整 `SKILL.md` instructions。

### Tools 权限策略

Tools 的权限链在工具执行前完成，不由具体工具 handler 自行判断。默认策略为：

- `read_only`：自动允许，适合只读搜索、读取和工具发现。
- `workspace_write`：执行前需要确认，适合写文件、编辑文件等会改变工作区状态的工具。
- `danger_full_access`：执行前需要确认，适合外部网络请求或后续高风险系统能力。

上层 agent 或未来前端只需要实现 `ToolPermissionPrompter`，收到 `ToolPermissionRequest` 后返回 `allow` 或 `deny`。如果工具需要确认但调用方没有传入 prompter，registry 会直接拒绝执行，避免无确认地运行高风险能力。

测试或轻量集成时，可以用一个简单的 prompter 模拟用户选择：

```rust
struct AllowPrompter;

impl ToolPermissionPrompter for AllowPrompter {
  fn confirm(&mut self, _request: &ToolPermissionRequest) -> ToolPermissionDecision {
    ToolPermissionDecision::Allow
  }
}
```

### 已实现工具

| 工具 | 权限 | 说明 |
|------|------|------|
| `bash` | `danger_full_access` | 在 workspace 内通过 `bash -lc` 执行命令，支持超时。 |
| `read_file` | `read_only` | 读取 workspace 内文本文件，支持按行 `offset` / `limit`。 |
| `write_file` | `workspace_write` | 写入 workspace 内文本文件，必要时创建父目录。 |
| `edit_file` | `workspace_write` | 替换 workspace 内文件文本，支持单次或全量替换。 |
| `glob_search` | `read_only` | 使用轻量通配符匹配搜索 workspace 文件。 |
| `grep_search` | `read_only` | 使用 Rust regex 搜索 workspace 文件内容。 |
| `WebFetch` | `read_only` | 获取 URL 内容，并将 HTML 转换为可读文本。 |
| `WebSearch` | `read_only` | 通过 DuckDuckGo HTML 搜索返回标题与引用 URL。 |
| `ToolSearch` | `read_only` | 搜索当前 registry 中的工具定义。 |
| `send_message` | `read_only` | 向另一个元灵发送一条内部消息。 |
| `Skill` | `read_only` | 按需加载本地 skill 的完整 `SKILL.md` instructions。 |
| `MCP` | `danger_full_access` | 调用已配置 MCP server 暴露的工具。 |
| `ListMcpResources` | `read_only` | 列出已配置 MCP server 的资源。 |
| `ReadMcpResource` | `read_only` | 读取指定 MCP resource。 |
| `McpAuth` | `danger_full_access` | 预留 MCP 远程认证入口。 |
| `Sleep` | `read_only` | 使用线程 sleep，不占用 shell 进程。 |
| `REPL` | `danger_full_access` | 通过子进程执行 `python3`、`node` 或 `bash` 代码。 |
| `PowerShell` | `danger_full_access` | 使用本机 `pwsh` 或 `powershell` 执行命令；未安装时返回明确错误。 |

太初不是绑定到单个项目目录运行的系统，因此 tools 默认使用 `global` 文件系统作用域。在 `global` 模式下，文件类工具可以按当前用户权限处理绝对路径和相对路径；相对路径会基于 `YUANLING_TOOLS_WORKSPACE_ROOT` 或当前进程目录解析。若某个派生智能体需要被限制在固定目录内，可以将 `YUANLING_TOOLS_FILESYSTEM_SCOPE=workspace`，此时文件类工具只允许 workspace 内相对路径，并拒绝绝对路径和 `..` 路径穿越。

### 内部调用方式

- `ToolRegistry::builtin()`：构建内置工具注册表。
- `definitions(allowed_tools)`：返回可传给 AI 的工具定义。
- `normalize_allowed_tools(values)`：解析 allowed tools 白名单。
- `search(query, max_results)`：搜索工具名称、描述和来源。
- `execute(name, input, allowed_tools, executor)`：校验工具并路由到执行器。
- `execute_with_permissions(name, input, config, executor, prompter)`：按配置与用户确认策略执行工具。
- `BuiltinToolExecutor::from_config(config)`：根据 tools 配置创建内建工具执行器。
- `registry_with_mcp_tools(mcp_config)`：发现 MCP tools，并把它们作为 runtime tools 注册进 `ToolRegistry`。

### Tools 配置（env）

- `YUANLING_TOOLS_ENABLED`：是否启用 tools 模块。
- `YUANLING_TOOLS_ALLOWED`：工具白名单，空值表示允许全部已注册工具。
- `YUANLING_TOOLS_MAX_SEARCH_RESULTS`：默认搜索结果数量。
- `YUANLING_TOOLS_FILESYSTEM_SCOPE`：文件系统作用域，`global` 表示全局用户环境，`workspace` 表示限制在固定目录内。
- `YUANLING_TOOLS_WORKSPACE_ROOT`：内建工具执行时的路径解析根目录，空值表示当前进程目录。
- `YUANLING_TOOLS_AUTO_ALLOW_READ`：是否自动允许只读工具。
- `YUANLING_TOOLS_CONFIRM_WORKSPACE_WRITE`：工作区写入工具是否需要确认。
- `YUANLING_TOOLS_CONFIRM_DANGER_FULL_ACCESS`：高风险工具是否需要确认。

### Tools 状态管理

Tools 模块现在包含系统级工具状态管理，用来控制某个工具最终可以被哪些元灵使用。它和 `spiritkind` 的成员 tools 配置不是同一层：`spiritkind` 表示某个子智能体被配置了哪些工具，tools 状态管理表示系统是否最终允许它使用这些工具。

工具访问模式使用数字语义：

- `1`：`enabled_all`，所有元灵可用。
- `2`：`disabled_all`，所有元灵不可用。
- `3`：`allow_only`，只有指定元灵可用。
- `4`：`deny_only`，除指定元灵外都可用。

默认没有状态规则时等价于 `enabled_all`，不会改变现有行为。状态文件默认保存到 `{BACKEND_DATA_DIR}/yuanling/tools/tool_state.json`，可通过 `YUANLING_TOOLS_STATE_DIR` 覆盖目录。Agent Loop 会先读取 spiritkind 的工具白名单，再叠加 tools 状态规则，只把最终可用的工具注入给 AI。

## Contact 模块（元灵内部通信与接收状态）

Contact 模块负责元灵之间的内部消息投递和接收状态管理。它按 `yuanling_id` 维护一个本地接收器，包含 pending 队列、inflight 队列和当前接收状态。它不等同于 context：contact 管“消息有没有送到、目标能不能接收”，context 管“元灵长期上下文如何被模型使用”。

### 核心能力

- 支持 `send_message(from_yuanling_id, to_yuanling_id, content)` 单条消息投递。
- 目标元灵 `idle` 时，后续 runtime/agent 可调用取消息入口把 pending 移入 inflight 并开始处理。
- 目标元灵 `busy` 时，消息仍会进入 pending，工具立即返回 `queued`，不会阻塞调用方。
- 目标元灵 `disabled` 时拒绝接收新消息。
- 状态使用数字语义：`1=idle`、`2=busy`、`3=disabled`。
- v1 不直接调用 AI、agent loop、4 秒输入聚合、蓄洪或泄洪；这些属于后续 runtime/agent 层。

### Contact 配置（env）

- `YUANLING_CONTACT_ENABLED`：是否启用 contact 模块。
- `YUANLING_CONTACT_STORAGE_DIR`：contact JSON snapshot 存储目录，默认 `{BACKEND_DATA_DIR}/yuanling/contact`。

## Agent 模块（消息驱动循环）

Agent 模块负责把 contact、context、ai、tools、skills 和 mcp 串成完整运行循环。contact 只负责保存消息和状态；agent 负责在目标元灵空闲时泄洪处理 pending 消息，并在 AI 调用工具时持续循环。

### 核心能力

- 用户默认 ID：`000000`，默认入口元灵：`000001`，司衡默认 ID：`000002`。
- `receive_user_message(content)` 默认从 `000000` 发送到 `000001`。
- 目标是用户 ID 时只写入用户 contact，不启动 Agent Loop。
- 目标是可运行元灵且 idle 时，agent 取 pending 消息进入 busy 并开始处理。
- AI 返回 tool use 时执行工具，把 tool result 写回 context，再继续下一轮 AI。
- `send_message` 在 agent 内会触发目标元灵调度，但不会让工具本身递归调用 agent。

### Agent 配置（env）

- `YUANLING_AGENT_ENABLED`：是否启用 agent 模块。
- `YUANLING_AGENT_DEFAULT_USER_ID`：默认用户端点 ID，默认 `000000`。
- `YUANLING_AGENT_DEFAULT_ENTRY_ID`：默认入口元灵 ID，默认 `000001`。
- `YUANLING_AGENT_USER_IDS`：用户端点 ID 列表，默认 `000000`。
- `YUANLING_AGENT_STREAMING_ENABLED`：Agent 调用 AI 时是否请求流式。
- `YUANLING_AGENT_MAX_TOOL_ITERATIONS`：工具循环上限，`0` 表示直到 AI 停止。
- `YUANLING_AGENT_MAX_OUTPUT_TOKENS`：Agent 单次 AI 输出 token 上限。

## MCP 模块（外部 MCP server 连接与发现）

MCP 模块负责连接外部 Model Context Protocol server，发现其工具和资源，并把 MCP 工具转换成 tools 模块可接收的 runtime tools。它不负责前端路由，也不直接决定权限；权限仍由 tools 模块统一处理。

### 核心能力

- 支持 MCP JSON-RPC 2.0 Content-Length framing。
- 支持 `initialize`、`tools/list`、`tools/call`、`resources/list`、`resources/read`。
- 支持 stdio MCP server 的 spawn、初始化、发现、调用和 shutdown。
- 支持 MCP 工具限定命名：`mcp__{server}__{tool}`。
- 支持 best-effort 工具发现，部分 server 失败时返回 degraded report。
- 支持 `runtime_tool_definitions()`，可把 MCP tools 转换成 tools 模块的 `RuntimeToolDefinition`。
- 支持 `McpToolExecutor`，为后续 agent loop 执行 MCP 工具预留入口。
- `/yuanling/status` 会展示 MCP 配置、server preflight 状态和 degraded 状态。

### MCP 配置（env）

- `YUANLING_MCP_ENABLED`：是否启用 MCP 模块。
- `YUANLING_MCP_CONFIG_STORAGE_DIR`：MCP 页面配置存储目录，默认 `{BACKEND_DATA_DIR}/yuanling/mcp`。
- `YUANLING_MCP_SERVERS_JSON`：MCP server JSON 配置。
- `YUANLING_MCP_INITIALIZE_TIMEOUT_MS`：初始化握手超时时间。
- `YUANLING_MCP_LIST_TOOLS_TIMEOUT_MS`：工具发现超时时间。
- `YUANLING_MCP_TOOL_CALL_TIMEOUT_MS`：工具调用默认超时时间。
- `YUANLING_MCP_RESOURCE_TIMEOUT_MS`：资源读取超时时间。

stdio server 示例：

```json
{
  "filesystem": {
    "type": "stdio",
    "command": "python3",
    "args": ["server.py"],
    "env": {},
    "tool_call_timeout_ms": 30000
  }
}
```

### MCP 配置接口与前端页面

前端侧边栏 `MCP` 页面参考 `propertypes/mcp.html` 的 server 卡片和编辑面板设计，使用以下后端接口：

- `GET /yuanling/mcp/config`：读取 MCP 模块配置视图和轻量 preflight 状态。
- `GET /yuanling/mcp/servers`：读取 env 与本地文件合并后的 MCP server 列表。
- `POST /yuanling/mcp/servers`：创建或保存一个本地 MCP server 配置。
- `PUT /yuanling/mcp/servers/{name}`：更新指定 MCP server 配置。
- `DELETE /yuanling/mcp/servers/{name}`：从本地 MCP 配置文件移除 server；env 定义的 server 不会被物理修改。
- `POST /yuanling/mcp/discover`：执行 best-effort 工具发现，返回已发现工具、失败 server 和 unsupported server。

本地页面配置保存到 `{BACKEND_DATA_DIR}/yuanling/mcp/servers.json`，同时继续兼容 `YUANLING_MCP_SERVERS_JSON`。如果同名配置同时存在，env 配置具有更高优先级。

## Skills 模块（技能注册、发现与注入）

Skills 模块负责管理可注入给 AI 的本地技能说明。一个 skill 的核心载体是 `SKILL.md`：frontmatter 中声明 `name` 和 `description`，正文提供具体 instructions/prompt。skills 不等同于 tools，tools 是可执行函数，skills 是指导模型如何完成某类任务的能力说明。

### 核心能力

- 默认加载 `{BACKEND_DATA_DIR}/yuanling/skills/<skill>/SKILL.md`。
- 可通过 `YUANLING_SKILLS_ROOTS` 增加显式 root。
- 可选择加载用户级 `~/.taichu/skills`。
- 不进行无界祖先目录扫描，避免跨项目技能泄漏和技能注入。
- 支持 `SkillRegistry::discover(config)` 发现技能。
- 支持 `injections(allowed_skills)` 输出可注入 AI 的轻量技能清单。
- 支持 `load(skill, args, config)` 加载完整 `SKILL.md` prompt。
- 支持自动注入 context：默认只注入技能清单与描述，不直接塞入完整 `SKILL.md`，避免上下文膨胀。
- Tools 模块内置只读 `Skill` 工具，可在模型真正需要某个技能时按需加载完整 instructions。
- 支持技能状态：`1=active`、`2=disabled`、`3=deleted`，后续删除、更新和禁用能力会沿用这套状态入口。
- 支持 runtime skills 扩展入口，方便后续插件、MCP 或 agent 动态注册技能。

### SKILL.md 格式

```markdown
---
name: writer
description: Writing guidance for structured responses.
status: active
---

# Writer

Use this skill when the task requires polished writing.
```

### Skills 配置（env）

- `YUANLING_SKILLS_ENABLED`：是否启用 skills 模块。
- `YUANLING_SKILLS_ROOTS`：额外 skills 根目录，多个路径按系统路径分隔符分隔。
- `YUANLING_SKILLS_ALLOWED`：技能白名单，空值表示允许全部已发现技能。
- `YUANLING_SKILLS_INCLUDE_USER_HOME`：是否加载 `~/.taichu/skills`。
- `YUANLING_SKILLS_MAX_PROMPT_CHARS`：单个 skill prompt 最大加载字符数。
- `YUANLING_SKILLS_MAX_SEARCH_RESULTS`：技能搜索默认返回数量。
- `YUANLING_SKILLS_AUTO_INJECT_ENABLED`：是否自动把技能清单注入 context。
- `YUANLING_SKILLS_AUTO_INJECT_MAX_ITEMS`：单次注入 context 的最大技能数量。

### Skills 配置接口与前端页面

前端侧边栏 `Skills` 页面参考 `propertypes/skills.html` 的列表卡片、搜索过滤和详情面板设计，使用以下后端接口：

- `GET /yuanling/skills/config`：读取 skills 配置视图、root、统计信息和所有 skill 状态。
- `GET /yuanling/skills`：读取 skill 列表。
- `GET /yuanling/skills/search?q=keyword`：按名称、描述和来源搜索 skill。
- `GET /yuanling/skills/{id}`：加载 active skill 的完整 `SKILL.md` instructions。
- `POST /yuanling/skills/install`：从本地 `SKILL.md` 文件或包含 `SKILL.md` 的目录安装 skill。
- `PUT /yuanling/skills/{id}/status`：更新 skill 状态，`1=active`、`2=disabled`、`3=deleted`。

当前删除采用状态删除，不物理移除 `SKILL.md` 文件，避免误删用户本地技能资产。

## Context 模块（元灵长期上下文）

Context 模块是 Yuanling 的长期上下文管理层。上层传入 `yuanling_id` 后，context 负责加载该元灵的长期历史消息、追加新消息、构建本轮模型请求可用的上下文，并在上下文超过阈值时进行 compact。

### 核心数据结构

- `YuanlingContext`：元灵长期上下文容器，包含 `yuanling_id`、创建/更新时间、消息列表、压缩记录和模型信息。
- `ContextMessage`：上下文消息，包含角色、内容块和可选 token 使用信息。
- `ContextBlock`：消息内容块，支持 `text`、`tool_use`、`tool_result`。
- `ContextCompaction`：压缩记录，包含压缩次数、移除消息数、摘要和压缩时间。
- `ContextPromptEntry`：用户 prompt 历史记录。
- `ContextLineage`：yuanling context lineage 来源记录。
- `ContextUsageSummary`：yuanling context 级 token 使用汇总。

### 持久化格式

- 默认存储目录：`{BACKEND_DATA_DIR}/yuanling/context/yuanlings`
- 文件名：`{yuanling_id}.jsonl`
- JSONL 记录类型：
  - `context_meta`
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

### 元灵上下文生命周期

- `tail_turns` 模式会按 `YUANLING_CONTEXT_MAX_TURNS` 保留最近轮次。
- `tail_tokens` 模式优先按 token 阈值 compact。
- yuanling context TTL 到期后默认归档旧 JSONL 文件，并返回新的空 yuanling context。
- yuanling context 文件超过 `YUANLING_CONTEXT_ROTATE_AFTER_BYTES` 时会轮转，最多保留 `YUANLING_CONTEXT_MAX_ROTATED_FILES` 个历史文件。

### 内部调用方式

- `load_context(yuanling_id, config)`：加载 yuanling context，不存在则返回空 yuanling context。
- `save_context(context, config)`：保存完整 yuanling context。
- `append_message(yuanling_id, message, config)`：追加消息并持久化。
- `append_prompt_entry(yuanling_id, text, config)`：追加用户 prompt 历史。
- `clone_context(parent_yuanling_id, new_yuanling_id, branch_name, config)`：复制一个 yuanling context 分支。
- `build_context(yuanling_id, config)`：加载 yuanling context，并在必要时 compact 后返回可用于模型请求的上下文。
- `compact_context(context, config)`：对已有 yuanling context 执行本地 compact。
- `compact_context_with_ai(context, config, ai_config)`：通过 AI 模块生成语义 summary，失败时回退本地 compact。

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
- `YUANLING_CONTEXT_STATE_ENABLED`：是否启用元灵长期上下文状态（布尔）
- `YUANLING_CONTEXT_TTL_MINUTES`：元灵上下文有效期（分钟）
- `YUANLING_CONTEXT_RETENTION_MODE`：`tail_turns` 或 `tail_tokens`
- `YUANLING_CONTEXT_MAX_TURNS`：每元灵最多保留轮次
- `YUANLING_CONTEXT_MAX_TOKENS`：每元灵上下文 token 上限
- `YUANLING_CONTEXT_KEEP_SYSTEM_PROMPT`：是否保留系统提示词到上下文
- `YUANLING_CONTEXT_STORAGE_DIR`：yuanling context JSONL 存储目录，默认跟随 `BACKEND_DATA_DIR`
- `YUANLING_CONTEXT_AUTO_COMPACT_ENABLED`：是否自动 compact
- `YUANLING_CONTEXT_COMPACT_THRESHOLD_TOKENS`：触发 compact 的 token 阈值
- `YUANLING_CONTEXT_PRESERVE_RECENT_MESSAGES`：compact 后保留的最近消息数
- `YUANLING_CONTEXT_AI_COMPACT_ENABLED`：是否启用 AI 语义 compact
- `YUANLING_CONTEXT_COMPACT_MAX_OUTPUT_TOKENS`：AI compact summary 最大输出 token
- `YUANLING_CONTEXT_COMPACT_SYSTEM_PROMPT`：AI compact 的系统提示词
- `YUANLING_CONTEXT_INPUT_TOKEN_PRICE_PER_1M`：输入 token 单价，用于本地成本估算
- `YUANLING_CONTEXT_OUTPUT_TOKEN_PRICE_PER_1M`：输出 token 单价，用于本地成本估算
- `YUANLING_CONTEXT_ROTATE_AFTER_BYTES`：yuanling context 文件轮转阈值
- `YUANLING_CONTEXT_MAX_ROTATED_FILES`：最多保留的轮转文件数
- `YUANLING_CONTEXT_EXPIRE_ACTION`：过期处理方式，默认 `archive`
