# 本机应用替换验收

- 替换位置：/Applications/Coding Tools MCP.app
- 来源：本工作区新增四个浏览器工具后的源码，npm ci 后执行 Tauri release 构建；版本仍为 0.2.1。
- 架构：arm64，与本机 Apple Silicon 匹配；旧应用 universal 包保留在备份中。
- 本机 ad-hoc 签名，codesign --verify --deep --strict 通过。
- 备份：/Users/robot/Documents/ChatGPT/tools-coding/app-backup-20260912-234024
- 应用启动成功，界面显示 WorkBuddy、量化研究两工作区。
- profiles.json SHA-256：16d257db7e75f67d73015fcbcd89220d00d7ef8a829b6677b38041a7754e1457。替换前后和首次启动后完全一致，工作区、OAuth 密钥、隧道、端口和偏好未改写。
- 已安装可执行文件 SHA-256：8a351dc2dbac739df08d878c1c5bfd651f99ea7ce9084cc94d4e9195a13f5e39，与构建产物一致，包含四个浏览器工具。
- 替换前两工作区 MCP 均未监听；替换后保持 stopped，未擅自启动公网服务。
- 浏览器扩展和本机桥接仍需单独启用；应用替换不会自动安装 Playwright 扩展。

回滚：退出应用，用备份中的 original-at-replacement.app 恢复 /Applications/Coding Tools MCP.app。config 目录为配置备份；本次未覆盖原配置。
