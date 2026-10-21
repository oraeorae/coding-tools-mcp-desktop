# 设计文档：workspace-allowed-roots

## 概述

本设计覆盖 FR-1、FR-2、FR-3、FR-4：在现有工作区边界模型上增加显式只读允许根目录，并保持写入和 host 执行边界不变。

## 技术方案

在 `RuntimeConfig` 增加 `allowed_roots: Vec<String>`。MCP listener 使用新的 `Workspace::with_allowed_roots` 构造器；Workspace 在构造时 canonicalize 主根目录和允许根目录。读取路径的边界检查改为匹配任一根目录，写入路径仍调用仅匹配主根目录的检查。

```text
RuntimeConfig.allowed_roots
        ↓
MCP listener
        ↓
Workspace { root, allowed_roots }
        ↓
read path: root OR allowed_roots
write path: root only
```

## 数据模型

| 字段 | 类型 | 约束 | 说明 |
|---|---|---|---|
| `RuntimeConfig.allowed_roots` | `Vec<String>` | 默认空；保存绝对路径 | MCP 读访问 allowlist |

## 文件结构

- `src-tauri/src/workspace/model.rs`：配置字段及默认值。
- `src-tauri/src/mcp/listener.rs`：把配置传入 Workspace。
- `src-tauri/src/tools/workspace.rs`：多根目录解析与边界校验。
- `src-tauri/tests/call_tool_security.rs`：安全回归测试。
- `src/lib/types.ts`、`RuntimePolicyForm.svelte`、工作区页面：配置编辑与保存。

## 安全决策

只允许读取，不把 allowed roots 作为 `filesystem_scope=host` 的别名；`resolve_for_write`、符号链接写入拒绝和受保护目录规则继续以主工作区为边界。

## 测试策略

- Rust 单测验证多根目录匹配、未授权路径拒绝、外部写入拒绝。
- TypeScript 检查配置字段能从页面保存到 RuntimeConfig。
- 运行受影响 Rust 测试及前端检查。
