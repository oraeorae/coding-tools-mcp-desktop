# 本地浏览器只读桥接交付记录

日期：2026-09-12。基线 d0b5313ec39ea108f35dcc0773a2c6ba6d147e28；所有改动留在工作区，未提交、未替换已安装应用或重启原服务。

## 结果
三个工具已实现：browser_list_tabs、browser_read_page、browser_screenshot。默认标签页固定为首次本地扩展选择结果；显式读取其他 ID 不改变默认值。配置按规范工作区路径隔离，HTTP 仅 127.0.0.1 随机端口且 bearer 验证。Playwright 通用工具只在进程内使用，对外无脚本执行入口。原 MCP/Actions 鉴权代码未更改。

## 验证证据
- 规格 check_spec：4 项 FR，0 错误、0 警告。
- GitNexus：call_tool 上游影响 9 个符号（MEDIUM），input_schema LOW，wrap_mcp_tool_result LOW；新增 adapter/server 后续修改影响 LOW。
- `cargo test --manifest-path src-tauri/Cargo.toml --lib --tests`：184 通过，0 失败（初次执行时新的 browser_tools 集成文件尚未纳入，因此独立运行下一项）。
- `cargo test --manifest-path src-tauri/Cargo.toml --test browser_tools -- --include-ignored`：4 通过，含 Rust 分发 → token HTTP 桥接 → Playwright → 真实 Chrome → MCP 图片封装；验证跨工作区拒绝。
- `npm run test:browser`：4 通过，覆盖参数、Host/Origin/token、并发/超时、真实 Chrome 选中页、Unicode 分页、隐藏/密码字段排除、截图、显式其他页、默认页关闭不回退。
- `node --test tests/*.test.mjs`：已有前端 4 项通过。
- `git diff --check`：通过。
- 真实浏览器测试使用本机 Chrome 可执行文件的独立无头上下文及合成 localhost 页面，没有读取个人浏览器页面。
- 依赖安装审计：97 packages installed，0 vulnerabilities（本次 npm 安装输出）。版本锁定 @playwright/mcp 0.0.80、@modelcontextprotocol/sdk 1.30.0，锁文件随交付。
- Rust 仅有原有 ui_memory.rs 中 bytes_to_mb 未使用警告。

## 代码审查
按 code_review 返回的 Git/Plan 证据指南对已跟踪 diff 及全部新增模块进行了本次自审。工具本身是指导式收集器，不是独立语义审查员。
已确认：未新增公网浏览器端点；调用参数不能注入代码、URL、路径、token；错误返回不包含原始 Playwright 页面日志；已修复错误提示过泛、重复关闭与清理失败可能遗留配置问题。未发现未解决的高优先级代码问题。

## 本机启用状态与边界
源码和隔离浏览器测试完成，个人 Chrome 扩展连接尚未完成。实际启动预检证实当前 Chrome 配置未安装 Playwright Extension，退出码 1 并显示官方安装指引。尝试通过 Codex 浏览器工具打开 Chrome Web Store 时返回 `Not allowed`，没有绕过限制，也未安装扩展。
用户安装扩展并运行本次桌面构建后，执行 `npm run browser:start -- --workspace <实际工作区>`，在扩展中选择页面即可启用。旧版已安装应用不会自动更新。
Windows 代码路径作了规范化处理，但本次没有 Windows 实机验收。Playwright 扩展的完整实际连接仍需首次本地安装和页面选择；无头浏览器测试不代表该步骤通过。

## 文件清单
新增：bridge/browser/{adapter.mjs,core.mjs,server.mjs,package.json,package-lock.json,test/browser.test.mjs,test/fixture.mjs}；src-tauri/src/tools/browser.rs；src-tauri/tests/browser_tools.rs；docs/browser-bridge.md；docs/specs/implement-first-version-local/{requirements.md,design.md,tasks.md}；本报告。
修改：README.md、package.json、src-tauri/src/tools/{mod.rs,dispatch.rs,registry.rs,workspace.rs}、src-tauri/tests/call_tool_contract.rs。
仓库规定的 .mcp-probe-kit 本地启动器及检查点另存于本机，不属于浏览器产品模块。
