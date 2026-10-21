<script lang="ts">
  import { ChevronLeft, ChevronRight, FileText, RefreshCw } from "@lucide/svelte";
  import { readResearchHistory, type ResearchHistory, type ResearchState, type ResearchTopic } from "$lib/api/research";

  interface Props {
    workspaceId: string;
  }

  let { workspaceId }: Props = $props();
  let history = $state<ResearchHistory | null>(null);
  let loading = $state(true);
  let error = $state("");
  let selectedDate = $state("");
  let selectedTopicId = $state("");
  let loadGeneration = 0;

  const dateFormatter = new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
  const timeFormatter = new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });

  function dateKey(timestampMs: number): string {
    const date = new Date(timestampMs);
    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    return `${year}-${month}-${day}`;
  }

  const availableDates = $derived(
    Array.from(new Set((history?.topics ?? []).map((topic) => dateKey(topic.timestampMs)))).sort().reverse(),
  );
  const dayTopics = $derived(
    (history?.topics ?? []).filter((topic) => dateKey(topic.timestampMs) === selectedDate),
  );
  const selectedTopic = $derived(
    dayTopics.find((topic) => topic.id === selectedTopicId) ?? dayTopics[0] ?? null,
  );
  const evidenceCount = $derived(dayTopics.reduce((sum, topic) => sum + topic.evidence.length, 0));
  const reviewCount = $derived(dayTopics.filter((topic) => topic.state === "needs_review").length);

  function stateLabel(state: ResearchState): string {
    return {
      research: "研究记录",
      partial: "部分证据",
      verified: "已验证",
      needs_review: "待复核",
    }[state];
  }

  function ensureSelection() {
    if (!availableDates.includes(selectedDate)) {
      selectedDate = availableDates[0] ?? "";
    }
    if (!dayTopics.some((topic) => topic.id === selectedTopicId)) {
      selectedTopicId = dayTopics[0]?.id ?? "";
    }
  }

  async function load() {
    const generation = ++loadGeneration;
    loading = true;
    error = "";
    try {
      const result = await readResearchHistory(workspaceId);
      if (generation !== loadGeneration) return;
      history = result;
      ensureSelection();
    } catch (cause) {
      if (generation !== loadGeneration) return;
      error = String(cause);
      history = null;
    } finally {
      if (generation === loadGeneration) loading = false;
    }
  }

  function moveDay(offset: number) {
    const index = availableDates.indexOf(selectedDate);
    const next = availableDates[index + offset];
    if (!next) return;
    selectedDate = next;
    selectedTopicId = "";
    ensureSelection();
  }

  function selectDate(value: string) {
    selectedDate = value;
    selectedTopicId = "";
    ensureSelection();
  }

  function formattedDate(value: string): string {
    if (!value) return "暂无日期";
    return dateFormatter.format(new Date(`${value}T12:00:00`));
  }

  function evidenceGroups(topic: ResearchTopic) {
    const order = ["test", "finding", "runtime", "file", "note"];
    return order
      .map((kind) => ({
        kind,
        label: topic.evidence.find((item) => item.kind === kind)?.label ?? kind,
        items: topic.evidence.filter((item) => item.kind === kind),
      }))
      .filter((group) => group.items.length > 0);
  }

  $effect(() => {
    const id = workspaceId;
    void load();
    return () => {
      loadGeneration += 1;
    };
  });
</script>

