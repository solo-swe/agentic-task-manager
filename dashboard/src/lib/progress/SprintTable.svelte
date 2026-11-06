<script lang="ts">
  import { BACKLOG } from '../filters';
  import { type Forecast, type SprintForecast } from '../forecast';
  import { formatMinutes, formatMinutesRange, formatWhen } from '../format';
  import { dashboard } from '../state.svelte';
  import { phaseColor } from '../theme';
  import InfoTip from '../ui/InfoTip.svelte';

  let {
    forecast,
    now,
    selected,
    onselect,
  }: { forecast: Forecast; now: number; selected?: string; onselect?: (key: string) => void } = $props();

  function onRowKeydown(e: KeyboardEvent, key: string) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      onselect?.(key);
    }
  }

  // Skipped tasks count as neither done nor left to do: a sprint of done and skipped tasks is 100%
  const pct = (row: { done: number; total: number; skipped: number }) =>
    row.total ? Math.round((row.done / row.total) * 100) : row.skipped ? 100 : 0;

  // Closed sprints fold into one row (T-241): on a long project they are most of the table and say nothing new. One
  // closed sprint is shown as it is. The row opens to show them; the choice lasts while the tab is open.
  const isClosed = (row: SprintForecast) => row.sprint?.state === 'closed';
  const closed = $derived(forecast.sprints.filter(isClosed));
  const folded = $derived(closed.length >= 2);
  let expanded = $state(false);
  const closedTasks = $derived(closed.reduce((n, s) => n + s.total + s.skipped, 0));
  const lastClosed = $derived(
    Math.max(...closed.map((s) => Date.parse(s.sprint?.end ?? '') || (s.completedAt ?? -Infinity))),
  );
  const closedLabel = $derived(
    `✓ ${closed.length} closed sprints · ${closedTasks} tasks` +
      (Number.isFinite(lastClosed) ? ` · last closed ${formatWhen(lastClosed, now)}` : ''),
  );
  const rows = $derived(folded && !expanded ? forecast.sprints.filter((s) => !isClosed(s)) : forecast.sprints);
  // The first sprint with an estimate is the loop's next: the ones after it include its work, and the work before them
  const firstEstimated = $derived(
    forecast.sprints
      .filter((s) => s.estimate)
      .sort((a, b) => a.estimate!.minutes.likely - b.estimate!.minutes.likely)[0]?.key,
  );
  // A sprint keeps its colour wherever it is: by its place in the project's list
  const color = (key: string) => {
    const index = dashboard.sprints.sprints.findIndex((s) => s.id === key);
    return index < 0 ? 'var(--text-muted)' : phaseColor(index);
  };
  const closedAt = (row: SprintForecast) => Date.parse(row.sprint?.end ?? '') || row.completedAt;
</script>

