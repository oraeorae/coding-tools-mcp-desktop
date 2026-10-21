# 设计文档：origin/main 上游集成

## 概述

本次采用“固定引用、隔离工作树、Git 三方合并、逐冲突语义整合、全量回归”的集成方式。对应 FR-1、FR-2、FR-3 及全部非功能需求。

## 技术方案

| 类别 | 选择 | 理由 | 关联需求 |
|---|---|---|---|
| 恢复点 | 备份分支 + `git stash push -u` | 同时保护提交历史和未提交文件 | FR-1 |
| 合并 | `git merge --no-ff origin/main` | 保留双方历史并提供共同祖先的三方冲突 | FR-1、FR-2 |
| 冲突处理 | 逐文件阅读 base/ours/theirs 后组合 | 两侧都包含重要且独立的产品能力 | FR-2 |
| 验证 | Rust 全测、Svelte check、Vite/Tauri build | 覆盖工具内核、桌面 UI 和打包链 | FR-3 |

## 架构设计

```text
origin/main@e3ba181 ─┐
                    ├─ Git merge commit ── 冲突语义整合 ── 回归与审查
local HEAD@2dec90c ──┘                         │
dirty worktree ── stash ── restore ───────────┘
```

主要集成轴：

- `tools/file|exec|workspace`：组合上游有界扫描、跨平台路径与本地量化 allowlist。
- `mcp/listener|server|upstream`：组合本地 OAuth/历史期限与上游工作区级上游 MCP。
- `runtime|lib|commands`：组合上游启动参数与本地退出清理、服务恢复。
- `actions|browser|local_update|request_log`：上游缺少但本地必须保留。
- `workspace/model` 与前端表单：合并审计、上游 MCP、FRP TLS、allowed roots、研究看板字段。

## 数据模型

现有 `RuntimeConfig`、工作区配置和应用设置采用 serde 默认值兼容。上游新增字段必须保留默认值；本地字段不得重命名或丢弃。配置迁移以读取旧 `profiles.json` 成功且写回不丢字段为验收点。

## API 设计

| 接口 | 合并要求 | 关联需求 |
|---|---|---|
| MCP `/mcp` | 保留 OAuth、历史工具、30 工具目录并增加上游 MCP 聚合 | FR-2 |
| Actions | 保留 PDF 返回和认证契约 | FR-2 |
| Tauri commands | 注册审计/访问日志与现有 browser/research/update/window commands | FR-2 |
| Runtime lifecycle | 保留启动恢复和退出时 shutdown_all | FR-2、FR-3 |

## 文件结构

合并可能触及上游相对本地差异中的 141 个文件。重点新增目录为 `src-tauri/src/audit/`、`src-tauri/src/mcp/upstream.rs`、`src/lib/api/access-logs.ts`、`src/lib/api/audit.ts`、`src/lib/components/AuditSettingsCard.svelte`、`src/lib/components/UpstreamMcpForm.svelte` 和 `src/routes/settings/audit/`；重点冲突文件位于 `src-tauri/src/tools/`、`src-tauri/src/mcp/`、`src-tauri/src/runtime/`、`src-tauri/src/workspace/model.rs`、`src-tauri/src/lib.rs` 与工作区页面。

## 设计决策

### 决策 1：保留本地产品版本和增量能力（FR-2）

上游发布版本为 0.2.4，本地已是 0.3.7。选择以本地版本与本地独有功能为基线，逐项吸收上游功能，而不采用整树替换。

### 决策 2：不自动提交原有脏工作树（FR-1）

原有变更归用户所有。使用 stash 暂存后恢复，最终不擅自把所有变更压成一个提交；合并提交只记录已提交历史的上游合并，恢复后的工作树继续保持用户可审查状态。

## 测试策略

- 合并前后运行 `cargo test --manifest-path src-tauri/Cargo.toml`。
- 运行 `npm run check`、`npm run build` 和 `npm run desktop:build`。
- 针对冲突模块运行定向测试：路径安全、有界读取、exec、OAuth/MCP、runtime shutdown、审计和 macOS Info.plist。
- 扫描冲突标记、检查版本一致性，并用 GitNexus `detect-changes` 分析影响。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|---|---|---|
| 141 文件跨模块差异 | 高 | 固定 SHA、恢复点、按模块分批解决和全测 |
| 上游删除本地独有文件 | 高 | 合并前列出本地独有功能，禁止机械接受删除 |
| 配置模型字段冲突 | 高 | serde 默认兼容、配置 round-trip 测试 |
| 工具契约和数量变化 | 高 | registry、OpenAPI、连接器目录测试 |
| macOS 签名/TCC 改动 | 中 | Info.plist/Entitlements 测试和 bundle 构建 |

## 检查清单

- [x] 覆盖 FR-1、FR-2、FR-3
- [x] 文件和模块来自真实 diff
- [x] 决策与测试策略可执行
- [x] 高风险均有缓解措施
