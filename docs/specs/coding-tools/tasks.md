# 任务清单：coding-tools
## 交付物清单（Scope-lock）
新增 8 个实现/测试文件：browser_runtime.rs、commands/browser.rs、session_archive.rs、session_io.rs、api/browser.ts、BrowserPanel.svelte、session_recovery.rs、health.test.mjs；既有未跟踪浏览器/PDF源码随版本纳入。修改文件见发布提交清单；规格 3 个，最终验收报告 1 个。新增模块小于 500 行；已超过 500 行的 exec/lib/workspace 页面仅加接线，逻辑放独立模块。
## 任务列表
- [x] 1. 实现私有会话归档与原工具恢复读取。
  证据：src-tauri/src/tools/session.rs:18 `sessions: Mutex<HashMap<String, Arc<ExecSession>>>`；exec.rs 完成分支 `ctx.sessions.remove`；context.rs 每次 `SessionStore::new()`。
  文件：session_archive.rs <350 行；session.rs 小范围接线；exec.rs monitor 修改；context.rs 共享存储。需求 FR-3；设计：技术方案/数据模型。
- [x] 2. 实现桌面浏览器连接、状态、停止与健康路由。
  证据：bridge/browser/server.mjs `startBridge` 创建配置使用 wx；main 从终端启动；HealthPanel.svelte 仅显示 MCP/Actions。
  文件：browser_runtime.rs <400 行；commands/browser.rs <100 行；BrowserPanel.svelte <180 行；api/browser.ts <50 行；lib.rs/commands/mod.rs 接线；workspace 页面加组件；server.mjs <150 行。需求 FR-1/FR-2；设计：技术方案/API。
- [x] 3. 完成 PDF 合成验收、回归、版本递增、备份与应用替换。
  证据：docs/reports/actions-pdf/validation.md 记录本地测试通过但未替换及目标GPT未验证；tauri.conf.json 为0.2.1。
  文件：5处版本源、测试、README、browser-bridge.md、验收报告。需求 FR-4；设计：测试策略/风险评估。
## 验收
全量检查通过；明确输出保留边界；实际安装版本/签名/架构与构建匹配；保存回滚路径及服务检查结果。

## 需求覆盖矩阵
|需求|任务|
|---|---|
|FR-1|2|
|FR-2|2|
|FR-3|1|
|FR-4|3|
## 文件变更清单
新增归档、桥接管理、Tauri commands、API、组件及测试；修改 session/context/exec、lib/commands/mod 接线、bridge server、工作区页面及版本源，具体路径见 design.md。

## 验证证据
Rust 全量及浏览器集成回归、Svelte 检查和生产构建已执行，日志位于父目录 diagnostics/optimization-*。安装阶段在最终报告逐项记录。

- [x] 4. 为PDF准备增加5秒超时及后台读取并发门，测试阻塞后不重复启动读线程，再构建替换0.3.1。证据：diagnostics/installed-app-sample.txt显示File::open阻塞；临时目录620字节真实下载成功。文件：actions/pdf.rs和pdf_tests.rs；需求FR-4/NFR-4；设计：现场验收修正。
