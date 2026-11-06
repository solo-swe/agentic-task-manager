<script lang="ts">
  import { BACKLOG } from '../filters';
  import { forecastSprints, type SprintForecast } from '../forecast';
  import { formatWhen } from '../format';
  import { progressBreakdown } from '../insights';
  import { notDone, RESOLUTION_LABELS } from '../relations';
  import { dashboard } from '../state.svelte';
  import { kindStyle, phaseColor, typeStyle } from '../theme';
  import { kindOf, statusLabel as vocabStatusLabel } from '../vocab';
  import Icon from '../ui/Icon.svelte';
  import Ring from '../ui/Ring.svelte';

  // Compact: a progress bar. Click: overall completion, the status split, and completion per phase and type.
  // Archived tasks count (all done): from meta.archived until the archive is loaded, then from the history (T-238)
  const p = $derived(
    dashboard.archiveLoaded
      ? progressBreakdown(dashboard.history, dashboard.vocab, 0, dashboard.archivedCount)
      : progressBreakdown(dashboard.tasks, dashboard.vocab, dashboard.archivedCount),
  );
  const pct = $derived(Math.round(p.overall.ratio * 100));
  let open = $state(false);
  // The popover's breakdown by phase and type needs the archived tasks themselves
  $effect(() => {
    if (open && dashboard.archivedCount > 0) dashboard.wantArchive();
  });
  const archivedNote = $derived(p.archived ? ` (${p.archived} archived, done)` : '');
  let root: HTMLElement;
  const capitalize = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
  const statusStyle = (status: string) => kindStyle(kindOf(dashboard.vocab, status));
  const statusLabel = (status: string) => vocabStatusLabel(dashboard.vocab, status);

  // By sprint (T-243, SR-7.4): a ring for each sprint not closed, and the backlog; the closed ones, which would be a row
  // of full rings, are one chip (✓ 12 closed) that opens the Progress tab on the last of them.
  const source = $derived(dashboard.archiveLoaded ? dashboard.history : dashboard.tasks);
  // The finished tasks that weren't done, by resolution (SR-2.8)
  const ended = $derived(notDone(source, dashboard.vocab));
  const rows = $derived(forecastSprints(source, Date.now(), source, dashboard.vocab, dashboard.sprints).sprints);
  const openRows = $derived(rows.filter((r) => r.sprint?.state !== 'closed'));
  const closedRows = $derived(rows.filter((r) => r.sprint?.state === 'closed'));
  const closedTitle = $derived.by(() => {
    const when = Math.max(...closedRows.map((r) => Date.parse(r.sprint?.end ?? '') || (r.completedAt ?? -Infinity)));
    return (
      `${closedRows.length === 1 ? closedRows[0].key : `${closedRows.length} sprints`}: closed` +
      `${Number.isFinite(when) ? `, the last on ${formatWhen(when, Date.now())}` : ''} · open on the Progress tab`
    );
  });
  const ratio = (r: SprintForecast) => (r.total ? r.done / r.total : r.skipped ? 1 : 0);
  const ringColor = (key: string) => {
    const index = dashboard.sprints.sprints.findIndex((s) => s.id === key);
    return index < 0 ? 'var(--text-muted)' : phaseColor(index);
  };
  const rowLabel = (key: string) => (key === BACKLOG ? 'Backlog' : key);
  function openSprint(key: string) {
    dashboard.progressSprint = key;
    dashboard.selectTab('progress');
    open = false;
  }

  // Go to: the Progress tab, and the Kanban filtered to the statuses that need a look
  const countOfKind = (kind: string) =>
    p.byStatus.filter((s) => kindOf(dashboard.vocab, s.status) === kind).reduce((n, s) => n + s.count, 0);
  const GO_TO = [
    { kind: 'review', label: 'In review' },
    { kind: 'blocked', label: 'Blocked' },
    { kind: 'active', label: 'In progress' },
  ] as const;
  function showStatuses(statuses: string[]) {
    dashboard.clearFilters();
    for (const status of statuses) dashboard.toggleFilter('status', status);
    dashboard.selectTab('kanban');
    open = false;
  }
  const statusesOfKind = (kind: string) => dashboard.vocab.statuses.filter((s) => s.kind === kind).map((s) => s.id);
</script>

<svelte:window
  onclick={(e) => open && !root.contains(e.target as Node) && (open = false)}
  onkeydown={(e) => e.key === 'Escape' && (open = false)}
/>

