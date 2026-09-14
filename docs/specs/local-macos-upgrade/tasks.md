# 任务清单：local-macos-upgrade

## 交付物清单

- 本地 macOS 升级 Node 编排器及测试。
- `npm run install:local` 命令。
- 一次性服务接力 Rust 模块及测试。
- 0.3.3 五处版本同步、验证日志和本规格。

## 任务列表

## 阶段 1：规格与边界

- [x] 1.1 固定本地源码、macOS、无 sudo、精确进程和一次性服务接力边界
  - **证据块**：`docs/project-context/how-to-develop.md` 要求五处版本同步与构建门禁；`src-tauri/src/commands/runtime.rs` 已有单工作区 MCP/Actions 启动函数。
  - **涉及文件**：本规格目录，合计约 180 行。
  - _需求：FR-1 至 FR-5_｜_设计：方案、安全与回滚_

## 阶段 2：实现

- [x] 2.1 新增可测试的 Node 本地升级编排器并暴露 npm 命令
  - **证据块**：`package.json` 当前没有本地安装命令；现有手工流程使用 Tauri app bundle、codesign、ditto 与精确 PID 停止。
  - **涉及文件**：`scripts/install-local-macos.mjs` 不超过 500 行；`scripts/install-local-macos.test.mjs` 不超过 220 行；`package.json`。
  - _需求：FR-1、FR-2、FR-3、FR-5_｜_设计：升级流程、安全与回滚_

- [x] 2.2 新增一次性服务接力消费模块并复用现有启动链路
  - **证据块**：`src-tauri/src/lib.rs` setup 当前只初始化状态与托盘；`src-tauri/src/commands/runtime.rs` 的启动函数可复用完整资源、端口和隧道检查。
  - **涉及文件**：`src-tauri/src/local_update.rs` 不超过 250 行；`src-tauri/src/lib.rs`；`src-tauri/src/commands/runtime.rs`。
  - _需求：FR-4、FR-5_｜_设计：数据契约、升级流程_

- [x] 2.3 将应用版本从 0.3.2 同步递增至 0.3.3
  - **证据块**：`docs/project-context/how-to-develop.md` 明确安装包功能变更必须同步五处版本源。
  - **涉及文件**：`package.json`、`package-lock.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`、`src-tauri/tauri.conf.json`。
  - _需求：FR-2、FR-5_｜_设计：升级流程_

## 阶段 3：验证与安装

- [x] 3.1 运行 Node、Rust、Svelte 和 Cargo 测试及 dry-run
  - **证据块**：本功能横跨 Node 编排和 Rust 启动恢复，必须分别验证纯逻辑和集成入口。
  - **涉及文件**：测试日志与本规格任务状态。
  - _需求：FR-1 至 FR-5_｜_设计：测试_

- [ ] 3.2 提交源码后执行真实 `npm run install:local` 并核对 0.3.3、签名、配置与服务恢复
  - **证据块**：项目构建规则要求从已提交版本构建；当前安装版为 0.3.2。
  - **涉及文件**：本地构建产物、应用数据下非敏感恢复结果、备份目录。
  - _需求：FR-3、FR-4、FR-5_｜_设计：升级流程、安全与回滚_

## 需求覆盖矩阵

| 需求 | 任务 |
|---|---|
| FR-1 | 2.1、3.1 |
| FR-2 | 2.1、2.3、3.1 |
| FR-3 | 2.1、3.2 |
| FR-4 | 2.2、3.2 |
| FR-5 | 2.1、2.2、2.3、3.2 |

## 检查点

- [x] 范围与回滚边界明确。
- [x] 规格通过 `check_spec`。
- [x] 实现与测试通过。
- [ ] 真实本地升级通过。

## 文件变更清单

| 文件 | 操作 | 行数预算 | 说明 |
|---|---|---:|---|
| `scripts/install-local-macos.mjs` | 新建 | 500 | 本地升级编排 |
| `scripts/install-local-macos.test.mjs` | 新建 | 220 | Node 纯逻辑测试 |
| `package.json` | 修改 | 2 | npm 命令与测试入口 |
| `src-tauri/src/local_update.rs` | 新建 | 250 | 服务接力消费 |
| `src-tauri/src/lib.rs` | 修改 | 8 | 注册启动恢复任务 |
| `src-tauri/src/commands/runtime.rs` | 修改 | 2 | 复用服务启动函数 |
| 五处版本源 | 修改 | 10 | 0.3.3 同步 |
