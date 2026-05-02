# 元灵模块设计草图（第一版）

## 一句话
元灵 = 将「AI 调用能力 + 内部通信 + 工具 + 技能 + MCP + 记忆 + Agent 循环」组合成可激活智能体。

## 目标边界（MVP）

- 先保证模块能被装配（可见结构）
- 能够注册/展示配置项
- 提供一个统一入口：`/yuanling/status`
- 预留后续接入真实 `ai` 服务商的路径
- 明确 `context`（yuanling context）与 `memory`（全局记忆）分工

## 子模块职责（第一版）

- `ai`：模型配置、参数（prompt/top_p/temperature）与供应商抽象
- `tools`：工具注册、发现、白名单过滤、权限元数据和执行器抽象
- `skills`：技能注册与可用集合
- `mcp`：MCP 连接器注册与可用状态
- `contact`：元灵之间的内部通信、接收队列和接收状态
- `context`：元灵长期上下文策略与 yuanling context 生命周期管理
- `memory`：全局记忆策略定义（窗口策略 / 全量策略）
- `agent`：agent loop 的基础执行入口与状态

## Agent 设计边界

- `agent` 是消息驱动运行层，负责把 contact、context、ai、tools、skills、mcp 串成被动循环。
- 默认身份约定：`000000=用户端点`、`000001=司言/默认入口`、`000002=司衡`；其中 `000000` 不运行 Agent Loop。
- 蓄洪/泄洪属于 agent：contact 只存 pending/inflight 和状态；agent 在目标 idle 时取消息处理，busy 时让消息留在 pending。
- AI 调用工具时进入 Agent Loop；工具结果写回 context，再进入下一轮 AI，直到 AI 不再调用工具。
- `send_message` 在 agent 内被包装：投递到用户 ID 只入用户收件箱；投递到其他元灵会加入 dispatch 队列并触发目标元灵。
- `YUANLING_AGENT_MAX_TOOL_ITERATIONS=0` 表示默认直到 AI 停止；设置为大于 0 时才做硬性截断。

## AI 设计边界

- `.env` AI 配置仍是系统默认模型配置，用于兼容已有 AI config、compose 和 send 调用。
- 多 AI 实例配置用于前端 `Agents` tab 和后续 spiritkind/agent 选择不同模型实例。
- AI 实例保存到 `{BACKEND_DATA_DIR}/yuanling/ai/instances.json`，可通过 `YUANLING_AI_INSTANCES_STORAGE_DIR` 覆盖目录。
- AI 实例包含 provider、base URL、request path、API key、model、prompt template、timeout、auth header、stream、max tokens 和常用采样参数。
- API key 可以本地持久化，但后端读取接口只返回 `has_api_key`，不回传明文 key。
- 更新实例时不传或传空 `api_key` 表示保留旧 key，只有显式传入新 key 才覆盖。
- `Test Run` 会向实例发送一条测试消息，并用非流式请求返回完整模型回复，方便前端直接确认该实例是否可用。
- 当前页面只负责配置 AI 实例，不负责把某个实例绑定到具体 spiritkind 成员；绑定关系后续在元族或 agent 配置层实现。

## Context 设计边界

- `context` 只管理单个 `yuanling_id` 对应的长期消息历史、上下文窗口和 compact。
- yuanling context 使用本地 JSONL 文件持久化，不依赖数据库。
- compact 优先复用主 AI 模块生成语义摘要；AI 不可用时回退本地 deterministic 摘要。
- `memory` 不参与当前 yuanling context 的上下文裁剪；它后续用于跨 yuanling context 的全局记忆。
- context 同时负责 yuanling context prompt history、lineage、TTL 归档、JSONL 文件轮转、token usage 汇总和成本估算。

## Contact 设计边界

- `contact` 按 `yuanling_id` 管理元灵内部消息接收器，包含 pending 队列、inflight 队列和接收状态。
- 状态使用数字枚举：`1=idle`、`2=busy`、`3=disabled`。
- `send_message` 工具每次只发送一条消息，参数为 `from_yuanling_id`、`to_yuanling_id`、`content`。
- 目标元灵 busy 时不阻塞调用方，消息进入 pending 队列并立即返回 `queued`。
- v1 不在 contact 内直接调用 AI 或 agent loop；后续 runtime/agent 在目标 idle 时调用 contact 的取消息入口。

## Tools 设计边界

