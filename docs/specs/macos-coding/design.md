# 设计文档：macOS MCP 隧道看门狗

## 概述

本设计覆盖 FR-1、FR-2、FR-3 及全部非功能需求。看门狗作为独立 LaunchAgent 定时运行，不进入桌面端进程，也不改变现有 MCP、OAuth proxy 或 tunnel 配置。

## 技术方案

| 类别 | 选择 | 理由 | 关联需求 |
|------|------|------|----------|
| 调度 | 用户级 launchd `StartInterval=60` | 登录后自动运行、无需常驻新进程 | FR-2, FR-3 |
| 实现 | Bash 3.2 脚本 | 本机原生、易审计、易回滚 | FR-1, FR-3 |
| JSON 校验 | Python 3 单行解析 | 精确校验 discovery 与 OAuth 元数据 | FR-1 |
| 状态 | Application Support 下的纯文本状态文件 | 不含密钥，便于原子更新和排查 | FR-2, FR-3 |

### 架构设计

```text
launchd 每 60 秒
       |
       v
非阻塞目录锁
       |
       v
本地 discovery ----失败----> 记录 local_unhealthy，不重启
       |
      成功
       v
公网 discovery + OAuth 元数据
       |
 连续失败两次
       v
冷却与每小时上限闸门
       |
       v
kickstart -k cloudflared LaunchAgent
       |
       v
有限次数复检并记录 recovered / still_unhealthy
```

## 数据模型

| 文件 | 内容 | 约束 | 说明 |
|------|------|------|------|
| `failure-count` | 非负整数 | 非数字按 0 处理 | 连续公网失败轮数 |
| `last-restart` | Unix 秒 | 非数字按 0 处理 | 冷却判断 |
| `restart-history` | 每行一个 Unix 秒 | 每轮裁剪为最近一小时 | 重启频率限制 |
| `watchdog.log` | 无敏感信息文本 | 超 1 MiB 裁剪到 2000 行 | 运行与自愈审计 |

## 接口设计

脚本支持以下命令：

- `check`：执行一次健康检测与必要自愈，供 launchd 调用。
- `status`：输出当前计数、冷却状态和最近日志，不执行重启。

安装器支持：

- `install`：安装运行副本和 LaunchAgent，加载后立即运行一次。
- `uninstall`：卸载看门狗自身，保留现有 cloudflared 与 MCP 服务。

测试通过环境变量替换 URL、状态目录、日志路径和 `launchctl`，生产 LaunchAgent 不设置这些覆盖项。

## 文件结构

```text
scripts/macos/
├── coding-tools-mcp-tunnel-watchdog.sh
├── install-coding-tools-mcp-tunnel-watchdog.sh
└── test-coding-tools-mcp-tunnel-watchdog.sh

~/Library/Application Support/coding-tools-mcp-desktop/bin/
└── coding-tools-mcp-tunnel-watchdog.sh

~/Library/LaunchAgents/
└── com.robot.coding-tools-mcp-tunnel-watchdog.plist
```

## 设计决策

### 仅重启 tunnel，不重启 MCP（FR-1、FR-2）

本次事故中本地 MCP 和 OAuth 均正常，失效的是 Cloudflare connector。重启桌面端会扩大影响，还可能中断本地任务，因此自愈动作严格限制为 sibling LaunchAgent 的 `kickstart -k`。

### 不在周期检查中执行真实 OAuth 工具调用（FR-3）

真实 OAuth 调用最能证明端到端可用，但需要凭据并产生 Token。周期任务改用 discovery 与 OAuth 元数据验证；恢复部署验收时再由人工控制的诊断流程执行一次授权工具调用。

### 双阈值和频率上限（FR-2）

连续两次失败过滤短暂抖动；5 分钟冷却和每小时 3 次上限阻止代理长期异常时形成重启风暴。

## 测试策略

- 语法检查：`bash -n`、`plutil -lint`。
- 正常路径：本地与公网模拟成功，不触发重启并清零失败计数。
- 首次失败：计数加一，不重启。
- 第二次失败：触发一次模拟 `launchctl kickstart -k`。
- 本地失败：无论公网结果如何都不重启。
- 冷却和小时上限：达到闸门后不重启。
- 并发锁：已有锁时立即退出。
- 安装验证：LaunchAgent 已加载，正常生产检测不重启 tunnel。
- 端到端验证：公网 GET 200、未授权 POST 401、授权 `get_default_cwd` 成功。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 公网短暂抖动 | 误重启 | 连续失败阈值和冷却 |
| 代理长期不可用 | 重启循环 | 每小时 3 次硬上限 |
| 本地 MCP 停止 | 无意义重启 tunnel | 本地健康前置闸门 |
| 两个目标部分故障 | 共用 tunnel 被重启 | 仅任一本地健康且对应公网失败时处理；动作有冷却 |
| 脚本并发 | 状态竞争 | 原子目录锁 |
