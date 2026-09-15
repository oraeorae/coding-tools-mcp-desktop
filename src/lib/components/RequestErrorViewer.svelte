<script lang="ts">
  import { onMount } from "svelte";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import {
    clearRequestErrors,
    readRequestErrors,
    type LogService,
    type RequestErrorEntry,
  } from "$lib/api/logs";

  interface Props {
    workspaceId: string;
    service: LogService;
  }

  let { workspaceId, service }: Props = $props();
  let entries = $state<RequestErrorEntry[]>([]);
  let busy = $state(false);
  let feedback = $state("");
  let error = $state("");

  async function refresh() {
    if (busy) return;
    busy = true;
    error = "";
    try {
      entries = await readRequestErrors(workspaceId, service);
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  function printableEntry(entry: RequestErrorEntry): string {
    const timestamp = new Date(entry.timestampMs).toLocaleString();
    const status = entry.status === null ? "无 HTTP 状态" : `HTTP ${entry.status}`;
    const details = [entry.source, entry.method, entry.path, status, entry.code, entry.message];
    if (entry.tool) details.push(`tool=${entry.tool}`);
    if (entry.requestId) details.push(`requestId=${entry.requestId}`);
    return `[${timestamp}] ${details.join(" | ")}`;
  }

  async function copyAll() {
    error = "";
    try {
      await navigator.clipboard.writeText(entries.map(printableEntry).join("\n"));
      feedback = `已复制 ${entries.length} 条错误`;
    } catch (err) {
      error = `复制失败：${String(err)}`;
    }
  }

  async function clearAll() {
    const accepted = await confirm(`确定清空当前工作区的 ${service === "mcp" ? "MCP" : "Actions"} 请求错误吗？`, {
      title: "清空请求错误",
      kind: "warning",
    });
    if (!accepted) return;
    error = "";
    try {
      await clearRequestErrors(workspaceId, service);
      entries = [];
      feedback = "请求错误已清空";
    } catch (err) {
      error = `清空失败：${String(err)}`;
    }
  }

  onMount(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), 3000);
    return () => window.clearInterval(timer);
  });
</script>

<section class="tx-card border-[var(--color-error)]/30 p-5">
  <div class="flex flex-wrap items-start justify-between gap-3">
    <div>
      <h3 class="font-semibold">请求错误</h3>
      <p class="mt-1 text-sm text-[var(--color-text-muted)]">
        最近 200 条；每个服务最多保留约 2 MiB。不会记录鉴权头、密钥或请求正文。
      </p>
    </div>
    <div class="flex flex-wrap gap-2">
      <button class="tx-btn-ghost disabled:opacity-50" type="button" disabled={busy} onclick={refresh}>
        {busy ? "刷新中…" : "刷新"}
      </button>
      <button class="tx-btn-ghost disabled:opacity-50" type="button" disabled={entries.length === 0} onclick={copyAll}>
        复制
      </button>
      <button class="tx-btn-ghost disabled:opacity-50" type="button" disabled={entries.length === 0} onclick={clearAll}>
        清空
      </button>
    </div>
  </div>

  {#if feedback}
    <p class="mt-3 text-sm text-[var(--color-text-muted)]">{feedback}</p>
  {/if}
  {#if error}
    <p class="mt-3 rounded-lg border border-[var(--color-error)]/30 bg-[var(--color-error)]/10 px-3 py-2 text-sm text-[var(--color-error)]">
      {error}
    </p>
  {/if}

  {#if entries.length > 0}
    <div class="mt-4 grid max-h-96 gap-2 overflow-auto">
      {#each entries as entry (entry.id)}
        <article class="rounded-lg border border-[var(--color-error)]/25 bg-[var(--color-error)]/5 p-3">
          <div class="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs">
            <span class="font-semibold text-[var(--color-error)]">
              {entry.status === null ? entry.code : `HTTP ${entry.status}`}
            </span>
            <span class="font-mono">{entry.method} {entry.path}</span>
            <span class="text-[var(--color-text-muted)]">{new Date(entry.timestampMs).toLocaleString()}</span>
          </div>
          <p class="mt-2 break-words text-sm">{entry.message}</p>
          {#if entry.tool || entry.requestId}
            <p class="mt-1 break-words font-mono text-xs text-[var(--color-text-muted)]">
              {entry.tool ? `tool=${entry.tool}` : ""}{entry.tool && entry.requestId ? " · " : ""}{entry.requestId ? `requestId=${entry.requestId}` : ""}
            </p>
          {/if}
        </article>
      {/each}
    </div>
  {:else if !busy && !error}
    <p class="mt-4 text-sm text-[var(--color-text-muted)]">当前没有请求错误</p>
  {/if}

  <p class="mt-4 text-xs text-[var(--color-text-muted)]">
    Cloudflare 在请求抵达本机前拦截的事件不会被动出现；运行健康检查后，检测到的公网错误会记录在这里。
  </p>
</section>