- `tools` 是工具能力管理层，不直接承担 MCP server 生命周期。
- MCP/LSP/插件等外部工具后续通过 runtime/plugin 工具定义注册进 `ToolRegistry`。
- 当前内建执行器已支持 `bash`、文件读写编辑、glob/grep、`WebFetch`、`WebSearch`、`ToolSearch`、`send_message`、`Skill`、MCP 管理/调用工具、`Sleep`、`REPL` 和 `PowerShell`。
- 每个基于元灵派生的智能体可以通过 allowed tools 控制自身工具集合。
- 工具权限由 registry 在执行前统一处理，具体工具 handler 不负责自行弹窗或判断风险。
- 当前通过 `ToolPermissionPrompter` trait 预留用户确认/拒绝入口，后续 agent 或前端实现该 trait 即可接入审批体验。
- 默认策略是只读工具自动允许，工作区写入和高风险工具必须确认；没有确认器时直接拒绝执行。
- tools 默认采用 `global` 文件系统作用域，符合太初运行在用户全局环境中的系统特性；文件类工具可以按当前用户权限访问绝对路径和相对路径。
- 若需要把某个派生智能体限制在固定目录内，可以将 `YUANLING_TOOLS_FILESYSTEM_SCOPE=workspace`，此时文件类工具只允许 workspace root 内相对路径，并拒绝绝对路径和路径穿越。
- `YUANLING_TOOLS_WORKSPACE_ROOT` 在 `global` 模式下作为相对路径解析根目录，在 `workspace` 模式下作为强约束边界。
- `WebSearch` 使用 DuckDuckGo HTML 搜索作为当前轻量实现，后续如果需要企业级稳定性，可以替换为正式搜索供应商，但不改变工具入口。
- `Skill` 是只读工具，会通过 skills 模块加载完整 `SKILL.md`；context 默认只注入 skills 清单，完整技能正文由此工具按需加载。
- MCP 在 tools 中有两种入口：固定工具 `MCP` / `ListMcpResources` / `ReadMcpResource` / `McpAuth`，以及 `registry_with_mcp_tools()` 动态发现后注册的 runtime tools。
- MCP 工具默认按 `danger_full_access` 处理，因为外部 MCP server 的真实能力不可由 Yuanling 静态判断；后续如果 MCP annotations 可稳定映射权限，再细化为只读或写入级别。

### Tools 状态管理

Tools 模块现在包含系统级工具状态管理，用来控制某个工具最终可以被哪些元灵使用。它和 `spiritkind` 的成员 tools 配置不是同一层：`spiritkind` 表示某个子智能体被配置了哪些工具，tools 状态管理表示系统是否最终允许它使用这些工具。

工具访问模式使用数字语义：

- `1`：`enabled_all`，所有元灵可用。
- `2`：`disabled_all`，所有元灵不可用。
- `3`：`allow_only`，只有指定元灵可用。
- `4`：`deny_only`，除指定元灵外都可用。

默认没有状态规则时等价于 `enabled_all`，不会改变现有行为。状态文件默认保存到 `{BACKEND_DATA_DIR}/yuanling/tools/tool_state.json`，可通过 `YUANLING_TOOLS_STATE_DIR` 覆盖目录。Agent Loop 会先读取 spiritkind 的工具白名单，再叠加 tools 状态规则，只把最终可用的工具注入给 AI。

## MCP 设计边界

- `mcp` 是外部 MCP server 的连接、发现、状态和调用管理层，不直接拥有工具权限策略。
- 当前 v1 可执行传输只支持 `stdio`，因为它最适合本地全局环境和用户自定义 server；`http`、`sse`、`ws` 先保留配置和 preflight 状态，但不会执行调用。
- MCP 工具使用限定命名：`mcp__{server}__{tool}`，避免和内置 tools、runtime tools、plugin tools 冲突。
- `McpServerManager` 负责 server 生命周期：spawn、initialize、tools/list、resources/list、resources/read、tools/call、shutdown。
- `runtime_tool_definitions()` 可把 MCP 发现到的工具转换成 tools 模块的 `RuntimeToolDefinition`，后续 agent 可以把这些定义注入 AI。
- `McpToolExecutor` 实现 tools 的 `ToolExecutor` trait，后续可以和带 runtime tools 的 `ToolRegistry` 组合执行 MCP 工具。
- `/yuanling/status` 展示 MCP 配置、preflight 和降级状态；前端配置页通过 `/yuanling/mcp/*` 最小 REST 接口管理本地 MCP server 配置。
- MCP preflight 目前只做轻量检查：stdio command 是否存在，远程传输是否属于当前可执行范围；不会为了 status 做完整握手，避免启动阻塞。
- MCP 页面配置保存到 `{BACKEND_DATA_DIR}/yuanling/mcp/servers.json`，同时继续兼容 `YUANLING_MCP_SERVERS_JSON`。env 同名 server 优先级更高。

## Skills 设计边界

- `skills` 是本地技能说明管理层，负责发现、加载、筛选、搜索和生成 AI 注入视图。
- skill 的核心载体是 `SKILL.md`，frontmatter 提供 `name` / `description` / `status`，正文提供可注入模型的 instructions。
- skills 不执行系统操作；如果需要执行能力，应通过 tools 或后续 agent loop 调度。
- 默认根目录是 `{BACKEND_DATA_DIR}/yuanling/skills`，额外 root 必须通过 `YUANLING_SKILLS_ROOTS` 显式配置。
- 为避免跨项目泄漏和注入风险，本模块不做无界祖先目录扫描。
- 后续 plugin/MCP/agent 生成的动态技能可通过 `RuntimeSkillDefinition` 注册到 `SkillRegistry`。
- context 构建时会自动注入一个轻量 skills 清单，告诉模型当前有哪些技能可用；完整 `SKILL.md` 不会默认进入上下文。
- tools 内置只读 `Skill` 工具，后续 agent loop 可在模型选择技能后加载完整技能说明，再把结果追加回元灵长期上下文。
- 技能状态使用数字语义：`1=active`、`2=disabled`、`3=deleted`。当前 active 才会被发现、搜索、注入和加载；disabled/deleted 会保留数据入口，方便后续实现删除、更新和禁用管理。
- 自动注入可通过 `YUANLING_SKILLS_AUTO_INJECT_ENABLED=false` 关闭，也可以用 `YUANLING_SKILLS_AUTO_INJECT_MAX_ITEMS` 控制注入数量。
- 前端配置页通过 `/yuanling/skills/*` 最小 REST 接口读取配置、安装本地 skill、加载 active skill prompt，并通过状态写回 frontmatter；删除采用 `status: deleted`，不物理移除用户文件。
