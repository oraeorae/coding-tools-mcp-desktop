# 统一请求错误日志验收（0.3.6）

## 结果

Coding Tools MCP 0.3.6 已从当前本地源码构建并安装到 `/Applications/Coding Tools MCP.app`。安装器保持配置不变并恢复了升级前运行的 MCP 与 Actions 服务。

## 自动化验证

- `cargo test --manifest-path src-tauri/Cargo.toml --lib --tests`：212 项通过，0 项失败，1 项真实浏览器环境测试按设计忽略。
- `npm run check`：0 个错误，0 个警告。
- `npm run build`：生产构建通过。
- `npm run test:local-install`：5 项通过。
- `check_spec`：FR-1 至 FR-4 全部通过，无警告。
- ARC-8 architecture validate：通过，无证据缺口。
- GitNexus detect-changes：13 个文件、12 个符号、12 条执行流，风险为 high；风险来源是两个 HTTP 主入口和 Tauri command 注册。全量回归与安装后请求验证已覆盖这些入口。

## 安装后请求验证

安装版本通过 `CFBundleShortVersionString` 核对为 0.3.6，应用签名通过 `codesign --verify --deep --strict`。

使用不含凭据和用户内容的合成请求验证：

- MCP `POST /mcp` 无鉴权返回 HTTP 401，`request-errors-mcp.jsonl` 从 0 条增加到 1 条。
- Actions 未鉴权请求返回 HTTP 401，`request-errors-actions.jsonl` 从 0 条增加到 1 条。
- 两条记录只包含服务、来源、方法、路径、状态码、错误分类和时间，不包含请求头、鉴权值、请求正文或响应正文。
- MCP 日志页可见 `HTTP 401 POST /mcp`；Actions 日志页可见对应 401。
- 两个页面均显示刷新、复制、清空操作，并每 3 秒自动刷新。

验收测试生成的两条合成错误在验证后已清理，没有保留在日常日志中。

## 运行状态

- MCP `http://127.0.0.1:28766/mcp`：HTTP 200。
- Actions `http://127.0.0.1:8788/openapi.json`：HTTP 200。
- 安装备份：`/Users/robot/Library/Application Support/coding-tools-mcp-desktop/data/update-backups/2026-09-15T01-24-52-648Z-to-0.3.6`。

## 边界

Cloudflare 在请求抵达本机以前拦截的事件无法由本地 listener 被动记录。应用执行健康检查后，检测到的公网请求错误会写入对应工作区的请求错误日志。
