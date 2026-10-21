# 任务清单：统一请求错误日志

## 概述

实现结构化请求错误采集、持久化和桌面展示，并验证现有 MCP、Actions、隧道和升级行为不受影响。

## 交付物清单（Scope-lock）

- **预计新建文件数**: 2 个生产代码文件、3 个规格文件
- **预计修改文件数**: 8 个生产代码文件
- **预计新增/修改函数数**: 约 16 个
- **交付物逐项列举**:
  1. `src-tauri/src/request_log.rs`
  2. `src/lib/components/RequestErrorViewer.svelte`
  3. `src-tauri/src/mcp/listener.rs`
  4. `src-tauri/src/actions/listener.rs`
  5. `src-tauri/src/commands/logs.rs`
  6. `src-tauri/src/commands/health.rs`
  7. `src-tauri/src/commands/mod.rs`
  8. `src-tauri/src/lib.rs`
  9. `src/lib/api/logs.ts`
  10. `src/lib/components/LogViewer.svelte`

## 任务列表

### 阶段 1: 准备工作

- [x] 1.1 分析日志和请求入口，完成影响评估并避开高风险通用日志函数
  - **证据块**: `src-tauri/src/commands/logs.rs:79-103` 只读取文本尾部；`src-tauri/src/mcp/listener.rs:184-275` 处理 MCP 请求；`src-tauri/src/actions/listener.rs:350-399` 返回 Actions 错误。GitNexus 判定 `append_profile_log` 为 CRITICAL，因此不修改该符号。
  - **涉及文件**: 仅分析，无生产代码修改
  - _需求: FR-1、FR-4_ ｜ _设计: 设计决策 2_

### 阶段 2: 核心实现

- [x] 2.1 实现有界 JSONL 错误存储，保证字段受控、损坏行容错和单服务清空
  - **证据块**: `src-tauri/src/tunnel/supervisor.rs:1125-1141` 的现有追加日志没有结构和轮转；新模块独立实现。
  - **涉及文件**: `src-tauri/src/request_log.rs`，预算 330 行；`src-tauri/src/lib.rs`，预算 5 行
  - _需求: FR-1、FR-4_ ｜ _设计: 数据模型、API 设计_
- [x] 2.2 接入 MCP、Actions 和健康检查错误捕获，不读取请求头与正文
  - **证据块**: `src-tauri/src/mcp/listener.rs:189-191` 的鉴权失败直接返回；`src-tauri/src/actions/listener.rs:379-398` 的工具错误返回 422；`src-tauri/src/commands/health.rs:17-20` 当前未持久化失败项目。
  - **涉及文件**: `src-tauri/src/mcp/listener.rs`、`src-tauri/src/actions/listener.rs`、`src-tauri/src/commands/health.rs`，各预算 10-35 行
  - _需求: FR-1、FR-2、FR-4_ ｜ _设计: 架构设计_
- [x] 2.3 提供读取与清空命令，并在日志页展示、复制和管理错误记录
  - **证据块**: `src/lib/components/LogViewer.svelte:20-32` 只刷新文本日志；`src-tauri/src/commands/logs.rs:79-103` 只有读取命令。
  - **涉及文件**: `src-tauri/src/commands/logs.rs`、`src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`、`src/lib/api/logs.ts`、`src/lib/components/RequestErrorViewer.svelte`、`src/lib/components/LogViewer.svelte`，新增组件预算 190 行，其余各 5-30 行
  - _需求: FR-3_ ｜ _设计: API 设计、文件结构_

### 阶段 3: 集成测试

- [x] 3.1 对照验收标准验证采集、安全、轮转、UI 和回归
  - **证据块**: `docs/project-context/how-to-test.md` 规定 Rust 单测、静态检查和生产构建；现有 listener 测试验证同步端口失败。
  - **涉及文件**: `src-tauri/src/request_log.rs` 内单测及现有构建配置
  - _需求: FR-1、FR-2、FR-3、FR-4_ ｜ _设计: 测试策略_

## 检查点

- [x] 阶段 1 完成后：规格、真实入口与影响范围一致
- [x] 阶段 2 完成后：错误可写入、读取、清空并在日志页展示
- [x] 阶段 3 完成后：测试、静态检查、构建与实际安装验证通过

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---------|----------|----------|------|
| FR-1 | 架构设计、API 设计 | 2.1、2.2、3.1 | 已验证 |
| FR-2 | 架构设计 | 2.2、3.1 | 已验证 |
| FR-3 | API 设计、文件结构 | 2.3、3.1 | 已验证 |
| FR-4 | 数据模型、设计决策 | 2.1、2.2、3.1 | 已验证 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `src-tauri/src/request_log.rs` | 新建 | 330 | 存储、中间件和单测 |
| `src/lib/components/RequestErrorViewer.svelte` | 新建 | 190 | 错误列表交互 |
| `src-tauri/src/mcp/listener.rs` | 修改 | 35 | HTTP 与协议错误捕获 |
| `src-tauri/src/actions/listener.rs` | 修改 | 15 | HTTP 错误捕获 |
| `src-tauri/src/commands/logs.rs` | 修改 | 25 | 读取和清空命令 |
| `src-tauri/src/commands/health.rs` | 修改 | 30 | 健康检查失败记录 |
| `src-tauri/src/commands/mod.rs` | 修改 | 5 | 命令导出 |
| `src-tauri/src/lib.rs` | 修改 | 8 | 模块和命令注册 |
| `src/lib/api/logs.ts` | 修改 | 30 | 前端 API 类型与调用 |
| `src/lib/components/LogViewer.svelte` | 修改 | 5 | 嵌入错误列表 |

## 检查清单

- [x] 交付物清单已锁定
- [x] 每条任务标题具体
- [x] 每条任务含证据块
- [x] 每条任务标注文件与行数预算
- [x] 单个新增文件预算不超过 500 行
- [x] 每条任务回链到 FR 与设计章节
- [x] 需求覆盖矩阵无遗漏
- [x] 阶段 3 对照验收标准核验
- [x] 全文无模板占位符

