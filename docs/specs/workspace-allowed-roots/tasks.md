# 任务清单：workspace-allowed-roots

## 交付物清单

- 预计新建文件数：0
- 预计修改文件数：7
- 预计新增/修改函数数：约 12

## 任务列表

### 阶段 1：配置模型

- [ ] 1.1 在 RuntimeConfig 增加带 serde 默认值的 allowed_roots，并更新前端类型。
  - **证据块**：`src-tauri/src/workspace/model.rs:52-78` 当前 RuntimeConfig 字段；`src/lib/types.ts:20-35` 当前前端类型。
  - **涉及文件**：上述两个文件，各少于 100 行变更。
  - _需求: FR-1, FR-4_ ｜ _设计: 数据模型_

### 阶段 2：路径安全与集成

- [ ] 2.1 增加多根目录 Workspace 构造和读取边界匹配，保持写入边界为主根目录。
  - **证据块**：`src-tauri/src/tools/workspace.rs:125-335` 当前 canonicalize 和 ensure_inside_workspace 实现。
  - **涉及文件**：`src-tauri/src/tools/workspace.rs`，少于 180 行变更。
  - _需求: FR-2, FR-3_ ｜ _设计: 技术方案、安全决策_
- [ ] 2.2 将 RuntimeConfig.allowed_roots 传入 MCP listener，并在策略页面增加多行编辑。
  - **证据块**：`src-tauri/src/mcp/listener.rs:47-67` 当前 Workspace 构造；`src/routes/workspace/[id]/+page.svelte:400-425` 当前策略保存链路。
  - **涉及文件**：`src-tauri/src/mcp/listener.rs`、`src/routes/workspace/[id]/+page.svelte`、`src/lib/components/RuntimePolicyForm.svelte`，各少于 100 行变更。
  - _需求: FR-1, FR-4_ ｜ _设计: 技术方案_

### 阶段 3：验证

- [ ] 3.1 增加并运行路径安全回归测试，确认允许读取、未授权拒绝和外部写入拒绝。
  - **证据块**：`src-tauri/tests/call_tool_security.rs:1-35` 当前符号链接越界测试。
  - **涉及文件**：`src-tauri/tests/call_tool_security.rs`。
  - _需求: FR-2, FR-3_ ｜ _设计: 测试策略_

## 需求覆盖矩阵

| 需求 ID | 设计章节 | 任务编号 | 状态 |
|---|---|---|---|
| FR-1 | 数据模型、技术方案 | 1.1, 2.2 | 未开始 |
| FR-2 | 技术方案 | 2.1, 3.1 | 未开始 |
| FR-3 | 安全决策 | 2.1, 3.1 | 未开始 |
| FR-4 | 数据模型、技术方案 | 1.1, 2.2 | 未开始 |

## 文件变更清单

| 文件 | 操作 | 说明 |
|---|---|---|
| `src-tauri/src/workspace/model.rs` | 修改 | 持久化配置 |
| `src-tauri/src/mcp/listener.rs` | 修改 | 传递配置 |
| `src-tauri/src/tools/workspace.rs` | 修改 | 多根读边界 |
| `src-tauri/tests/call_tool_security.rs` | 修改 | 回归测试 |
| `src/lib/types.ts` | 修改 | 前端类型 |
| `src/lib/components/RuntimePolicyForm.svelte` | 修改 | 编辑控件 |
| `src/routes/workspace/[id]/+page.svelte` | 修改 | 保存链路 |
