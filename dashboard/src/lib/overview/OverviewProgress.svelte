<script lang="ts">
  // The overview's Progress tab (T-329): each shown project's done of total, its active sprint and open releases (a row
  // opens the project's own Progress tab), then every project's tasks together in one burndown, and tasks done per day,
  // week or month, stacked by project (their sprints don't line up, so velocity is by period here)
  import { burndown } from '../burndown';
  import { forecastSprints } from '../forecast';
  import { formatDate } from '../timeline';
  import type { ProjectEntry } from '../load';
  import { mergedTasks, projectProgress, velocityByPeriod, VELOCITY_UNITS, type VelocityUnit } from '../overview-data';
  import { phaseColor } from '../theme';
  import { dueText, NO_SPRINTS } from '../sprints';
  import { dashboard } from '../state.svelte';
  import { DEFAULT_VOCABULARY } from '../vocab';
  import BurndownChart from '../progress/BurndownChart.svelte';
  import ProjectLogo from '../ui/ProjectLogo.svelte';

  let { projects }: { projects: readonly ProjectEntry[] } = $props();

  const shownIds = $derived(new Set(projects.map((p) => p.id)));
  const sources = $derived(dashboard.projectSources.filter((s) => shownIds.has(s.project.id)));
  const now = $derived(dashboard.loadedAt);
  const rows = $derived(sources.map((s) => projectProgress(s, now)));

  // Every project's tasks as one project's, without sprints: the burndown of all of them, last 14 days or all time
  const merged = $derived(mergedTasks(sources));
  let range = $state<'14d' | 'all'>('14d');
  const forecast = $derived(forecastSprints(merged.tasks, now, merged.tasks, DEFAULT_VOCABULARY, NO_SPRINTS));
  const chart = $derived(
    burndown(
      merged.tasks,
      forecast,
      now,
      undefined,
      DEFAULT_VOCABULARY,
      range === '14d' ? now - 14 * 86_400_000 : undefined,
      NO_SPRINTS,
    ),
  );
  // Tasks done per day, week or month (the projects' sprints don't line up), stacked by project
  let unit = $state<VelocityUnit>('week');
  const velocity = $derived(velocityByPeriod(sources, now, unit));
  const maxBar = $derived(Math.max(1, ...velocity.bars.map((b) => b.total)));
  const height = (n: number) => `${(n / maxBar) * 100}%`;
  const average = $derived(Math.round(velocity.average * 10) / 10);
  const pct = (done: number, total: number) => (total ? Math.round((100 * done) / total) : 0);
</script>

<div class="overview-progress" id="overview-progress">
  {#if !dashboard.projectSourcesLoaded}
    <p class="progress-empty" id="overview-progress-loading">Loading every project's tasks…</p>
  {:else if !rows.length}
    <p class="progress-empty">No projects to show</p>
  {:else}
    <section class="overview-progress-projects" aria-label="Each project">
      {#each rows as row (row.project.id)}
        <button
          class="overview-progress-row"
          data-id={row.project.id}
          title="Open {row.project.name}'s Progress tab"
          onclick={() => dashboard.openProject(row.project.id, { view: 'progress' })}
        >
          <span class="overview-progress-name"
            ><ProjectLogo project={row.project} size={20} /><span>{row.project.name}</span></span
          >
          <span class="overview-progress-total">
            <span class="overview-progress-bar" aria-hidden="true"
              ><span style:width="{pct(row.done, row.total)}%"></span></span
            >
            <span class="overview-progress-count">{row.done} / {row.total} · {pct(row.done, row.total)}%</span>
          </span>
          <span class="overview-progress-sprint" title={row.sprint?.sprint.goal ?? null}>
            {#if row.sprint}
              <span class="project-card-label">Sprint</span>
              <strong>{row.sprint.sprint.id}</strong>
              <span>{row.sprint.done} / {row.sprint.total}</span>
              {#if row.sprint.dueIn !== undefined}<span class="muted" class:overdue={row.sprint.dueIn < 0}
                  >· {dueText(row.sprint.dueIn)}</span
                >{/if}
            {:else}
              <span class="muted">No active sprint</span>
            {/if}
          </span>
          <span class="overview-progress-releases">
            {#each row.releases as r (r.id)}
              <span class="overview-progress-release" title={r.name ?? r.id}>{r.id} {r.done}/{r.total}</span>
            {:else}
              <span class="muted">No open release</span>
            {/each}
          </span>
        </button>
      {/each}
    </section>

    <div class="burndown-scope-row">
      <h3 class="burndown-title">Burndown · <span class="burndown-focus">every project</span></h3>
      <span class="activity-range burndown-range" id="overview-burndown-range" role="group" aria-label="Time range">
        {#each [['14d', 'Last 14 days'], ['all', 'All time']] as const as [option, label] (option)}
          <button
            class:active={range === option}
            aria-pressed={range === option}
            data-range={option}
            onclick={() => (range = option)}>{label}</button
          >
        {/each}
      </span>
    </div>
    {#if chart.untimed || !chart.points.length}
      <p class="progress-empty">No timeline yet: tasks get times when agents start and finish them.</p>
    {:else}
      <BurndownChart data={chart} {now} />
    {/if}

    <section class="velocity" id="overview-velocity" aria-label="Velocity">
      <div class="burndown-scope-row">
        <h3 class="burndown-title">
          Velocity <span class="velocity-sub"
            >tasks done per {unit}, the last {velocity.bars.length} · average {average}</span
          >
        </h3>
        <span class="activity-range burndown-range" id="overview-velocity-unit" role="group" aria-label="Per">
          {#each VELOCITY_UNITS as u (u.unit)}
            <button
              class:active={unit === u.unit}
              aria-pressed={unit === u.unit}
              data-unit={u.unit}
              onclick={() => (unit = u.unit)}>{u.label}</button
            >
          {/each}
        </span>
      </div>
      <!-- A colour per project, stacked in the projects' order (T-329 review) -->
      <div class="velocity-legend" aria-hidden="true">
        {#each sources as s, i (s.project.id)}
          <span><span class="legend-swatch" style:background={phaseColor(i)}></span>{s.project.name}</span>
        {/each}
      </div>
      <div
        class="velocity-plot"
        role="img"
        aria-label="Tasks done per {unit}: {velocity.bars.map((b) => b.total).join(', ')}; average {average}"
      >
        <div class="velocity-average" style:bottom={height(velocity.average)} aria-hidden="true">
          <span>avg {average}</span>
        </div>
        {#each velocity.bars as b (b.start)}
          <div
            class="velocity-column"
            title="{unit === 'day'
              ? formatDate(b.start + 1)
              : `From ${formatDate(b.start)}`}: {b.total} done{b.byProject
              .map((n, i) => (n ? `\n${sources[i].project.name}: ${n}` : ''))
              .join('')}"
          >
            <span class="velocity-stack" style:height={height(b.total)}>
              {#each b.byProject as n, i (i)}
                {#if n}<span class="velocity-segment" style:flex-grow={n} style:background={phaseColor(i)}></span>{/if}
              {/each}
            </span>
            <span class="velocity-value" style:bottom={height(b.total)}>{b.total}</span>
          </div>
        {/each}
      </div>
      <div class="velocity-names" aria-hidden="true">
        {#each velocity.bars as b (b.start)}<span>{formatDate(b.start + (unit === 'day' ? 1 : 0))}</span>{/each}
      </div>
    </section>
  {/if}
</div>
