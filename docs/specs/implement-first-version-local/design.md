# 设计文档：本地浏览器只读工具

## 概述
对应需求 FR-1、FR-2、FR-3、FR-4、NFR-1 至 NFR-3。

## 技术方案
Rust 工具 -> 带 token 的 loopback HTTP -> Node bridge -> 内存 MCP transport -> Playwright createConnection(extension=true) -> 用户授权页。
三个工具在所有正常工具 profile 注册为只读、open world；本机未启用时返回 BROWSER_NOT_CONNECTED。browser_read_page 与 screenshot 默认使用扩展选择的 page，禁止 pages()[0] 回退。

## 数据模型
~/.coding-tools-mcp/browser/<sha256(normalized canonical workspace)>.json 保存 version、port、token、workspace。目录 0700、文件 0600；桥接启动独占配置，退出时仅移除自己持有 token 的配置。
标签 ID 只在本次 Playwright 会话稳定，附着在 Node Page 对象，不在网页 DOM 写入。断线后旧 ID 失效。

## API 设计
browser_list_tabs({cursor?:integer,limit?:integer}) -> tabs、selected_tab_id、next_cursor。
browser_read_page({tab_id?:string,cursor?:integer,max_chars?:integer}) -> tab_id、title、url、text、total_chars、next_cursor。
browser_screenshot({tab_id?:string}) -> tab_id、title、url、mime_type、base64、bytes。
未知字段、负数、非整数、超范围参数在两端拒绝。网页正文标记为不可信内容。

## 文件结构
bridge/browser/package.json、package-lock.json、adapter.mjs、core.mjs、server.mjs、test/*.test.mjs。
src-tauri/src/tools/browser.rs 新模块；tools/mod.rs、dispatch.rs、registry.rs、workspace.rs 接入。
src-tauri/tests/browser_tools.rs 集成测试；docs/browser-bridge.md 操作指南。

## 设计决策
FR-4：选择独立本机桥接而不是将通用 Playwright MCP 暴露到公网；只有固定的只读函数可以被调用。首次连接由本地操作者完成，不因远程请求自动授权。生产模式仅 extension；真实测试通过依赖注入使用专门的临时 Chromium 上下文。
FR-2：返回分页可见文本，不采集 cookie、localStorage、密码字段或隐式 iframe。页面数据仅作为内容，不作为指令。
FR-3：截图视口 JPEG，不滚动页面或写文件。

## 测试策略
Node 测参数、授权 token、Host/Origin、队列并发、超时、页关闭、选中页、分页与协议；真实临时浏览器测试正文/截图和稳定 tab ID。Rust 测 schemas/profile、断线、配置和图片 MCP 封装，并运行相关回归。测试仅用合成页面。

## 风险评估
公共工具分发是核心路径，需回归既有工具。Playwright 固定版本且格式适配有测试。用户浏览器扩展安装/选择若需手动操作，明确区分自动验证与待用户连接。

## 检查清单
- [x] 设计覆盖所有 FR，接口有界，安装可回滚。
