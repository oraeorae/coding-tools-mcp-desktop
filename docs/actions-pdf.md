# 从自定义 GPT 获取本地 PDF 原文件

本功能使用 GPT Actions 的 `openaiFileResponse` URL 返回方式。需要运行含此功能的 Coding Tools MCP 构建，然后在自定义 GPT 中导入新的 Actions OpenAPI；普通 MCP 工具返回不会自动获得这一能力。

## 配置

1. 在 Coding Tools MCP 的目标工作区启用 Actions 服务，配置 API Key 或 OAuth。
2. 确保 Actions 公网 HTTPS 地址指向该服务，且本机在线。不要使用 MCP 的 `/mcp` 地址。公网地址需写入 Actions 配置并重启 Actions 服务，使签发链接使用正确地址。
3. 在自定义 GPT 的编辑界面添加或更新 Action，从 **Actions 公网地址 + `/openapi.json`** 导入 schema，并设置与本地相同的鉴权。现有 GPT 也需要重新导入，旧 schema 不会包含新接口。
4. 开启自定义 GPT 的“代码解释器与数据分析”，方便进一步处理 PDF；具体附件读取效果以实际 GPT 验收为准。
5. 在 GPT Instructions 中加入下方说明，先用合成 PDF 测试，再使用所需文件。

## 建议 Instructions

> 用户要求获取或分析本地 PDF 时，先使用文件列表工具确认准确路径，再调用 coding_return_pdf。该操作会把完整 PDF 传入当前对话。仅请求用户选定的文件；多个同名文件先确认。不要使用 read_file 读取 PDF，不要把路径或普通链接当成已上传附件。附件返回后再分析；链接过期时重新调用。单个文件超过 10 MB 时说明限制，不能静默截断。

## 接口

```http
POST /actions/return_pdf
Authorization: Bearer <你的 Actions 凭据>
Content-Type: application/json

{"path":"reports/example.pdf"}
```

支持工作区相对路径，以及显式的本地绝对路径（沿用现有只读权限）。例如 `/Users/robot/Documents/reports/example.pdf`。相对路径中的隐式外部符号链接会被拒绝。

成功响应顶层直接包含：

```json
{"openaiFileResponse":["https://你的Actions域名/action-files/随机令牌"]}
```

`GET /action-files/{token}` 返回原始 PDF 字节，并设置 `Content-Type: application/pdf`、`Content-Disposition: attachment`（含中文 UTF-8 文件名）、`Cache-Control: no-store`。

链接自身是临时下载凭据，不需要额外 Authorization，适用于 ChatGPT 文件下载器。令牌有效 5 分钟，支持重试，失效后返回 404；它仅对应签发时的文件快照，不接受任意路径查询。重启服务会使所有旧链接失效。每个工作区最多缓存 32 MB / 64 个文件；满额返回 429，等待过期后重试。单个 PDF 最大 10,000,000 字节，本版本每次返回一个文件。

## 验收

- 本地 HTTP 测试：签发前要求有效鉴权；响应字段在顶层；无凭据下载成功；字节完全一致；中文响应头正确；本地文件被替换后仍能下载原件；无效/过期令牌返回 404；非 PDF/目录/超大文件被拒绝。
- ChatGPT 验收：导入新 schema 后确认存在 `coding_return_pdf`，要求获取测试 PDF；确认对话出现文件附件并能回答指定页内容。仅有 HTTP 测试成功不能证明 ChatGPT UI 已完成接入。
- 本次测试只使用程序生成的合成 PDF，没有传输私人 PDF。

官方契约：https://developers.openai.com/api/docs/actions/sending-files
