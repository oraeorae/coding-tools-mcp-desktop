# GPT Actions PDF 验证记录

验证日期：2026-09-13 Asia/Shanghai。

## 结果

源代码实现完成并通过本地 HTTP 验证，开发二进制构建成功。尚未替换 `/Applications` 中应用，尚未验证公网下载和目标自定义 GPT 的附件显示。

## 变更

新增 `POST /actions/return_pdf`，沿用现有 Actions 鉴权和只读路径解析，响应为顶层 `openaiFileResponse` 数组。原 PDF 字节保存在工作区实例的内存缓存，随机 UUID 下载能力 5 分钟后失效；只接受普通 PDF 文件、10,000,000 字节上限。缓存上限 32 MB / 64 项，后台每 30 秒清理，下载和插入时也检查过期。下载 GET 返回 PDF MIME、包含中文 UTF-8 文件名的 Content-Disposition、no-store 和 nosniff。

## 命令与证据

工具链路径：`/Users/robot/.rustup/toolchains/stable-aarch64-apple-darwin/bin`，执行时加入 PATH。

- `cargo test --manifest-path src-tauri/Cargo.toml --lib actions::`：退出码 0，11 项通过，见 actions-tests.log。
- `cargo test --manifest-path src-tauri/Cargo.toml --lib`：退出码 0，116 项通过，见 lib-tests.log。
- `cargo build --manifest-path src-tauri/Cargo.toml`：退出码 0，见 build.log。
- `git diff --check`：退出码 0。
- `check_spec`：4 条需求通过，无错误和警告，见 spec-check.json。

编译仅有既存 `commands/ui_memory.rs::bytes_to_mb` 未使用警告。

## 审查

GitNexus 对 `serve` 和 `build_openapi` upstream 影响分析均为 LOW，直接调用者分别为 1 个和 4 个；涉及 Actions 启动与 Runtime 启动链。索引使用已有注册名称 coding-tools-browser-dev；源码核对了实际目标路径。

mcp-probe-kit code_review 返回审查指南，默认 Git diff 包含既有浏览器改动且不包含未跟踪文件正文，因此该工具结果不能单独作为 PDF 审查通过证据。已显式阅读新增 pdf.rs、pdf_tests.rs 和三个接入点，核对：静态专用路由不进入通用工具包装；POST 有原鉴权中间件；下载无需另发 Bearer 但必须持有效随机令牌；链接基址不来自请求 Host；读取与插入串行限制快照内存；原路径改变不影响已签发内容；缓存满不驱逐仍有效链接。未发现本次范围内待修复的高优先级问题。

边界：PDF 签名检查不是完整格式有效性校验；现有明确外部绝对路径的只读策略保留。ChatGPT 的原生附件接收、页内容分析以及真实公网 10 秒下载要求仍需目标 GPT 与服务部署后验收。没有使用私人 PDF 或调用外部模型处理文档。

## 接入

参见 ../../actions-pdf.md。目标自定义 GPT 的链接/名称尚待用户提供。运行新构建、更新目标 GPT 的 Actions schema 后，使用合成文件完成最后一段验收。
