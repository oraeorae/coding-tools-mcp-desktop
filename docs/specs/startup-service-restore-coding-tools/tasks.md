# 任务清单：启动时恢复上次运行的服务

## 概述

实现可配置的启动恢复功能，并以独立状态文件控制影响范围。

## 交付物清单（Scope-lock）

- **预计新建文件数**: 2 个生产文件
- **预计修改文件数**: 5 个生产文件
- **预计新增/修改函数数**: 约 12 个
- **交付物逐项列举**:
  1. `src-tauri/src/startup_restore.rs`
  2. `src-tauri/src/commands/startup_restore.rs`
  3. `src-tauri/src/commands/mod.rs`
  4. `src-tauri/src/lib.rs`
  5. `src-tauri/src/local_update.rs`
  6. `src/lib/api/settings.ts`
  7. `src/routes/settings/general/+page.svelte`

## 任务列表

### 阶段 1: 准备工作

- [x] 1.1 固化独立状态文件和生命周期接入方案，避开统一设置转换链
  - **证据块**: `src-tauri/src/settings/model.rs:102-120` 显示 `from_data/apply_to` 会复制全部全局设置；GitNexus 对 `from_data` 给出 24 个受影响符号和 12 条流程。
  - **涉及文件**: `docs/specs/startup-service-restore-coding-tools/`，约 260 行。
  - _需求: FR-1, FR-2, FR-3_ ｜ _设计: 设计决策 1、设计决策 2_

### 阶段 2: 核心实现

- [x] 2.1 实现启动恢复状态存储，提供原子写入、运行快照和失败隔离恢复
  - **证据块**: `src-tauri/src/local_update.rs:79-149` 已有延迟恢复和逐服务启动路径；`src-tauri/src/runtime/supervisor.rs:76-113` 已提供运行检查与活动服务集合。
  - **涉及文件**: `src-tauri/src/startup_restore.rs` 新建，预算 260 行；`src-tauri/src/commands/startup_restore.rs` 新建，预算 35 行。
  - _需求: FR-1, FR-2, FR-3_ ｜ _设计: 技术方案、数据模型、API 设计_

- [x] 2.2 接入 Tauri 生命周期和 IPC 注册，运行时跟踪并在启动时延迟恢复
  - **证据块**: `src-tauri/src/lib.rs:164-176` 是 setup 入口，`src-tauri/src/lib.rs:230-238` 区分真实退出与 WebView 重建。
  - **涉及文件**: `src-tauri/src/lib.rs` 修改约 18 行；`src-tauri/src/local_update.rs` 修改约 7 行；`src-tauri/src/commands/mod.rs` 修改约 4 行。
  - _需求: FR-1, FR-2, FR-3_ ｜ _设计: 架构设计_

- [x] 2.3 在通用设置页加入即时持久化开关和明确说明
  - **证据块**: `src/routes/settings/general/+page.svelte:20-44` 已有设置加载与保存模式；`src/lib/api/settings.ts:41-52` 已封装通用设置 IPC。
  - **涉及文件**: `src/lib/api/settings.ts` 修改约 12 行；`src/routes/settings/general/+page.svelte` 修改约 65 行。
  - _需求: FR-1_ ｜ _设计: API 设计_

### 阶段 3: 集成测试

- [x] 3.1 对照验收标准执行单测、静态检查、构建和本机重启验证
  - **证据块**: `src-tauri/src/local_update.rs:152-180` 展示生命周期恢复单测模式；`package.json` 定义前端检查与构建脚本。
  - **涉及文件**: `src-tauri/src/startup_restore.rs` 内置单测；本机安装验证不新增测试文件。
  - _需求: FR-1, FR-2, FR-3, NFR-1, NFR-2, NFR-3_ ｜ _设计: 测试策略_

## 检查点

- [x] 阶段 1 完成后：规格校验通过，影响范围已报告。
- [x] 阶段 2 完成后：生产代码与交付物清单数量一致。
- [x] 阶段 3 完成后：自动测试通过且本机重启可见恢复成功。

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---------|----------|----------|------|
| FR-1 | API 设计 | 2.1, 2.2, 2.3, 3.1 | 已完成 |
| FR-2 | 设计决策 2 | 2.1, 2.2, 3.1 | 已完成 |
| FR-3 | 架构设计 | 2.1, 2.2, 3.1 | 已完成 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `src-tauri/src/startup_restore.rs` | 新建 | 260 | 状态读写、快照、恢复与单测 |
| `src-tauri/src/commands/startup_restore.rs` | 新建 | 35 | Tauri 命令 |
| `src-tauri/src/commands/mod.rs` | 修改 | 4 | 导出命令 |
| `src-tauri/src/lib.rs` | 修改 | 16 | 注册模块、调度与退出快照 |
| `src-tauri/src/local_update.rs` | 修改 | 7 | 暴露待处理升级接力检测 |
| `src/lib/api/settings.ts` | 修改 | 12 | 前端 IPC 封装 |
| `src/routes/settings/general/+page.svelte` | 修改 | 65 | 设置卡片与交互 |

## 检查清单

- [x] 交付物清单已锁定
- [x] 每条任务标题具体
- [x] 每条任务含证据块
- [x] 每条任务标注文件与行数预算
- [x] 任务分阶段合理
- [x] 每条任务回链 FR 与设计章节
- [x] 需求覆盖矩阵无遗漏
- [x] 阶段 3 对照验收标准核验
- [x] 全文无占位符
