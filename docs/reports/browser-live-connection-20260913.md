# 实机浏览器连接验证

Playwright Extension 0.4.0 已获用户授权；量化研究工作区本机桥接启动成功。

通过带本机 token 的桥接依次调用 list/open/read/screenshot：均成功。使用 Example Domain 公开测试页，打开后默认页 ID 一致，正文包含 Example Domain，截图 JPEG。未读取个人标签内容。

当前桥接在 Codex 启动的前台进程运行，未配置开机自动启动。外部 MCP 工具已注册；旧连接器会话可能仍需重新同步工具列表。
