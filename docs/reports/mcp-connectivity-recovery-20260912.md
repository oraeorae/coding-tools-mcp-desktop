# MCP 公网连接恢复

2026-09-12：配置 JSON 与应用替换前备份逐项相同，字节差异仅格式/序列化。工作区、端口、域名、OAuth 和密钥未改。

故障：量化研究 MCP stopped，28766 未监听；Actions 在 8788 运行。公网实际返回 530 / Cloudflare 1033，隧道不可用。

处理：通过已安装应用 UI 启动现有 MCP；kickstart 既有 com.robot.cloudflared.coding-tools-mcp 服务，未修改配置、路由或鉴权。

验收：本地 MCP 28766、代理 28767、公网 mcp.weorall.com/mcp 的未授权 POST 均返回 401。使用已连接 Coding Tools MCP weorall 成功调用 get_default_cwd 和 server_info，工作区 /Users/robot/Documents/量化、OAuth enabled、core 30 工具，含四个 browser 工具。

浏览器桥接尚未连接，量化工作区对应的桥接配置不存在。公网 MCP 恢复不代表浏览器已经授权。
