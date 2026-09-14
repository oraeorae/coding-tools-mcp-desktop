# 设计文档：coding-tools
## 概述
覆盖 FR-1、FR-2、FR-3、FR-4 及 NFR-1/2/3。
## 技术方案
浏览器桌面 commands 调用独立 browser_runtime 模块；bundle resources 携带 bridge/browser，使用系统 Node，stdio EOF 控制所属桥接生命周期。新增经过现有 bearer 鉴权的 GET /health，仅报告桥接可用性，不读取标签。桌面面板显示本机依赖、桥接状态、工具版本和原有健康检查入口。
会话存储新增 session_archive 模块：按规范工作区及 harness 根隔离；运行中进程内共享 SessionStore；500ms 有界快照、最终态保存，私有文件原子替换；旧 owner 未完成记录读取时标为 server_restart，永不伪装为成功。read_output 使用绝对字节偏移并报告已丢弃前缀，防止循环翻页。read_output/write_stdin 空轮询支持归档读取。
## 数据模型
桥接状态：state/message/managed/nodeAvailable，不含 token/页面正文。
会话记录：owner、updatedAt、snapshot、stdout/stderr bytes、total bytes；最多 100 条已结束记录，活动记录不被清理。
## API 设计
Tauri：browser_status(id)、start_browser_bridge(id)、stop_browser_bridge(id)。MCP 参数兼容，归档仅通过原会话引用读取。
## 文件结构
新增 src-tauri/src/browser_runtime.rs、src-tauri/src/commands/browser.rs、src-tauri/src/tools/session_archive.rs、src/lib/api/browser.ts、src/lib/components/BrowserPanel.svelte、对应测试。
修改 src-tauri/src/lib.rs、commands/mod.rs、tools/session.rs、tools/context.rs、tools/exec.rs、tools/mod.rs、bridge/browser/server.mjs、workspace 页面及 5 处版本源、README/浏览器文档。
## 设计决策
FR-1 使用随包脚本和现有系统 Node，减少用户终端步骤；缺 Node 时清楚提示，不后台下载安装。
FR-3 仅恢复可查证状态和有界输出；不承诺跨应用重启接管子进程。
FR-4 本机备份回滚，用户本轮已授权替换；不触发远程发布。
## 测试策略
桥接 health 鉴权、无页面读取、重复启动与停止；归档跨 store 恢复、重启未知状态、非零退出、输出分页与非法 ID；全量 Rust、Svelte check、Node tests，构建签名与实际启动检查。
## 风险评估
公共 session 改动影响命令核心，必须全量回归；浏览器扩展连接需要本机授权；公网/GPT 配置缺失单独记录；配置/令牌不进入报告。

## 现场验收修正（FR-4 / NFR-4）
Documents合成文件在macOS File::open阻塞，临时目录同字节PDF通过。增加PDF准备5秒整体超时，单工作区Semaphore许可随blocking worker移动，直到worker真正退出才释放；超时不是读取被取消，后续请求仍受并发限制。原缓存保持32MB/64项上限。版本递增0.3.1。
