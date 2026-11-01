<script lang="ts">
  import { BURNDOWN_RANGES, burndown, rangeSprint, rangeStart } from '../lib/burndown';
  import { ACTIVE, BACKLOG, FILTER_GROUPS } from '../lib/filters';
  import { forecastSprints, withWholeSprintEstimates } from '../lib/forecast';
  import { dashboard } from '../lib/state.svelte';
  import { timeUse } from '../lib/timeuse';
  import BurndownChart from './BurndownChart.svelte';
  import ComponentTable from './ComponentTable.svelte';
  import InfoTip from './InfoTip.svelte';
  import ReleaseView from './ReleaseView.svelte';
  import SprintTable from './SprintTable.svelte';
  import TimeSplitBar from './TimeSplitBar.svelte';
  import VelocityChart from './VelocityChart.svelte';

  // Progress is history: archived tasks count too, so the tab loads the archive once shown (D-5.2). It stays mounted
  // while hidden, so only when it is the selected tab.
  $effect(() => {
    if (dashboard.tab === 'progress' && dashboard.archivedCount > 0) dashboard.wantArchive();
  });
  const tasks = $derived(dashboard.historyFiltered);

  // Recomputed whenever tasks or filters change (auto-refresh reloads keep it current)
  const now = $derived.by(() => (dashboard.history, tasks, Date.now()));
  // The search and filters pick a slice of the tasks (the status and stage filters don't apply here: T-239). The slice
  // gets its own counts and lines, but when a sprint finishes is the whole sprint's: the loop works through all of it.
  // Nor the Sprint filter, which scopes the board views: this tab shows every sprint and focuses one itself (T-323)
  const applies = (g: (typeof FILTER_GROUPS)[number]) => g !== 'sprint' && g !== 'status' && g !== 'stage';
  const filtered = $derived(
    dashboard.query.trim() !== '' || FILTER_GROUPS.some((g) => applies(g) && dashboard.filters[g].size > 0),
  );
  const ignored = $derived((['status', 'stage'] as const).filter((g) => dashboard.filters[g].size > 0));
  const applied = $derived(
    [
      ...FILTER_GROUPS.filter((g) => applies(g) && dashboard.filters[g].size > 0).map(
        (g) => `${g === 'agent' ? 'started by' : g} ${[...dashboard.filters[g]].join(', ')}`,
      ),
      ...(dashboard.query.trim() ? [`“${dashboard.query.trim()}”`] : []),
    ].join(' · '),
  );
  const model = $derived(dashboard.sprints);
  const whole = $derived(forecastSprints(dashboard.history, now, dashboard.history, dashboard.vocab, model));
  const forecast = $derived(
    filtered
      ? withWholeSprintEstimates(forecastSprints(tasks, now, dashboard.history, dashboard.vocab, model), whole)
      : whole,
  );
  // Clicking a sprint row focuses the burndown on that sprint (FR-4.6); a filter that hides it resets the focus
  // The focused sprint is part of the project's saved view (dashboard state)
  // By default the active sprint, whichever it is (ACTIVE, T-323); all sprints while none is active
  const selected = $derived(
    dashboard.progressSprint === ACTIVE
      ? (model.sprints.find((s) => s.state === 'active')?.id ?? null)
      : dashboard.progressSprint,
  );
  const focus = $derived(selected !== null && forecast.sprints.some((s) => s.key === selected) ? selected : undefined);
  const label = (key: string) => (key === BACKLOG ? 'the backlog' : key);
  // The time range (saved with the view): the focused sprint's, or the one the loop works on, by default (T-240)
  const range = $derived(dashboard.progressRange);
  const from = $derived(rangeStart(range, dashboard.history, whole, now, focus, model));
  const current = $derived(rangeSprint(whole, focus));
  const RANGE_LABELS = $derived({
    sprint: current === undefined ? 'This sprint' : `Since ${label(current)} started`,
    '14d': 'Last 14 days',
    all: 'All time',
  });
  const chart = $derived(burndown(tasks, forecast, now, focus, dashboard.vocab, from, model));
  // Agent time, waiting on you and idle over the burndown's range, for the matching tasks (T-242; shared with the
  // Activity popup, timeuse.ts)
  const use = $derived(
    timeUse(tasks, from === undefined ? null : { start: from, end: chart.end ?? now }, now, dashboard.vocab),
  );

  // The By component table: the focused sprint's tasks, else all of them (T-323)
  const focusTasks = $derived(
    focus === undefined ? tasks : tasks.filter((t) => (model.sprintOf(t) ?? BACKLOG) === focus),
  );
  const focusLabel = $derived(
    (focus === undefined ? 'All sprints' : focus === BACKLOG ? 'Backlog' : focus) +
      (filtered ? ' · matching tasks' : ''),
  );

  const toggle = (key: string) => (dashboard.progressSprint = focus === key ? null : key);
