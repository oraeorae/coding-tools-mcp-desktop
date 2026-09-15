# 设计文档：统一请求错误日志

## 概述

新增独立 `request_log` Rust 模块，负责结构化记录、脱敏约束、轮转、读取和清空。MCP 与 Actions 路由使用轻量 Axum middleware 捕获 HTTP 4xx/5xx；MCP handler 额外捕获 HTTP 200 中的协议错误；健康检查 command 将失败项目写入相应服务日志。Svelte 日志页面使用独立组件展示记录。

**对应需求:** FR-1、FR-2、FR-3、FR-4、NFR-1、NFR-2、NFR-3

## 技术方案

### 技术选型

| 类别 | 选择 | 理由 | 关联需求 |
|------|------|------|----------|
| 存储 | 每工作区、每服务 JSONL | 追加成本低，损坏单行不影响其他记录 | FR-1、FR-4 |
| 捕获 | Axum middleware 加 MCP 协议显式记录 | 覆盖 HTTP 错误和 HTTP 200 协议错误 | FR-1 |
| UI | 现有日志页内嵌 RequestErrorViewer，每 3 秒刷新 | 用户无需跳转，复用工作区和服务上下文 | FR-3 |
| 容量 | 1 MiB 单文件加一个轮转文件 | 总容量有界且保留近期上下文 | FR-4 |

### 架构设计

```text
MCP / Actions HTTP request
  -> request error middleware
  -> status >= 400 -> request_log::record

MCP JSON-RPC response
  -> error or isError -> request_log::record

Health command
  -> failed HealthItem -> request_log::record

RequestErrorViewer
  -> read_request_errors / clear_request_errors
  -> logs/<workspace>/request-errors-<service>.jsonl
```

## 数据模型

| 实体/字段 | 类型 | 约束 | 说明 |
|-----------|------|------|------|
| id | String | UUID v4 | 单条记录标识 |
| timestampMs | u64 | Unix 毫秒 | 发生时间 |
| service | String | mcp 或 actions | 服务过滤键 |
| source | String | http、rpc 或 health | 错误来源 |
| method | String | 不含头和正文 | HTTP 或 RPC 方法 |
| path | String | 不含查询参数 | 路径或健康检查标签 |
| status | Option<u16> | HTTP 状态 | 无状态码时为空 |
| code | String | 最长 80 字符 | 协议或错误分类 |
| message | String | 单行，最长 500 字符 | 可展示原因 |
| requestId | String | 最长 120 字符 | JSON-RPC ID，缺失为空 |
| tool | String | 最长 120 字符 | 工具名，缺失为空 |

## API 设计

| 方法/函数 | 路径/签名 | 入参 | 出参 | 关联需求 |
|-----------|-----------|------|------|----------|
| record_request_error | Rust 函数 | RequestErrorInput | 无，写入失败静默降级并输出 stderr | FR-1、FR-2、FR-4 |
| request_error_middleware | Axum middleware | Request、Next、上下文 | 原 Response | FR-1 |
| read_request_errors | Tauri command | workspace id、service | 最近 200 条 RequestErrorEntry | FR-3 |
| clear_request_errors | Tauri command | workspace id、service | 空 | FR-3 |

## 文件结构

```text
src-tauri/src/request_log.rs
src-tauri/src/mcp/listener.rs
src-tauri/src/actions/listener.rs
src-tauri/src/commands/logs.rs
src-tauri/src/commands/health.rs
src-tauri/src/commands/mod.rs
src-tauri/src/lib.rs
src/lib/api/logs.ts
src/lib/components/RequestErrorViewer.svelte
src/lib/components/LogViewer.svelte
```

## 设计决策

### 决策 1: 不读取响应正文（关联需求: FR-4）

**问题**: 响应正文可能提供更多错误细节，也可能包含命令输出、路径或用户内容。

**选项**:
1. 缓冲并解析所有响应正文。
2. 仅记录状态、路由元数据，并在已知安全的 MCP 协议位置添加有限字段。

**决策**: 选择选项 2。

**理由**: 从结构上避免意外保存密钥和工作区内容，同时保留定位所需的状态与入口信息。

### 决策 2: 独立错误日志文件（关联需求: FR-3、FR-4）

**问题**: 现有文本日志无结构且没有容量上限。

**选项**:
1. 修改通用 `append_profile_log`。
2. 新建有界 JSONL 存储。

**决策**: 选择选项 2。

**理由**: `append_profile_log` 被隧道关键链路广泛使用，独立模块可控制影响范围并支持结构化 UI。

## 测试策略

- Rust 单测验证字段清洗、轮转、损坏行容错、读取顺序和按服务清空。
- listener 测试验证错误中间件对 4xx 记录、对 2xx 不记录。
- 健康检查 command 的映射逻辑通过纯函数单测验证。
- 运行 `cargo test`、`npm run check` 和 `npm run build`。
- 安装到 `/Applications` 后发起一条无鉴权请求，验证 UI 和日志文件均出现 401 记录。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| HTTP middleware 进入主请求链 | 中 | 不修改响应，只在返回后检查状态；完整回归 MCP 与 Actions |
| 多请求并发写同一文件 | 中 | 进程内互斥锁保护轮转和追加 |
| 日志包含敏感信息 | 高 | 采集接口不接受头或正文，限制已知字段并截断 |
| Cloudflare 在源站前拦截 | 中 | 健康检查主动记录；界面说明本机无法被动看到未到达请求 |

## 检查清单

- [x] 技术方案与现有架构一致
- [x] 覆盖全部 FR
- [x] 文件结构对应真实代码库
- [x] 数据模型与接口契约清晰
- [x] 关键设计决策已记录
- [x] 测试策略可验证验收标准
