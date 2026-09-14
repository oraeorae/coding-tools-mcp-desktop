# 本地 Chrome 浏览器桥接

此功能提供四个工具：打开网址、列出已连接标签页、读取正文、截图。

## 使用前准备

1. 使用包含此功能的桌面端构建。旧版已安装的 0.2.1 不会因为下载源码而自动获得工具。
2. 安装 Node.js 20 或更高版本，并在 Chrome 中安装微软官方 **Playwright Extension**。安装入口与说明：<https://github.com/microsoft/playwright/tree/main/packages/extension#readme>。
3. 打开你要分享的 HTTP/HTTPS 网页。首次连接使用扩展的选择/确认界面，不需要复制 cookie，也不需要开放 Chrome 调试端口。

## 桌面端连接（0.3.0）

工作区页面的“本机浏览器”提供连接、停止和刷新状态。安装包内含 Node 桥接脚本及依赖，使用已安装的 Node.js 20+；首次仍需在 Playwright 扩展中选择分享的标签页。已由外部终端启动的连接会标记为外部管理，桌面端不会终止它。退出桌面端会关闭它自己启动的桥接。

状态检查仅调用本机带鉴权 `/health`，不列标签、不读取正文。旧桥接没有此接口，会提示在原终端停止后重新连接。清理陈旧配置只在本机端口明确拒绝连接且配置内容未改变时进行，超时或鉴权失败不会删除配置。

## 手动启动（开发/兼容方式）

在仓库根目录执行一次：

```sh
npm run browser:setup
```

每次需要浏览器时执行，路径必须是桌面端中同一个工作区目录：

```sh
npm run browser:start -- --workspace "/absolute/path/to/your/workspace"
```

在 Chrome 打开的扩展连接界面选择要分享的标签页。终端显示 `Browser bridge ready` 后，已有 MCP 客户端可以使用浏览器工具。首次连接最多等待 120 秒；窗口没有出现时检查 Chrome 配置文件是否安装并启用了 Playwright Extension。

启动命令明确选择工作区；浏览器读取不会由某次远程调用偷偷开启。桥接保持前台运行，按 Ctrl+C 断开。一个工作区同时只允许一个桥接进程；不同工作区需分别启动和授权。

本版不依赖 Codex 的 @Chrome，也不直接对外暴露 Playwright MCP。Node 依赖独立安装，原有桌面构建不需要打包 Node。可以在克隆目录中运行 `npm ci`、`npm run desktop` 测试修改后的桌面程序；不要把“源码已改”当成“现有安装已升级”。

## 工具用法

### browser_open

```json
{"url":"https://example.com/article"}
```

在已连接的 Chrome 上下文中新建标签页，保留原页面。等待 DOMContentLoaded 后返回 `tab_id`、标题和最终 URL，并将新页设为桥接的默认页。接着调用 `browser_read_page({})` 读取正文，再由模型分析；需要图表时调用 `browser_screenshot({})`。这里的默认页是桥接选中状态，不保证切换 Chrome 的前台焦点。

只接受完整 HTTP/HTTPS 地址，最多 2048 UTF-8 字节，不接受用户名密码、空白、控制字符、反斜线或其它参数。导航等待最多 10 秒，失败返回 `NAVIGATION_FAILED` 并关闭失败新页，保留之前的默认页。网站返回的 404 等错误页仍可读取，动态网页可能需要稍后再次读取；工具不绕过登录或验证码。

打开网页会访问网站并改变浏览器状态，因此 `browser_open` 不在 read-only profile 中。只有本机桥接已连接后才可调用。读取具体旧标签不会改变新默认页。

### browser_list_tabs

```json
{"cursor":0,"limit":50}
```

返回已连接上下文内 HTTP/HTTPS 标签的 `tab_id`、`title`、`url`、`selected`，以及 `selected_tab_id` 和 `next_cursor`。最多列 200 个，超限标记 `truncated`。扩展只提供它授权给当前连接的页面，不保证列出 Chrome 所有窗口中的所有标签。

### browser_read_page

```json
{}
```

默认读取最近成功 `browser_open` 打开的页；尚未调用打开工具时，读取首次本地连接时选中的页。指定另一页：

