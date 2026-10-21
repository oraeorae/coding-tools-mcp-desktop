# 需求文档：打开网页后分析
## 功能概述
在已有本机桥接中增加 browser_open，通过现有 MCP 接收 URL，打开后供读取与分析。
## 范围边界
包含导航、默认页状态、参数验证、注册与测试。排除点击、表单、任意脚本、安装替换。
## 术语定义
默认页是首次本地选择或最近成功 browser_open 的标签页。
## 需求列表
### FR-1: 打开指定网页
**优先级:** Must
**用户故事:** 用户发送网址后希望打开并分析内容。
#### 验收标准（EARS）
WHEN 调用 browser_open 且 url 是有效 HTTP/HTTPS 地址 THEN 系统 SHALL 新建标签并等待 DOMContentLoaded，返回 tab_id、最终 URL、标题及 selected=true。
WHEN 打开成功 THEN 系统 SHALL 将新标签作为后续无 tab_id 读取、截图与列表的默认页。
IF 导航失败 THEN 系统 SHALL 返回 NAVIGATION_FAILED，关闭失败新标签并保留旧默认页；原网页不被导航。
### FR-2: 权限与有界输入
**优先级:** Must
**用户故事:** 用户希望导航保持本机连接与外部鉴权边界。
#### 验收标准（EARS）
WHEN 注册 browser_open THEN 系统 SHALL 标记非只读，仅在 core 和 advanced 提供，read-only 拒绝。
IF URL 缺失、非字符串、超过 2048 UTF-8 字节、包含凭据或控制字符、空白、反斜线、非 HTTP/HTTPS 协议或含未知参数 THEN 系统 SHALL 拒绝。
WHEN 调用导航 THEN 系统 SHALL 保留本机回环桥接和既有 MCP 鉴权，不增加公网入口。
## 非功能需求
导航等待最多 10 秒，已有桥接总超时与串行规则不变。网页内容仍是不可信数据。动态内容不保证完全加载。
## 依赖关系
复用已锁定 Playwright context.newPage/page.goto API 和 reqwest::Url，不增加依赖。
## 检查清单
- [x] 可测试需求与范围已明确。
