# 任务清单：macOS MCP 隧道看门狗

## 概述

实现并部署独立的 macOS launchd 看门狗，覆盖 FR-1、FR-2、FR-3。

## 交付物清单（Scope-lock）

- **预计新建文件数**：仓库 6 个，用户运行目录 2 个。
- **预计修改文件数**：0 个现有源码文件。
- **预计新增/修改函数数**：约 12 个 shell 函数。
- **交付物逐项列举**：
  1. `docs/specs/macos-coding/requirements.md`
  2. `docs/specs/macos-coding/design.md`
  3. `docs/specs/macos-coding/tasks.md`
  4. `scripts/macos/coding-tools-mcp-tunnel-watchdog.sh`
  5. `scripts/macos/install-coding-tools-mcp-tunnel-watchdog.sh`
  6. `scripts/macos/test-coding-tools-mcp-tunnel-watchdog.sh`
  7. 用户 Application Support 中的运行脚本副本
  8. 用户 LaunchAgents 中的 plist

## 任务列表

### 阶段 1：准备工作

- [x] 1.1 核对现有健康检查与运行管理边界，确认公网失败不会被运行时 supervisor 自动修复
  - **证据块**：`src-tauri/src/health/checker.rs:59` 仅返回公网检查结果；`src-tauri/src/runtime/supervisor.rs:423` 只依据本地监听端口刷新状态。
  - **涉及文件**：仅阅读现有源码。
  - _需求：FR-1_ ｜ _设计：架构设计_

### 阶段 2：核心实现

- [x] 2.1 实现无凭据健康检测和有界状态机，仅在本地健康且公网连续失败时恢复 tunnel
  - **证据块**：现有 `check_mcp_public_url` 接受公网 200/401/405，但没有连续失败、冷却或恢复动作。
  - **涉及文件**：`scripts/macos/coding-tools-mcp-tunnel-watchdog.sh`，预算 260 行以内。
  - _需求：FR-1、FR-2、FR-3_ ｜ _设计：技术方案、数据模型_

- [x] 2.2 实现可重复安装与安全卸载，加载用户级 LaunchAgent 且不操作 MCP 桌面进程
  - **证据块**：现有 tunnel 服务标签为 `com.robot.cloudflared.coding-tools-mcp`，运行于 `gui/<uid>`。
  - **涉及文件**：`scripts/macos/install-coding-tools-mcp-tunnel-watchdog.sh`，预算 180 行以内。
  - _需求：FR-2、FR-3_ ｜ _设计：接口设计、文件结构_

### 阶段 3：集成测试

- [x] 3.1 以 mock HTTP 与 launchctl 验证正常、连续失败、本地失败、冷却、限流和锁竞争
  - **证据块**：事故复现为本地 200、公网 530、cloudflared 进程存活但控制面无 connector。
  - **涉及文件**：`scripts/macos/test-coding-tools-mcp-tunnel-watchdog.sh`，预算 260 行以内。
  - _需求：FR-1、FR-2、FR-3_ ｜ _设计：测试策略_

- [x] 3.2 安装后执行真实端到端验收并确认看门狗正常路径不会重启 tunnel
  - **证据块**：恢复后的真实基线为公网 GET 200、未授权 POST 401、授权 `get_default_cwd` 返回量化工作区。
  - **涉及文件**：用户运行目录的脚本与 LaunchAgent plist。
  - _需求：FR-1、FR-2、FR-3_ ｜ _设计：测试策略_

## 检查点

- [x] 阶段 1 完成后：边界确认，看门狗仅处理公网 tunnel。
- [x] 阶段 2 完成后：三个脚本通过 Bash 语法检查，LaunchAgent plist 通过校验并成功加载。
- [x] 阶段 3 完成后：mock 测试通过；两次真实健康运行均退出 0，cloudflared PID 保持不变，两个公网入口返回 HTTP 200。

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---------|----------|----------|------|
| FR-1 | 架构设计 | 1.1、2.1、3.1、3.2 | 完成 |
| FR-2 | 有界自动恢复 | 2.1、2.2、3.1、3.2 | 完成 |
| FR-3 | 安全、可回滚运行 | 2.1、2.2、3.1、3.2 | 完成 |

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|------|------|----------|------|
| `scripts/macos/coding-tools-mcp-tunnel-watchdog.sh` | 新建 | 260 | 健康检测与恢复状态机 |
| `scripts/macos/install-coding-tools-mcp-tunnel-watchdog.sh` | 新建 | 180 | 安装、加载与卸载 |
| `scripts/macos/test-coding-tools-mcp-tunnel-watchdog.sh` | 新建 | 260 | mock 集成测试 |
| Application Support 运行脚本 | 新建 | 与源脚本一致 | 安装副本 |
| LaunchAgent plist | 新建 | 约 45 | 60 秒调度 |
