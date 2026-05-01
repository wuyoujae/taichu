# 元灵模块设计草图（第一版）

## 一句话
元灵 = 将「AI 调用能力 + 工具 + 技能 + MCP + 记忆 + Agent 循环」组合成可激活智能体。

## 目标边界（MVP）

- 先保证模块能被装配（可见结构）
- 能够注册/展示配置项
- 提供一个统一入口：`/yuanling/status`
- 预留后续接入真实 `ai` 服务商的路径
- 明确 `context`（session 上下文）与 `memory`（全局记忆）分工

## 子模块职责（第一版）

- `ai`：模型配置、参数（prompt/top_p/temperature）与供应商抽象
- `tools`：工具注册与可用集合
- `skills`：技能注册与可用集合
- `mcp`：MCP 连接器注册与可用状态
- `context`：会话上下文策略与 session 生命周期管理
- `memory`：全局记忆策略定义（窗口策略 / 全量策略）
- `agent`：agent loop 的基础执行入口与状态

## Context 设计边界

- `context` 只管理单个 `session_id` 对应的会话历史、上下文窗口和 compact。
- session 使用本地 JSONL 文件持久化，不依赖数据库。
- compact 优先复用主 AI 模块生成语义摘要；AI 不可用时回退本地 deterministic 摘要。
- `memory` 不参与当前 session 的上下文裁剪；它后续用于跨 session 的全局记忆。
- context 同时负责 session prompt history、fork、TTL 归档、JSONL 文件轮转、token usage 汇总和成本估算。
