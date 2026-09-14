# 0.3.1 本机增量升级验收

## 变更
桌面浏览器连接/停止/健康检查，随包桥接依赖；命令按工作区私有归档并兼容原引用，状态重启时标为未知，不自动重跑；修复输出环形缓冲分页；包含已存在浏览器/PDF/Python增量。

## 构建前验证
- Rust lib + integration（包含 ignored 浏览器集成）：205 项通过，0 失败。
- Node 浏览器：6 项通过，包含真实隔离 Chrome 与 health 不读网页验证。
- 其他 Node：4 项通过。
- Svelte check：0 errors / 0 warnings；前端生产构建通过；cargo check 通过。
- 既存 bytes_to_mb 未使用警告保留。
- GitNexus 所有差异合并影响 46 个符号、26 条执行流，包含早前未提交增量。会话初始化 HIGH，完整差异 critical；已审阅实际源码及全量回归。图谱不完整覆盖未跟踪新文件，不能作为唯一验收证据。
- ARC-8 validate：passed=true。code_review 是指南与证据校验，本轮人工源码自审，不宣称独立代理审计。

## 设计边界与自审
远程 MCP 不提供桥接启动/停止；新增 Tauri 命令只控制自身子进程，外部桥接保持外部管理。GET /health 经原 loopback Host/Origin/bearer 校验，不调用 adapter。
归档目录 0700、文件 0600（Unix），UUID 文件名、大小限制、原子写入、工作区隔离；每流最后 1 MiB，保留100条已结束记录。已失联但仍存在的其他应用进程所持活动记录不清理。运行中变化按500ms间隔快照，无变化不重复写盘；输出失败通过 output_persistence_ok=false 可见。不是完整日志或跨进程接管。

## 安装与在线验收
最终安装结果见下方。旧服务：量化研究 MCP 28766 / Actions 8788；WorkBuddy 两服务停止。原应用无运行命令子进程。
PDF 原有 Actions 未配置公网地址/隧道；目标自定义 GPT 尚未提供。因此本机升级包含 PDF 接口，但不将本地成功当成 ChatGPT 附件显示成功。
替换前公开 MCP 从当前网络访问为 HTTP 403；本机服务在线。保留这项基线以区别既有网络问题与升级回归。

## 0.3.0 现场问题与 0.3.1 修复
安装后 server_info 实际返回0.3.0和30工具，MCP/Actions均恢复，配置JSON与原备份完全相同。PDF在Documents测试路径出现File::open系统调用阻塞（sample证据），/private/tmp同字节合成PDF下载成功（620字节，application/pdf）。不推测或修改系统隐私设置。0.3.1新增5秒准备超时，并使单并发许可随后台读取线程保留，避免超时后重复启动读取。测试覆盖许可仍被占用与后台结束后恢复。
浏览器按钮真实启动了随包桥接并展示扩展授权页；原失效配置清理成功，停止按钮回到未连接。扩展提示访问整个浏览器，本轮未确认该授权，测试页面与授权窗口已关闭。真实Chrome读页/截图链路由隔离浏览器集成测试验证。
公网MCP当前请求返回Cloudflare HTTP403/error1010，与替换前一致；本地MCP GET200。

## 0.3.1 最终安装结果
- 已替换 `/Applications/Coding Tools MCP.app`，本地 ad-hoc 签名通过 codesign deep/strict 检查，arm64；源提交 `4ce128e`。
- 应用 UI 与已安装服务 server_info 均确认 0.3.1 / 核心30工具。量化研究 MCP28766、Actions8788恢复运行；WorkBuddy保持停止。
- 替换前后 profiles.json SHA256一致，未更改密钥与服务配置。
- 最终安装包实测620字节合成PDF经return_pdf及下载接口原样返回，Content-Type application/pdf。测试不涉及私有PDF。
- 本地MCP HTTP200；公网MCP仍HTTP403（既有问题）。浏览器保持未连接，待用户扩展授权；GPT附件端到端验收仍缺目标GPT及Actions公网地址。
- 回滚备份：`/Users/robot/Documents/ChatGPT/tools-coding/app-backup-20260914-234011-0.3.0`，保留原0.2.1应用/配置与过渡0.3.0应用。
- 最终清单及实测：`../diagnostics/installed-0.3.1-manifest.json`、`../diagnostics/installed-0.3.1-local-verification.json`。
