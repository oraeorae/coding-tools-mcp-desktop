# 修改历史 / Changelog

本仓库源自 [xyTom/coding-tools-mcp](https://github.com/xyTom/coding-tools-mcp) 的早期实现，后续发展为独立的 Rust + Tauri 桌面版。这里记录本派生版本的主要改动；原项目自身的更新历史请以其仓库为准。

This repository was derived from an early version of [xyTom/coding-tools-mcp](https://github.com/xyTom/coding-tools-mcp) and subsequently evolved into an independent Rust + Tauri desktop edition. This file records the major changes in this derivative project; consult the original repository for its own release history.

## Unreleased

- 公开发布准备：补充原项目归属、派生关系和独立修改历史。
- 保留本地研究工作流、启动恢复、外部只读目录和 macOS 隧道守护等可靠性增强。

## 0.3.x — 2026-09 to 2026-10

- 增加本机浏览器桥接，可从 MCP 连接、读取和截图已授权的 Chrome 页面。
- 将长命令输出持久化为有界归档，支持重启后按 `output_ref` / `session_id` 继续读取。
- 增加 GPT Actions PDF 附件接口，并限制文件大小、页数和读取成本。
- 增加统一请求错误日志、工作区级 MCP 上游代理和工具公开范围控制。
- 增加本地安装与升级前检查，恢复升级后的工作区服务。
- 增强 macOS 文稿目录授权、子进程清理、Cloudflare/FRP 隧道恢复与 watchdog。
- 增加工作区外批准目录只读访问、启动服务恢复和有界研究看板。
- 历史会话变为可选，并为初始化与调用设置明确等待上限。

## 0.2.x — 2026-08 to 2026-09

- 将历史会话改为有界当前状态、可搜索索引和按需原文精读，减少新对话的上下文负担。
- 增加关闭窗口/托盘确认，并修复隔夜断网后的 FRP 健康恢复。
- 增强跨平台命令路径、超时和启动错误报告。
- 限制无界目录扫描及超大文件读取，降低内存和 I/O 风险。
- 增加调用审计、HTTP 请求日志和工作区配置的本地 MCP 上游。

## 0.1.x — 2026-07 to 2026-08

- 将核心能力重写为 Rust，并使用 Tauri 2 + Svelte 构建桌面客户端。
- 增加多工作区管理、MCP/Actions 服务、OAuth/Bearer 鉴权及 FRP/Cloudflare 隧道。
- 增加跨会话历史归档、检查点、验证和恢复提示词。
- 增加 `search_text`、文件/Patch/Git/命令工具，以及工作区优先的读写权限模型。
- 修复工作区创建、FRPC 停止、端口保存、窗口恢复与界面资源释放问题。

## 与原项目的主要区别

| 领域 | 原项目 | 本仓库 |
| --- | --- | --- |
| 主要实现 | Python MCP 运行时 | Rust MCP 内核 + Tauri/Svelte 桌面端 |
| 使用方式 | CLI、容器和远程运行时 | 图形化多工作区、本地服务与隧道管理 |
| 会话延续 | 以上游当前实现为准 | 项目内 Markdown 档案、有界状态、搜索与精读 |
| 扩展能力 | Python 工具与协议演进 | 浏览器桥接、GPT Actions、PDF、本机 MCP 上游 |
| 安全模型 | 以上游当前实现为准 | 工作区写隔离、外部只读目录、审计与桌面密钥存储 |

## 许可与归属

- 原始项目：<https://github.com/xyTom/coding-tools-mcp>
- 本派生仓库：<https://github.com/oraeorae/coding-tools-mcp-desktop>
- 许可证：Apache-2.0

本仓库不会暗示原作者为后续独立修改提供保证或背书。
