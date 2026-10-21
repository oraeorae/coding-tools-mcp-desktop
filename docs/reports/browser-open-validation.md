# browser_open 验证记录

日期：2026-09-12。基于前一轮未提交的本机浏览器桥接增量实现，未安装替换桌面应用或连接个人 Chrome。

## 实现
- browser_open({url}) 新建标签，等待 DOMContentLoaded，返回最终 URL、标题、tab_id；成功后更新默认读页/截图/列表选中状态。
- 导航失败关闭新页并保留原默认页；导航不覆盖旧网页。网页数据不作为指令。
- Node 和 Rust 验证 HTTP/HTTPS、URL 长度、凭据和未知参数；loopback、token 和 MCP 鉴权链不变。
- read-only 不曝光 browser_open。普通与高级配置标为非只读，兼容配置保留导航真实标注。Actions 加入 MUTATING_TOOLS，遵循已有写锁和 consequential 标记。

## 自动验证
- `BROWSER_TEST_EXECUTABLE='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' npm run test:browser`：5/5 通过。
- `cargo test --manifest-path src-tauri/Cargo.toml --lib --tests -- --include-ignored`，同上浏览器环境变量：189 通过，0 失败。包含 Rust dispatcher -> loopback -> Playwright -> Chrome 的 browser_open/read/screenshot 端到端测试。
- `node --test tests/*.test.mjs`：4/4 通过。
- `git diff --check`：通过。
- 合成浏览器测试覆盖重定向、成功后的默认页、原页保留、读取旧页不改默认、截图、失败新页清理、默认页关闭后重新打开、参数拒绝。

## 审查
GitNexus：公共 dispatch 中等影响，工具目录函数高影响（9 符号、4 直接调用方），改动限定于导航新增分支/条目及导航标注。mcp-probe-kit code_review 提供审查指南，实际由本轮自审源码、测试和 diff，并非独立审计。
补齐 Actions 状态变更清单和兼容配置真实标注后重新跑全量 Rust 回归。仅有原有 bytes_to_mb 未使用警告。

## 限制
本次使用隔离 headless Chrome 与合成内容，不代表用户扩展已启用。用户此前的 Playwright 扩展安装缺项未在本轮改变；实际启用需安装扩展、运行新桌面构建、启动桥接并本地选择授权页。动态页面 DOMContentLoaded 后可能仍在加载；模型应再读取需要的内容。未进行个人网页读取、发布或 Git 提交。