</script>

<div class="progress-view" id="progress">
  {#if filtered || ignored.length}
    <div class="progress-filter-banner" id="progress-filter-banner" role="status">
      {#if filtered}
        <span
          >Showing <strong>{tasks.length}</strong> of {dashboard.history.length} tasks: {applied}. Counts, bars and the
          burndown are the matching tasks'; when a sprint finishes is the whole sprint's.</span
        >
      {/if}
      {#if ignored.length}
        <span id="progress-filter-ignored"
          >{ignored.map((g) => (g === 'status' ? 'Status' : 'Stage')).join(' and ')} filter not applied here: progress is
          the split by status.</span
        >
      {/if}
      <button class="link-button" id="progress-clear-filters" onclick={() => dashboard.clearFilters()}
        >Clear filters</button
      >
    </div>
  {/if}
  {#if tasks.length === 0}
    <div class="progress-empty">No tasks match current filters</div>
  {:else}
    <div class="burndown-scope-row">
      <h3 class="burndown-title">
        Burndown ·
        <span class="burndown-focus" id="burndown-focus"
          >{focus === undefined ? 'All sprints' : focus === BACKLOG ? 'Backlog' : focus}{filtered
            ? ' · matching tasks'
            : ''}</span
        >
        <InfoTip label="the burndown" id="burndown-info">
          <p>
            <strong>Scope</strong> is how many tasks existed at each moment (skipped ones left out);
            <strong>Remaining</strong> is how many of them weren't done yet. Scope steps up when tasks are added; Remaining
            steps down as tasks are finished. A closed sprint's burndown ends where it closed; an open one's due date is marked.
          </p>
          <p>
            The dashed line is the <strong>forecast</strong>: the agents' pace (the middle of their last 10 tasks, the
            shaded fan its usual spread) times the work left, sprint by sprint. Tasks in review or blocked wait on
            people, not agents, so the forecast ends at their number, not at zero.
          </p>
          <p>
            It assumes <strong>one agent, one task at a time</strong>: parallel agents finish sooner. Tasks labelled
            <code>interactive</code> are left out of the pace, as they include time spent waiting on you. Filters choose the
            lines, not the pace.
          </p>
        </InfoTip>
      </h3>
      {#if focus !== undefined}
        <button class="burndown-all" onclick={() => (dashboard.progressSprint = null)}>Show all sprints</button>
      {/if}
      <span class="activity-range burndown-range" id="burndown-range" role="group" aria-label="Time range">
        {#each BURNDOWN_RANGES as option (option)}
          <button
            class:active={range === option}
            aria-pressed={range === option}
            data-range={option}
            onclick={() => (dashboard.progressRange = option)}>{RANGE_LABELS[option]}</button
          >
        {/each}
      </span>
    </div>
    {#if chart.untimed}
      <p class="progress-empty" id="burndown-empty">
        No timeline yet: tasks get times when agents start and finish them.
      </p>
    {:else}
      <BurndownChart data={chart} {now} />
    {/if}
    <SprintTable {forecast} {now} selected={focus} onselect={toggle} />
    <ComponentTable tasks={focusTasks} scope={focusLabel} />
    <VelocityChart />
    <ReleaseView />
    {#if use}
      <section class="progress-time-use" aria-label="Where the time went">
        <TimeSplitBar {use} id="progress-time-use" />
      </section>
    {/if}
  {/if}
</div>