<p class="progress-basis" id="progress-basis">
  {#if forecast.pace}
    <!-- One short line; the burndown's ⓘ explains what the pace is and what it assumes (T-240) -->
    <span title="{formatMinutesRange(forecast.pace.low, forecast.pace.high)} for the middle half of those tasks"
      >Pace: ~{formatMinutes(forecast.pace.likely)} per task (last {forecast.pace.basedOn} tasks, one agent at a time).</span
    >
    The active sprint first, then the planned ones, then the backlog, like <code>/atm:loop-tasks --all</code>.
  {:else}
    Not enough history to estimate yet: estimates need at least 3 completed tasks with timestamps.
  {/if}
  {#if onselect}<span class="phase-table-hint">Click a sprint to see its burndown.</span>{/if}
</p>

<table class="phase-table sprint-table" id="sprint-table">
  <thead>
    <tr>
      <th>Sprint</th>
      <th>Done</th>
      <th class="phase-table-progress">Progress</th>
      <th>Remaining</th>
      <th>
        Finished by
        <InfoTip label="the sprint estimates" id="sprint-table-info">
          <p>
            <strong>Finished by</strong> is when the sprint would be done if an agent loop keeps running from now. The loop
            takes the active sprint first, then the planned ones in order, then the backlog, so a sprint's estimate includes
            the work left in the ones before it.
          </p>
          <p>
            It counts only work agents can do: tasks <strong>blocked</strong> or <strong>in review</strong> wait on people
            and are left out (shown next to Remaining). It is for the whole sprint, whatever the filters. A closed sprint
            shows when it closed and what its report says: tasks carried over and tasks added mid-sprint.
          </p>
        </InfoTip>
      </th>
    </tr>
  </thead>
  <tbody>
    {#if folded}
      <tr class="phase-finished-group" id="sprint-closed">
        <td colspan="5">
          <button
            class="phase-finished-toggle"
            id="sprint-closed-toggle"
            aria-expanded={expanded}
            onclick={() => (expanded = !expanded)}
          >
            <span>{closedLabel}</span>
            <span class="phase-finished-action">{expanded ? 'Hide' : 'Show'}</span>
          </button>
        </td>
      </tr>
    {/if}
    {#each rows as row (row.key)}
      {@render sprintRow(row)}
    {/each}
  </tbody>
</table>

{#snippet sprintRow(row: SprintForecast)}
  {@const c = color(row.key)}
  {@const report = row.sprint?.report}
  <tr
    data-sprint={row.key}
    data-state={row.state}
    data-sprint-state={row.sprint?.state ?? 'backlog'}
    class:selected={selected === row.key}
    class:selectable={!!onselect}
    class:not-started={row.state === 'not_started'}
    tabindex={onselect ? 0 : undefined}
    aria-selected={onselect ? selected === row.key : undefined}
    onclick={() => onselect?.(row.key)}
    onkeydown={(e) => onRowKeydown(e, row.key)}
  >
    <td class="phase-cell"
      ><span class="phase-badge" style="background:color-mix(in srgb, {c} 12.5%, transparent);color:{c}"
        >{isClosed(row) ? '✓ ' : ''}{row.key === BACKLOG ? 'Backlog' : row.key}</span
      >{#if row.sprint && row.sprint.state !== 'closed'}<span class="phase-name sprint-state-name"
          >{row.sprint.state}{row.sprint.due ? ` · due ${row.sprint.due}` : ''}</span
        >{/if}</td
    >
    <td class="num"
      >{row.done}/{row.total}{#if row.skipped}<span class="skipped-note" title="Skipped: not counted as done or to do"
          >&nbsp;· {row.skipped} skipped</span
        >{/if}</td
    >
    <td class="phase-table-progress">
      <div class="progress-bar">
        <div class="progress-fill" style:width="{pct(row)}%" style:background={c}></div>
      </div>
    </td>
    <td class="num">
      {row.remaining}{#if row.blocked}<span class="phase-blocked" title="Blocked tasks are not included in the estimate"
          >{row.blocked} blocked</span
        >{/if}{#if row.inReview}<span
          class="phase-blocked phase-in-review"
          title="Tasks waiting for a review need no agent work and are not included in the estimate"
          >{row.inReview} in review</span
        >{/if}
    </td>
    <td
      class="phase-estimate"
      title={row.estimate
        ? `${formatMinutes(row.estimate.minutes.likely)} of agent work (${formatMinutesRange(row.estimate.minutes.low, row.estimate.minutes.high)}), for the whole sprint whatever the filters`
        : undefined}
    >
      {#if isClosed(row)}
        Closed{#if closedAt(row)}&nbsp;{formatWhen(closedAt(row)!, now)}{/if}
        {#if report?.carried?.length || report?.added?.length}
          <div class="estimate-calendar sprint-report">
            {[
              report.carried?.length ? `${report.carried.length} carried${report.to ? ` to ${report.to}` : ''}` : '',
              report.added?.length ? `${report.added.length} added mid-sprint` : '',
            ]
              .filter(Boolean)
              .join(' · ')}
          </div>
        {/if}
      {:else if row.state === 'done'}
        Done{#if row.completedAt}&nbsp;{formatWhen(row.completedAt, now)}{/if}
      {:else if row.estimate}
        <div class="estimate-main">≈ {formatWhen(row.estimate.finishAt.likely, now)}</div>
        <div class="estimate-calendar">
          {formatMinutes(row.estimate.minutes.likely)} of agent work{row.key === firstEstimated
            ? ''
            : ', including earlier sprints'}
        </div>
      {:else if row.remaining === 0 && row.blocked}
        Blocked
      {:else if row.remaining === 0 && row.inReview}
        Waiting for review
      {:else}
        <span class="estimate-none">Not enough history to estimate</span>
      {/if}
    </td>
  </tr>
{/snippet}