```json
{"tab_id":"从列表返回的ID","cursor":0,"max_chars":16000}
```

返回主文档可见文字和来源标题/URL。跟随 `next_cursor` 取下一页；单次最多 32000 个 Unicode 字符，不拆分 emoji。隐藏元素、输入框值、cookie 和存储不主动采集；canvas、图片、跨域 iframe、虚拟列表未加载内容不在正文提取范围。动态页面两次读取之间可能变化。

标签 ID 仅在本次连接有效。读取其他 tab_id 不改变默认页；默认页关闭后报错，不切换其他标签。重新选择默认页：Ctrl+C 后重新启动桥接，在扩展中选择。

### browser_screenshot

```json
{}
```

也接受 `tab_id`。返回当前视口 JPEG，最多 2 MiB，通过 MCP image content 显示。不滚动、不切换焦点、不保存工作区文件。截图包含页面当前可见内容；网页文字和截图均是未受信任的外部内容。

## 连接与权限

- 外部调用仍通过原有 MCP/Actions 的鉴权与共同工具分发入口。
- 三个读取工具在 core、read-only、advanced profile 中可发现；`browser_open` 在 core、advanced 提供，并标记为非只读；旧 compat-readonly-all 配置也保留这个工具的真实非只读标注；未在本机启动对应工作区桥接时返回 `BROWSER_NOT_CONNECTED`。
- 桥接只监听 `127.0.0.1` 随机端口，要求随机 bearer token，拒绝跨站 Origin/非本机 Host；不设置 CORS 放行。
- 本地配置在 `~/.coding-tools-mcp/browser/<工作区规范路径SHA256>.json`。Unix 目录权限 0700、文件 0600。路径与 token 由本地启动创建，远程工具参数不能设置 endpoint、token、脚本或文件路径。
- 不要给桥接端口配置 FRP/Cloudflare 隧道。只保留原有 MCP 端口的对外连接。
- 固定的页面提取逻辑通过内存中的 Playwright MCP 调用执行；通用脚本执行工具不对外暴露。桥接请求串行且有超时，超时后要求本地重启。
- 桥接不保存正文、截图或登录态；Playwright 临时输出目录在正常退出时删除。

## 故障排查

| 错误 | 操作 |
| --- | --- |
| BROWSER_EXTENSION_MISSING | 在 Chrome 安装官方 Playwright Extension 后重启桥接 |
| BROWSER_NOT_CONNECTED | 确认桥接在运行，`--workspace` 与桌面端工作区一致 |
| BRIDGE_ALREADY_CONFIGURED | 停止旧桥接；若进程已异常退出，检查配置中的端口对应进程，确认停止后仅删除该工作区的残留配置再启动 |
| TAB_NOT_FOUND | 原页面关闭或旧 ID 失效，刷新列表；默认页关闭需本地重新选择 |
| UNSUPPORTED_PAGE | 选择 HTTP/HTTPS 页；不读取 chrome://、file://、扩展页 |
| NAVIGATION_FAILED | 检查网址或网络后重试；原默认页保持不变 |
| PAGE_CHANGED | 页面读取时发生导航，刷新列表后重试 |
| BROWSER_BUSY | 前一请求尚未完成，稍后重试 |
| BROWSER_TIMEOUT / BROWSER_UNAVAILABLE | 检查浏览器和扩展，Ctrl+C 后重新连接 |
| OUTPUT_TOO_LARGE | 缩小浏览器窗口后截图 |

## 开发验证

```sh
npm run test:browser
cargo test --manifest-path src-tauri/Cargo.toml --lib --tests
cargo test --manifest-path src-tauri/Cargo.toml --test browser_tools -- --include-ignored
```

真实浏览器测试使用独立的无头 Chromium 和合成 localhost 页面，不连接个人配置文件。需要 Playwright Chromium（在 `bridge/browser` 中运行 `npx playwright install chromium`），或设置 `BROWSER_TEST_EXECUTABLE` 为本机 Chrome 可执行文件路径。最后一条显式启用 Rust → 本机桥接 → 浏览器端到端测试。

这些测试不等于个人 Chrome 的扩展已安装或连接成功。实际连接必须看到扩展选择界面及 `Browser bridge ready`，再从 MCP 读取所选页面确认。
