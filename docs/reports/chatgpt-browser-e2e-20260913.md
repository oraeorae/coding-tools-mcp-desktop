# ChatGPT 原对话浏览器验收

对话：https://chatgpt.com/c/6aa573af-6704-83e9-b427-7c035c58d90a

## 复现与修复
原对话仍使用旧的 26 工具快照；server_info 报告 30，但模型不能实际调用 browser 工具。已停止旧的终端权限重试，发送仅针对公开 Example Domain 的验收请求，复现缺少四工具。

在 ChatGPT 插件 -> Coding Tools MCP weorall -> 插件操作 -> 管理 -> 刷新后，管理页明确显示 browser_list_tabs、browser_open、browser_read_page、browser_screenshot。未改动本地配置、密钥或命令白名单。

## 原对话真实验收
刷新对话页面，发送公开网页验收：browser_open https://example.com/?chatgpt-browser-test=2，随后正文读取与截图。ChatGPT 完成工具调用并报告通过；工具详情显示截图 ok=true、image/jpeg、14763 bytes，tab_id=tab_mtyl71vs_0usmp68w7o9m，URL 与测试 URL 一致。正文包含 Example Domain 及其说明。

实际调用耗时约 2 分 19 秒。过程中未将等待状态误判为成功；最终核对了 ChatGPT 工具返回和本地 MCP 请求完成记录。

## 边界
只测试公开网页，未读取论坛私人内容；浏览器桥接仍需保持运行。本次是主动单次验收，没有创建持续监控任务。

官方刷新文档：https://developers.openai.com/plugins/deploy/connect-chatgpt#refresh-metadata
