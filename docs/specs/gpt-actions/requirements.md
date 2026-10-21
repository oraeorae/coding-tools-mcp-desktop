# GPT Actions PDF 原文件返回
## 功能概述
用户通过自定义 GPT 获取明确指定的本地 PDF，文件进入对话附件流程。只增加 Actions 接口及文档，不修改通用 MCP 工具、不替换已安装应用。
## 需求列表
- FR-1：POST /actions/return_pdf 接收 path，返回顶层 openaiFileResponse URL 数组，保留原始字节。
- FR-2：沿用 Actions 鉴权；文件路径沿用 resolve_read_path 只读权限，明确绝对路径允许外部文件，隐式外部符号链接拒绝。只接受普通 PDF，最大 10,000,000 字节。
- FR-3：下载能力令牌随机不可猜测，5 分钟失效，可重试；缓存有总量限制，停止服务清空；下载不依赖 Authorization。
- FR-4：OpenAPI 声明专用响应，包含导入自定义 GPT 和合成文件验收说明。
## 验收标准
有效文件下载字节一致；返回字段位于顶层；中文文件名通过响应头保留；无效、过期令牌不可下载；鉴权失败不可签发；超大、非 PDF、目录拒绝；同一路径被替换后下载仍为签发时原件。
## 非功能需求
每个工作区缓存最多 32 MB；只使用配置的公网 HTTPS 地址（localhost 允许开发 HTTP），不信任请求 Host；不上传私人文档做测试。
## 不在范围
ChatGPT 附件 UI 的自动化保证、OCR、摘要、真实私人 PDF 外传、生产应用升级。

## 依赖关系
复用 axum、uuid、reqwest、Workspace 与现有鉴权。
当用户请求 PDF 时，服务 SHALL 返回原始文件下载能力；当验证失败时 SHALL 返回错误且不签发链接。