<div class="widget" bind:this={root}>
  <button
    class="widget-card progress-card"
    id="progress-card"
    aria-expanded={open}
    title="Progress: {p.overall.done} of {p.overall.total} tasks done{archivedNote} · click for details"
    aria-label="Progress {pct}%, {p.overall.done} of {p.overall.total} tasks done"
    onclick={() => (open = !open)}
  >
    <span class="widget-body">
      <!-- A small ring like the panel's, without its built-in label: the number sits beside it -->
      <svg class="mini-ring" width="20" height="20" viewBox="0 0 20 20" aria-hidden="true">
        <circle cx="10" cy="10" r="8" fill="none" stroke="var(--surface2)" stroke-width="3" />
        {#if p.overall.ratio > 0}
          <circle
            cx="10"
            cy="10"
            r="8"
            fill="none"
            stroke="var(--green)"
            stroke-width="3"
            stroke-linecap="round"
            stroke-dasharray="{p.overall.ratio * 2 * Math.PI * 8} {2 * Math.PI * 8}"
            transform="rotate(-90 10 10)"
          />
        {/if}
      </svg>
      <span class="number">{pct}%</span>
    </span>
    <Icon name="chevron-down" size={14} />
  </button>
  {#if open}
    <div class="insights-panel" id="progress-panel" role="dialog" aria-label="Progress details">
      <div class="panel-heading">Progress</div>
      <div class="insights-hero">
        <Ring
          ratio={p.overall.ratio}
          color="var(--green)"
          size={132}
          stroke={12}
          label="All tasks"
          skipped={p.overall.skipped}
          total={p.overall.total}
        />
        <div class="insights-hero-text">
          <div class="insights-title">
            {p.overall.done} of {p.overall.total} tasks done{#if p.archived}<span
                class="skipped-note"
                id="overall-archived">&nbsp;· {p.archived} archived</span
              >{/if}{#if p.overall.skipped}<span class="skipped-note" id="overall-skipped"
                >&nbsp;· {p.overall.skipped} skipped</span
              >{/if}
          </div>
          <!-- Status split: a stacked bar (part of a whole), labelled in the legend -->
          <div class="status-bar" aria-hidden="true">
            {#each p.byStatus as s (s.status)}
              <span
                style="flex:{s.count};background:{statusStyle(s.status).color}"
                title="{statusLabel(s.status)}: {s.count}"
              ></span>
            {/each}
          </div>
          <!-- Each status opens the Kanban filtered to it -->
          <ul class="status-legend">
            {#each p.byStatus as s (s.status)}
              <li>
                <button
                  class="status-legend-link"
                  data-status={s.status}
                  title="Show {statusLabel(s.status)} tasks on the Kanban"
                  onclick={() => showStatuses([s.status])}
                  ><span class="legend-dot" style="background:{statusStyle(s.status).color}"></span>{statusLabel(
                    s.status,
                  )}<strong>{s.count}</strong></button
                >
              </li>
            {/each}
          </ul>
          {#if ended.length}
            <!-- Finished without being done, apart from done (SR-2.8): won't do, obsolete, duplicate -->
            <p class="skipped-note" id="progress-not-done">
              Not done: {ended.map((e) => `${e.count} ${RESOLUTION_LABELS[e.resolution] ?? e.resolution}`).join(' · ')}
            </p>
          {/if}
        </div>
      </div>

      <div class="insights-section ring-group" id="progress-by-sprint">
        <div class="ring-group-head"><div class="menu-label">By sprint</div></div>
        {#if openRows.length}
          <div class="ring-grid">
            {#each openRows as r (r.key)}
              <button
                class="ring-cell ring-cell-button"
                data-sprint={r.key}
                title="{rowLabel(r.key)}: {r.done} of {r.total} done{r.skipped
                  ? `, ${r.skipped} skipped`
                  : ''} · open on the Progress tab"
                onclick={() => openSprint(r.key)}
              >
                <Ring
                  ratio={ratio(r)}
                  color={ringColor(r.key)}
                  label={rowLabel(r.key)}
                  skipped={r.skipped}
                  total={r.total}
                />
                <span class="ring-label">{rowLabel(r.key)}</span>
                <span class="ring-count">{r.done}/{r.total}</span>
              </button>
            {/each}
          </div>
        {/if}
        {#if closedRows.length}
          <div class="finished-chips" id="progress-closed-chips">
            <button class="finished-chip" title={closedTitle} onclick={() => openSprint(closedRows.at(-1)!.key)}
              >✓ {closedRows.length === 1 ? closedRows[0].key : `${closedRows.length} closed`}</button
            >
          </div>
        {/if}
      </div>

      <!-- By type: one stacked bar of all tasks, a segment per type, its done part solid and the rest faint -->
      <div class="insights-section" id="progress-by-type">
        <div class="menu-label">By type</div>
        <div class="status-bar" aria-hidden="true">
          {#each p.byType as s (s.key)}
            {@const color = typeStyle(s.key).color}
            <span
              class="type-bar-segment"
              style:flex={s.total}
              title="{capitalize(s.label)}: {s.done} of {s.total} done"
              style:background="color-mix(in srgb, {color} 28%, transparent)"
              ><span
                class="type-bar-done"
                style:width="{s.total ? (s.done / s.total) * 100 : 0}%"
                style:background={color}
              ></span></span
            >
          {/each}
        </div>
        <ul class="status-legend">
          {#each p.byType as s (s.key)}
            <li data-type={s.key}>
              <span class="legend-dot" style="background:{typeStyle(s.key).color}"></span>{capitalize(s.label)}
              <strong>{s.done}/{s.total}</strong>
            </li>
          {/each}
        </ul>
      </div>

      <div class="insights-section progress-go-to" id="progress-go-to">
        <div class="menu-label">Go to</div>
        <div class="go-to-links">
          <button
            class="go-to-link"
            data-go="progress"
            onclick={() => {
              dashboard.selectTab('progress');
              open = false;
            }}>Progress tab</button
          >
          {#each GO_TO as item (item.kind)}
            <button
              class="go-to-link"
              data-go={item.kind}
              disabled={countOfKind(item.kind) === 0}
              onclick={() => showStatuses(statusesOfKind(item.kind))}>{item.label} ({countOfKind(item.kind)})</button
            >
          {/each}
        </div>
      </div>
    </div>
  {/if}
</div>
