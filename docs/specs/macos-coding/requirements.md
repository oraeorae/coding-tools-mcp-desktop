# 需求文档：macOS MCP 隧道看门狗

## 功能概述

为本机 Coding Tools MCP 共用的 Cloudflare named tunnel 增加独立、可回滚的 launchd 健康看门狗，解决 `cloudflared` 进程仍在但 Cloudflare 控制面已失联时无法自动恢复的问题。

## 历史经验与坑

- **可复用经验**：本地监听、公网路由、OAuth 鉴权和真实工具调用必须分层验证；当前网络固定使用 HTTP/2。
- **必须规避的坑**：不能把进程存在或公网 GET 200 单独视为完整可用；不能因公网失败直接重启桌面端；不能在日志中保存 OAuth 密码、授权码或 Token。

## 术语定义

- **本地健康**：MCP discovery 端点返回 HTTP 200，且 JSON 中 `name` 为 `coding-tools-mcp`。
- **公网健康**：公网 discovery 和 OAuth protected-resource 元数据均返回可识别的成功响应。
- **自愈**：在本地健康且公网连续失败后，仅重启 `com.robot.cloudflared.coding-tools-mcp`。

## 范围边界

**In Scope**

- 每 60 秒检测量化研究与消息面分析两个 MCP 公网入口。
- 连续失败两次后重启共用 Cloudflare tunnel。
- 5 分钟冷却、每小时最多 3 次重启、并发锁和有界无敏感信息日志。
- 安装为当前用户 LaunchAgent，并提供可重复安装和卸载入口。

**Out of Scope**

- 不修改 Surge、Clash 或系统网络配置。
- 不重启 Coding Tools MCP 桌面端或本地 MCP/Actions 服务。
- 不自动执行 OAuth 登录或保存任何凭据。
- 不把本功能集成进桌面 UI。

## 需求列表

### FR-1：分层健康检测

**优先级：Must**

作为本机 MCP 使用者，我希望看门狗区分本地服务故障和公网隧道故障，以避免错误重启。

1. WHEN 本地 discovery 返回 HTTP 200 且服务名匹配 THEN 系统 SHALL 继续检测对应公网 discovery 与 OAuth 元数据。
2. IF 本地 discovery 不健康 THEN 系统 SHALL 记录 `local_unhealthy` 并禁止重启 tunnel。
3. IF 任一本地健康目标的公网 discovery 或 OAuth 元数据不健康 THEN 系统 SHALL 将本轮记为公网失败。

### FR-2：有界自动恢复

**优先级：Must**

作为本机 MCP 使用者，我希望公网连续失败时自动恢复 tunnel，同时避免重启风暴。

1. WHEN 公网连续两轮失败且至少一个对应本地服务健康 THEN 系统 SHALL 只重启 `com.robot.cloudflared.coding-tools-mcp`。
2. WHILE 距上次重启不足 300 秒 THEN 系统 SHALL 禁止再次重启。
3. WHILE 最近一小时已重启 3 次 THEN 系统 SHALL 禁止再次重启并记录限流原因。
4. WHEN 重启发生 THEN 系统 SHALL 在有限等待后重新验证公网状态并记录结果。

### FR-3：安全、可回滚运行

**优先级：Must**

作为维护者，我希望守护任务可审计、可卸载且不泄露鉴权信息。

1. WHEN 多个调度实例重叠 THEN 系统 SHALL 通过非阻塞锁只允许一个实例运行。
2. WHEN 写运行日志 THEN 系统 SHALL 仅记录时间、目标、状态和恢复动作，不记录请求正文、密码、授权码或 Token。
3. WHEN 日志超过 1 MiB THEN 系统 SHALL 将其裁剪到最近 2000 行。
4. WHEN 执行卸载 THEN 系统 SHALL 停止并移除看门狗 LaunchAgent，但不影响 cloudflared 与 MCP 服务。

## 非功能需求

- **NFR-1（性能）**：单轮正常检测应在 15 秒内完成，不常驻进程。
- **NFR-2（安全）**：检测无需读取应用 secrets；日志不包含敏感信息。
- **NFR-3（兼容性）**：兼容 macOS 自带 Bash 3.2、launchd、curl 与 Python 3。
- **NFR-4（可维护性）**：源脚本、安装器与测试保存在仓库 `scripts/macos/`，运行副本安装在用户 Application Support。

## 依赖关系

- 依赖现有 LaunchAgent `com.robot.cloudflared.coding-tools-mcp`。
- 依赖本地端点 `127.0.0.1:28766`、`127.0.0.1:28769` 和两个公网域名。
- 依赖 `/usr/bin/curl`、`/usr/bin/python3`、`/bin/launchctl`。
