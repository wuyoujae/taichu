# Spiritkind 元族模块

元族（Spiritkind）是太初中所有基于元灵（Yuanling）派生出来的子智能体注册中心。元灵负责提供统一的基础能力，元族负责保存和管理具体子智能体的身份、职责、系统提示词、可用 tools、可用 skills、团队归属和启用状态。

## 模块职责

当前 `spiritkind` 只提供后端内部 Rust 能力，不暴露 HTTP 路由。

它负责：

- 创建和注册单个子智能体。
- 创建和注册三府团队。
- 保存元族注册表到本地文件。
- 生成联络表视图。
- 查询指定元灵 ID 对应的 prompt、tools 和 skills。
- 管理成员状态，例如启用、禁用、归档。

它不负责：

- 运行 Agent Loop。
- 管理上下文历史。
- 处理元灵之间的消息队列。
- 管理全局记忆。

这些职责分别由 `agent`、`context`、`contact` 和后续可插拔 `memory` 模块承担。

## 默认成员

首次加载注册表时，如果本地还没有 `registry.json`，模块会返回一个内存默认注册表：

- `000001`：司言（Verin），前台入口智能体。
- `000002`：司衡（Aegis），任务调度智能体。

默认注册表不会在加载时强制写盘；只有调用保存或注册函数后才会写入本地文件。

## 三府团队

三府（Triad Cell）是一种团队结构，由三个角色组成：

- 太一（Taiyi）：负责决策。
- 司工（Artifex）：负责执行。
- 司律（Lexon）：负责监督。

一个系统中可以存在多个三府，例如编程三府、邮件三府、文档三府。每个三府都有自己的 `team_id`、领域、职责描述和成员列表，并由司衡统一调度。

## 本地存储

默认存储目录：

```text
{BACKEND_DATA_DIR}/spiritkind
```

主注册文件：

```text
registry.json
```

可以通过环境变量覆盖：

```text
SPIRITKIND_ENABLED=true
SPIRITKIND_STORAGE_DIR=
```

当 `SPIRITKIND_STORAGE_DIR` 为空时，模块会使用默认目录。

## 内部调用入口

主要内部函数包括：

```rust
load_registry(config)
save_registry(registry, config)
register_member(request, config)
register_triad(request, config)
list_directory(config)
get_member(yuanling_id, config)
set_member_status(yuanling_id, status, config)
system_prompt_for(yuanling_id, config)
tools_for(yuanling_id, config)
skills_for(yuanling_id, config)
```

后续 `agent` 模块可以通过这些函数读取某个 `yuanling_id` 对应的系统提示词、工具白名单和技能白名单。

## 联络表视图

联络表用于展示子智能体之间的组织关系：

- 司言、司衡作为顶层角色单独展示。
- 三府成员按团队分组展示。
- 非团队成员保留在独立成员列表中，避免注册后不可见。

## 状态约定

状态字段使用数字语义：

- `1`：enabled，启用。
- `2`：disabled，禁用。
- `3`：archived，归档。

禁用或归档不会删除成员，只会改变状态，便于后续恢复、审计或展示历史结构。
