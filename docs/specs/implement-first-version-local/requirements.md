# 需求文档：本地浏览器只读工具

## 功能概述
让已通过现有 MCP 鉴权的客户端读取用户本机已连接、已授权的 Chrome 标签页。提供且仅提供三个浏览器工具。

## 历史经验与坑
当前 tools::call_tool 是 MCP 和 Actions 共同入口。不得分叉鉴权和策略。图谱记录提示 Rust 符号索引不可靠，需源码交叉核验。

## 术语定义
桥接：本机 Node 进程，通过 Playwright 官方扩展连接浏览器。
选中页：用户在扩展连接界面选择、由 Playwright 当前 page 表示的页，不取任意第一项。

## 范围边界
In Scope：FR-1 至 FR-4，三个只读工具，安装启动文档和测试。
Out of Scope：远程导航、点击、表单、任意脚本执行；替换已安装应用或重启用户现有 MCP 服务。

## 需求列表
### FR-1: 列出授权标签页
**优先级:** Must
**用户故事:** 用户希望 AI 能发现已连接浏览器的可读页面。
#### 验收标准（EARS）
WHEN 调用 browser_list_tabs THEN 系统 SHALL 返回稳定会话内 ID、标题、URL、选中标记和有界分页结果。
IF 尚未连接 THEN 系统 SHALL 返回可操作的错误，不自动创建空白浏览器。

### FR-2: 读取指定或默认页
**优先级:** Must
**用户故事:** 用户希望阅读已选文章，避免误读其他标签。
#### 验收标准（EARS）
WHEN 未提供 tab_id THEN 系统 SHALL 读取用户选中页。
WHEN 提供有效 tab_id THEN 系统 SHALL 读取该授权页面的可见正文、标题、URL，并提供字符分页。
IF 标签已关闭或协议不是 HTTP/HTTPS THEN 系统 SHALL 拒绝读取；不回退其他页面。

### FR-3: 截图
**优先级:** Must
**用户故事:** 用户希望 AI 能看到页面图表。
#### 验收标准（EARS）
WHEN 调用 browser_screenshot THEN 系统 SHALL 返回目标页当前视口 JPEG 和 MCP image content，不生成工作区文件。
IF 图片超过上限 THEN 系统 SHALL 返回 OUTPUT_TOO_LARGE。

### FR-4: 本机工作区绑定
**优先级:** Must
**用户故事:** 用户希望浏览器访问继续服从现有 MCP 权限并由本机显式启用。
#### 验收标准（EARS）
WHEN 本地启动桥接并指定 workspace THEN 系统 SHALL 仅绑定 127.0.0.1 随机端口，以随机 bearer token 保护，配置保存在用户目录且按规范工作区路径分离。
IF 远程请求提供 endpoint、代码、路径或未知参数 THEN 系统 SHALL 拒绝。
WHEN 浏览器工具通过 MCP/Actions 调用 THEN 系统 SHALL 继续走原有鉴权、曝光及共同分发入口。

## 非功能需求
NFR-1：正文单次最多 32000 字符，最多 200 个标签，每页最多 100 项，截图最多 2 MiB；请求 25 秒内超时。
NFR-2：禁用代理和重定向；HTTP 桥接拒绝 Origin 和非本机 Host；请求限长；配置不进 Git；不记录正文和 token。
NFR-3：Node >=20；桥接独立安装以保持原有桌面构建；Rust 接口跨平台，Windows 路径规范化一致。

## 依赖关系
锁定 @playwright/mcp 与官方公开 createConnection 接口，SDK 内存传输连接，现有 reqwest、sha2、dirs。

## 检查清单
- [x] 范围、需求 ID、可测试验收、依赖已明确。
