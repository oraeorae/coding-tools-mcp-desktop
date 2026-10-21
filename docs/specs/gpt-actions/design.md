# 设计
## 概述
src-tauri/src/actions/listener.rs 的 serve 注册鉴权路由，execute_action 包装通用结果。openapi.rs 的 build_openapi 生成工具接口。tools/workspace.rs 的 resolve_read_path 提供只读路径校验。
## 技术方案：文件接口（FR-1、FR-2）
独立 pdf.rs 模块接收 Workspace 与 AuthConfig。POST /actions/return_pdf 在原鉴权中间件内；限定普通文件、PDF 扩展名及 %PDF- 头、10 MB，限量读取并保存字节快照。
## 下载生命周期（FR-3）
UUID v4 capability URL，5 分钟 TTL。Arc 字节快照和文件名缓存在内存，32 MB 总上限，定期和读写时清理。GET /action-files/{token} 仅验证 capability，返回 application/pdf、attachment、no-store、nosniff。链接基址来自配置，开发允许 loopback HTTP，生产 HTTPS。
## OpenAPI 与兼容（FR-4）
在 build_openapi 添加专用路径，顶层 openaiFileResponse 是 URI 数组。现有 generic execute_action 不变。独立模块路由先合并到现有服务。
## 测试及风险
本地 HTTP 测试覆盖鉴权、签发、下载、中文头、无效令牌及原件快照；单元测试覆盖过期、缓存上限及路径边界。保留已有工作树改动。ChatGPT 自定义 GPT 导入需运行更新后的应用和配置公网 Actions 地址后实测。

## 文件结构
新增 actions/pdf.rs 和 actions/pdf_tests.rs，接入 mod.rs、listener.rs、openapi.rs；说明 docs/actions-pdf.md。
