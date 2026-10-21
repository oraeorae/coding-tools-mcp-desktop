# 设计文档：启动时恢复上次运行的服务

## 概述

本设计以独立 `startup-restore.json` 保存开关和运行快照，避开高影响面的统一 `AppSettings` 数据转换链。应用运行期间定时从运行时监督器刷新活动服务，退出时再做一次尽力捕获；应用启动时延迟恢复，并在启动前检查服务是否已经运行。

**对应需求:** FR-1、FR-2、FR-3、NFR-1、NFR-2、NFR-3

## 技术方案

### 技术选型

| 类别 | 选择 | 理由 | 关联需求 |
|------|------|------|----------|
| 持久化 | 独立 JSON 文件 | 降低现有配置迁移和密钥链的风险 | FR-1, FR-2 |
| 写入 | 同目录临时文件后 rename | 避免部分写入 | FR-2, NFR-2 |
| 恢复 | Tauri 异步任务延迟 1500ms，必要时等待升级接力结果 | 不阻塞 UI，并确保升级接力先完成 | FR-3, NFR-1 |
| IPC | 两个窄命令读取和设置布尔开关 | 避免向前端暴露内部快照 | FR-1 |

### 架构设计

```text
通用设置页
  -> get_startup_restore_enabled / set_startup_restore_enabled
  -> data/startup-restore.json

应用运行期间（每秒，仅状态变化时写盘）
  -> capture_running_services
  -> RuntimeSupervisor.active_tunnel_service_keys
  -> 原子保存运行快照

RunEvent::ExitRequested（尽力补充）
  -> capture_running_services
  -> RuntimeSupervisor.active_tunnel_service_keys
  -> 原子保存运行快照

应用 setup
  -> local_update::schedule_restore (350ms)
  -> startup_restore::schedule_restore (1500ms，并等待待处理的升级接力)
  -> 已运行检查
  -> start_mcp_service / start_actions_service
  -> 恢复尝试结束后才开始每秒跟踪，避免空运行时覆盖待恢复快照
```

## 数据模型

| 实体/字段 | 类型 | 约束 | 说明 |
|-----------|------|------|------|
| `StartupRestoreState.enabled` | bool | 缺省 false | 用户开关 |
| `StartupRestoreState.services` | array | 缺省空数组 | 最近一次运行服务快照 |
| `ServiceSnapshot.workspaceId` | string | 非空工作区 ID | 目标工作区 |
| `ServiceSnapshot.mcp` | bool | 布尔值 | 是否恢复 MCP |
| `ServiceSnapshot.actions` | bool | 布尔值 | 是否恢复 Actions |

## API 设计

| 方法/函数 | 路径/签名 | 入参 | 出参 | 关联需求 |
|-----------|-----------|------|------|----------|
| `get_startup_restore_enabled` | Tauri command | 无 | bool | FR-1 |
| `set_startup_restore_enabled` | Tauri command | `enabled: bool` | void | FR-1 |
| `capture_running_services` | Rust internal | `&AppHandle` | `AppResult<()>` | FR-2 |
| `schedule_restore` | Rust internal | `AppHandle` | void | FR-3 |

## 文件结构

```text
src-tauri/src/
├── startup_restore.rs
├── commands/startup_restore.rs
├── commands/mod.rs
└── lib.rs
src/
├── lib/api/settings.ts
└── routes/settings/general/+page.svelte
```

## 设计决策

### 决策 1: 独立状态文件（关联需求: FR-1, FR-2）

**问题**: 将新字段加入 `AppSettings` 会经过广泛使用的 `from_data/apply_to` 链。

**选项**:
1. 修改 `profiles.json` 统一模型：文件数量少，但影响隧道、软件安装和运行时。
2. 独立状态文件：多一个小文件，但边界明确且可单独回滚。

**决策**: 选择独立状态文件。

**理由**: GitNexus 显示统一设置转换链影响 24 个符号和 12 条流程；独立文件显著降低回归风险。

### 决策 2: 低频跟踪并在退出时补充捕获（关联需求: FR-2）

**问题**: macOS 原生菜单退出可能绕过 Tauri 的 `ExitRequested` 回调，而每次服务状态变化写盘会侵入四条运行时命令并与重启流程耦合。

**选项**:
1. 每次启动、停止后更新快照。
2. 仅在真实应用退出时读取运行时状态并写快照。
3. 每秒读取一次状态，仅在快照变化时写盘，并在退出回调中再尽力捕获。

**决策**: 选择低频跟踪加退出补充捕获。

**理由**: 与“上次关闭应用时仍在运行”语义一致，覆盖原生退出和意外终止，同时不侵入运行时启动/停止热路径；状态未变化时不会写盘。

## 测试策略

- Rust 单测验证缺失文件默认关闭、JSON 往返、快照归并、关闭时不覆盖服务列表。
- Rust 全量测试验证生命周期与现有升级恢复兼容。
- Svelte 检查与构建验证 IPC 类型和页面模板。
- 本机安装后开启开关，保留一个服务运行，退出并重开应用，核对服务监听与 UI 状态。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 升级接力与普通恢复同时启动端口 | 中 | 普通恢复延后并在每个服务前检查运行状态 |
| 单个旧工作区已删除或端口冲突 | 中 | 每个服务独立处理并继续后续恢复 |
| WebView 重建误写空快照 | 高 | 仅在未调用 `prevent_exit` 的真实退出分支捕获 |
| 强制杀进程无法触发退出回调 | 低 | 运行期间每秒刷新，保留最近一次运行快照 |

## 检查清单

- [x] 技术方案与现有架构一致
- [x] 覆盖全部 FR
- [x] 文件路径与真实代码库一致
- [x] 数据模型与接口契约清晰
- [x] 关键设计决策已记录
- [x] 测试策略可验证验收标准
