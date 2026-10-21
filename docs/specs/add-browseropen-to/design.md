# 设计文档：browser_open
## 概述
覆盖 FR-1、FR-2；沿用 Rust dispatch -> loopback bridge -> 固定 Playwright 函数。
## 技术方案
两端验证 url。readBrowser 固定函数新增导航分支，context.newPage 后 goto(waitUntil=domcontentloaded, timeout=10000)。失败关闭新页；成功后 adapter 更新 selectedId。其它工具语义不变。
## API 设计
browser_open({url:string}) 返回 ok、tab_id、title、url、selected、content_trust。URL 最多 2048 UTF-8 字节，协议 HTTP/HTTPS，禁止凭据/空白/控制符/反斜线及未知字段；Rust 使用 reqwest::Url，Node 使用 URL 解析。
## 数据模型
仅复用现有 Page ID 与 selectedId，无持久化结构变更。新标签保留用于后续读取，HTTP 错误页仍可读取分析。导航失败不会改变逻辑默认页。
## 设计决策
FR-1：新建页以保留原网页；不覆盖已有标签。FR-2：导航具有状态变化，注册为 readOnlyHint=false、destructiveHint=false、openWorldHint=true，禁止 read-only profile。loopback 目标地址仍不可由请求控制。
## 文件结构
修改 bridge/browser/core.mjs、adapter.mjs、server.mjs、test/browser.test.mjs；src-tauri/src/tools/browser.rs、dispatch.rs、registry.rs；src-tauri/tests/browser_tools.rs、call_tool_contract.rs；docs/browser-bridge.md 与 README.md。新增本规格三件套与 docs/reports/browser-open-validation.md。
## 测试策略
真实隔离 Chrome 合成页面验证导航、默认读取、截图、列表、重定向、导航失败后标签清理和旧默认页保留。Rust e2e 验证穿透分发与桥接导航。参数两端及 profile 负例，完整 Rust 回归。
## 风险评估
公共 dispatch 风险中等，跑全量回归；真实用户扩展仍未安装，本次使用隔离浏览器验证并明确区别。导航可能触发网站副作用，不把导航标记为只读。
## 检查清单
- [x] 覆盖 FR-1、FR-2，维持架构。
