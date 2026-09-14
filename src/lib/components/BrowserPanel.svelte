<script lang="ts">
  import { untrack } from "svelte";
  import { browserStatus, startBrowserBridge, stopBrowserBridge, type BrowserStatus } from "$lib/api/browser";
  interface Props { workspaceId: string }
  let { workspaceId }: Props = $props();
  let status = $state<BrowserStatus | null>(null);
  let busy = $state(false);
  let error = $state("");
  let generation = 0;
  const labels: Record<string,string> = { connected: "已连接", disconnected: "未连接", starting: "等待授权", external: "外部连接", error: "需要处理" };
  async function perform(action: typeof browserStatus) {
    if (busy) return;
    const current = generation, id = workspaceId;
    busy = true; error = "";
    try { const value = await action(id); if (current === generation) status = value; }
    catch (err) { if (current === generation) error = String(err); }
    finally { if (current === generation) busy = false; }
  }
  $effect(() => {
    const id = workspaceId;
    generation++; status = null; error = ""; busy = false;
    untrack(() => { void perform(browserStatus); });
    const timer = setInterval(() => { if (id === workspaceId && status?.state === "starting") void perform(browserStatus); }, 2500);
    return () => { clearInterval(timer); generation++; };
  });
</script>
<section class="tx-card p-5">
  <div class="flex flex-wrap items-start justify-between gap-3">
    <div>
      <h3 class="font-semibold">本机浏览器</h3>
      <p class="mt-1 text-sm text-[var(--color-text-muted)]">打开网址 · 读取正文 · 截图</p>
    </div>
    <span class="text-sm" class:text-green-600={status?.state === "connected"}>{status ? labels[status.state] ?? status.state : "检查中"}</span>
  </div>
  <p class="mt-3 whitespace-pre-wrap break-words text-sm text-[var(--color-text-muted)]">{status?.message ?? "正在检查本机连接…"}</p>
  {#if status && !status.nodeAvailable}<p class="mt-2 text-sm text-[var(--color-error)]">未找到 Node.js，需要安装 20 或更高版本。</p>{/if}
  {#if error}<p role="alert" class="mt-2 break-words text-sm text-[var(--color-error)]">{error}</p>{/if}
  <div class="mt-4 flex flex-wrap gap-2">
    <button type="button" class="tx-btn-primary disabled:opacity-50" disabled={busy || !status?.nodeAvailable || status?.state === "connected" || status?.state === "starting" || status?.state === "external"} onclick={() => perform(startBrowserBridge)}>连接浏览器</button>
    {#if status?.managed}<button type="button" class="tx-btn-ghost disabled:opacity-50" disabled={busy} onclick={() => perform(stopBrowserBridge)}>停止连接</button>{/if}
    <button type="button" class="tx-btn-ghost disabled:opacity-50" disabled={busy} onclick={() => perform(browserStatus)}>刷新状态</button>
  </div>
  {#if status}<p class="mt-3 text-xs text-[var(--color-text-muted)]">应用 {status.appVersion} · 核心工具 {status.toolCount} 个 · 客户端实际工具列表需在 ChatGPT 中核对</p>{/if}
  <p class="mt-3 text-xs text-[var(--color-text-muted)]">首次连接需要在 Chrome 的 Playwright 扩展中选择标签页。检查状态不会读取网页。更新应用后，如 ChatGPT 仍缺少工具，请刷新插件并开启新对话。</p>
</section>