<section class="research-shell" aria-labelledby="research-title">
  <header class="research-toolbar">
    <div>
      <p class="research-kicker">项目历史 · 只读</p>
      <h3 id="research-title">每日研究</h3>
      <p class="research-subtitle">按一天汇总研究主题、结论和可追溯证据。</p>
    </div>
    <div class="date-controls" aria-label="研究日期">
      <button type="button" class="icon-button" aria-label="后一天" disabled={availableDates.indexOf(selectedDate) <= 0} onclick={() => moveDay(-1)}>
        <ChevronLeft size={17} />
      </button>
      <label>
        <span class="sr-only">选择研究日期</span>
        <select value={selectedDate} onchange={(event) => selectDate(event.currentTarget.value)} disabled={availableDates.length === 0}>
          {#each availableDates as date}
            <option value={date}>{formattedDate(date)}</option>
          {/each}
        </select>
      </label>
      <button type="button" class="icon-button" aria-label="前一天" disabled={availableDates.indexOf(selectedDate) < 0 || availableDates.indexOf(selectedDate) >= availableDates.length - 1} onclick={() => moveDay(1)}>
        <ChevronRight size={17} />
      </button>
      <button type="button" class="refresh-button" onclick={() => void load()} disabled={loading}>
        <span class:spinning={loading} aria-hidden="true"><RefreshCw size={15} /></span>
        刷新
      </button>
    </div>
  </header>

  {#if loading}
    <div class="research-state" aria-live="polite">正在读取项目历史…</div>
  {:else if error}
    <div class="research-state error" role="alert">
      <strong>无法读取研究历史</strong>
      <span>{error}</span>
      <button type="button" onclick={() => void load()}>重新读取</button>
    </div>
  {:else if !history || history.topics.length === 0}
    <div class="research-state">
      <FileText size={22} />
      <strong>还没有可展示的研究历史</strong>
      <span>完成历史初始化并保存检查点后，主题、结论和证据会出现在这里。</span>
    </div>
  {:else}
    <div class="day-summary" aria-live="polite">
      <strong>{formattedDate(selectedDate)}</strong>
      <span>{dayTopics.length} 个主题</span>
      <span>{evidenceCount} 条证据</span>
      {#if reviewCount > 0}<span class="review-summary">{reviewCount} 项待复核</span>{/if}
      {#if history.truncated}<span>仅展示最近 {history.scannedFiles} 份历史</span>{/if}
    </div>

    {#if dayTopics.length === 0}
      <div class="research-state">当天没有研究记录。</div>
    {:else}
      <div class="research-workbench">
        <div class="topic-pane" aria-label="当天研究主题">
          <div class="pane-heading">
            <span>研究主题</span>
            <span>{dayTopics.length}</span>
          </div>
          <div class="topic-list">
            {#each dayTopics as topic}
              <button
                type="button"
                class="topic-row"
                class:selected={selectedTopic?.id === topic.id}
                aria-pressed={selectedTopic?.id === topic.id}
                onclick={() => (selectedTopicId = topic.id)}
              >
                <span class="topic-main">
                  <span class="topic-title">{topic.title}</span>
                  <span class="topic-summary">{topic.summary}</span>
                </span>
                <span class="topic-meta">
                  <span class="state-label" data-state={topic.state}>{stateLabel(topic.state)}</span>
                  <time datetime={new Date(topic.timestampMs).toISOString()}>{timeFormatter.format(new Date(topic.timestampMs))}</time>
                </span>
              </button>
            {/each}
          </div>
        </div>

        {#if selectedTopic}
          <article class="evidence-pane" aria-labelledby="selected-topic-title">
            <header class="evidence-header">
              <div>
                <div class="evidence-status">
                  <span class="state-label" data-state={selectedTopic.state}>{stateLabel(selectedTopic.state)}</span>
                  <span>{selectedTopic.sessionTitle}</span>
                </div>
                <h4 id="selected-topic-title">{selectedTopic.title}</h4>
              </div>
              <time datetime={new Date(selectedTopic.timestampMs).toISOString()}>
                {formattedDate(dateKey(selectedTopic.timestampMs))} {timeFormatter.format(new Date(selectedTopic.timestampMs))}
              </time>
            </header>

            <section class="conclusion-block" aria-labelledby="conclusion-heading">
              <p id="conclusion-heading" class="section-label">结论</p>
              {#if selectedTopic.conclusions.length > 0}
                <ul>
                  {#each selectedTopic.conclusions as conclusion}<li>{conclusion}</li>{/each}
                </ul>
              {:else}
                <p>{selectedTopic.summary}</p>
              {/if}
            </section>

            {#if selectedTopic.evidence.length > 0}
              <section class="evidence-groups" aria-labelledby="evidence-heading">
                <p id="evidence-heading" class="section-label">相关证据</p>
                {#each evidenceGroups(selectedTopic) as group}
                  <details open={group.kind === "test" || group.kind === "finding"}>
                    <summary><span>{group.label}</span><span>{group.items.length}</span></summary>
                    <ul>
                      {#each group.items as item}<li>{item.detail}</li>{/each}
                    </ul>
                  </details>
                {/each}
              </section>
            {:else}
              <section class="empty-evidence">
                <p class="section-label">相关证据</p>
                <p>尚未保存发现、验证、运行状态或相关文件。</p>
              </section>
            {/if}

            {#if selectedTopic.limitations.length > 0}
              <section class="limitations" aria-labelledby="limitations-heading">
                <p id="limitations-heading" class="section-label">待复核与限制</p>
                <ul>{#each selectedTopic.limitations as limitation}<li>{limitation}</li>{/each}</ul>
              </section>
            {/if}

            <footer class="source-footer">
              <FileText size={15} />
              <span>原始记录</span>
              <code>{selectedTopic.sourcePath}</code>
            </footer>
          </article>
        {/if}
      </div>
    {/if}

    {#if history.warnings.length > 0}
      <div class="history-warnings" role="status">
        {#each history.warnings as warning}<p>{warning}</p>{/each}
      </div>
    {/if}
  {/if}
</section>

<style>
  .research-shell {
    border: 1px solid var(--color-border);
    border-radius: 14px;
    background: var(--color-surface);
    overflow: hidden;
  }
  .research-toolbar {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 20px;
    padding: 20px 22px 18px;
    border-bottom: 1px solid var(--color-border);
  }
  .research-kicker, .section-label, .pane-heading {
    margin: 0;
    color: var(--color-text-muted);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: .08em;
    text-transform: uppercase;
  }
  h3, h4, p { margin: 0; }
  h3 { margin-top: 3px; font-size: 22px; line-height: 1.25; }
  h4 { margin-top: 8px; font-size: 18px; line-height: 1.4; }
  .research-subtitle { margin-top: 4px; color: var(--color-text-secondary); }
  .date-controls { display: flex; align-items: center; gap: 7px; }
  .date-controls select, .icon-button, .refresh-button, .research-state button {
    min-height: 36px;
    border: 1px solid var(--color-border);
    border-radius: 8px;
    background: transparent;
    color: var(--color-text);
  }
  .date-controls select { padding: 0 30px 0 10px; }
  .icon-button { width: 36px; display: grid; place-items: center; }
  .refresh-button { display: inline-flex; align-items: center; gap: 6px; padding: 0 11px; }
  button:not(:disabled) { cursor: pointer; }
  button:disabled { opacity: .45; }
  .spinning { animation: spin .8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .day-summary {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 16px;
    padding: 11px 22px;
    border-bottom: 1px solid var(--color-border);
    color: var(--color-text-secondary);
    font-size: 13px;
  }
  .day-summary strong { color: var(--color-text); }
  .review-summary { color: var(--color-warning); font-weight: 650; }
  .research-workbench { display: grid; grid-template-columns: minmax(270px, 38%) minmax(0, 62%); min-height: 430px; }
  .topic-pane { border-right: 1px solid var(--color-border); min-width: 0; }
  .pane-heading { display: flex; justify-content: space-between; padding: 13px 16px; border-bottom: 1px solid var(--color-border); }
  .topic-list { display: flex; flex-direction: column; }
  .topic-row {
    width: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 12px;
    padding: 14px 16px;
    border: 0;
    border-bottom: 1px solid var(--color-border);
    background: transparent;
    color: inherit;
    text-align: left;
  }
  .topic-row:hover { background: var(--color-surface-hover); }
  .topic-row.selected { background: var(--primary-soft); box-shadow: inset 3px 0 0 var(--color-accent); }
  .topic-main { min-width: 0; }
  .topic-title, .topic-summary { display: block; }
  .topic-title { font-weight: 650; line-height: 1.45; }
  .topic-summary { margin-top: 5px; color: var(--color-text-secondary); font-size: 12px; line-height: 1.45; overflow: hidden; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; }
  .topic-meta { display: flex; flex-direction: column; align-items: flex-end; gap: 8px; color: var(--color-text-muted); font-size: 11px; white-space: nowrap; }
  .state-label { display: inline-flex; align-items: center; min-height: 22px; padding: 1px 7px; border-radius: 999px; border: 1px solid var(--color-border); font-size: 11px; font-weight: 700; white-space: nowrap; }
  .state-label[data-state="verified"] { color: var(--color-success); border-color: color-mix(in srgb, var(--color-success) 35%, var(--color-border)); }
  .state-label[data-state="partial"] { color: var(--color-accent); border-color: color-mix(in srgb, var(--color-accent) 35%, var(--color-border)); }
  .state-label[data-state="needs_review"] { color: var(--color-warning); border-color: color-mix(in srgb, var(--color-warning) 42%, var(--color-border)); }
  .evidence-pane { min-width: 0; padding: 20px 22px; }
  .evidence-header { display: flex; justify-content: space-between; align-items: flex-start; gap: 18px; padding-bottom: 18px; border-bottom: 1px solid var(--color-border); }
  .evidence-header time { color: var(--color-text-muted); font-size: 12px; white-space: nowrap; }
  .evidence-status { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; color: var(--color-text-muted); font-size: 12px; }
  .conclusion-block, .evidence-groups, .empty-evidence, .limitations { padding: 18px 0; border-bottom: 1px solid var(--color-border); }
  .conclusion-block ul, .limitations ul, details ul { margin: 10px 0 0; padding-left: 20px; }
  .conclusion-block li, .limitations li, details li { margin-top: 7px; line-height: 1.6; }
  .conclusion-block > p:last-child, .empty-evidence > p:last-child { margin-top: 10px; line-height: 1.65; color: var(--color-text-secondary); }
  details { border-bottom: 1px solid var(--color-border); padding: 0; }
  details:last-child { border-bottom: 0; }
  summary { display: flex; justify-content: space-between; gap: 12px; padding: 12px 0; font-weight: 650; cursor: pointer; }
  details ul { margin-top: 0; padding-bottom: 12px; color: var(--color-text-secondary); }
  .limitations { color: var(--color-warning); }
  .limitations .section-label { color: currentColor; }
  .source-footer { display: flex; align-items: center; flex-wrap: wrap; gap: 7px; padding-top: 16px; color: var(--color-text-muted); font-size: 12px; }
  .source-footer code { color: var(--color-text-secondary); overflow-wrap: anywhere; }
  .research-state { min-height: 300px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 9px; padding: 32px; color: var(--color-text-secondary); text-align: center; }
  .research-state strong { color: var(--color-text); }
  .research-state button { margin-top: 4px; padding: 0 12px; }
  .research-state.error { color: var(--color-error); }
  .history-warnings { padding: 12px 20px; border-top: 1px solid var(--color-border); color: var(--color-warning); font-size: 12px; }
  .history-warnings p + p { margin-top: 4px; }
  .sr-only { position: absolute; width: 1px; height: 1px; padding: 0; margin: -1px; overflow: hidden; clip: rect(0, 0, 0, 0); white-space: nowrap; border: 0; }
  @media (max-width: 820px) {
    .research-toolbar { flex-direction: column; }
    .date-controls { width: 100%; }
    .date-controls label { flex: 1; }
    .date-controls select { width: 100%; }
    .research-workbench { grid-template-columns: 1fr; }
    .topic-pane { border-right: 0; border-bottom: 1px solid var(--color-border); }
  }
  @media (max-width: 520px) {
    .research-toolbar, .evidence-pane { padding: 16px; }
    .date-controls { flex-wrap: wrap; }
    .date-controls label { min-width: 160px; }
    .icon-button, .refresh-button, .date-controls select { min-height: 44px; }
    .day-summary { padding: 10px 16px; }
    .topic-row { grid-template-columns: 1fr; }
    .topic-meta { flex-direction: row; align-items: center; justify-content: space-between; }
    .evidence-header { flex-direction: column; }
  }
  @media (prefers-reduced-motion: reduce) {
    .spinning { animation: none; }
  }
</style>
