<script lang="ts">
  // The overview's Progress tab (T-329): each shown project's done of total, its active sprint and open releases (a row
  // opens the project's own Progress tab), then every project's tasks together in one burndown, and tasks done per
  // week (their sprints don't line up, so velocity is by week here)
  import { burndown } from '../lib/burndown';
  import { forecastSprints } from '../lib/forecast';
  import { formatDate } from '../lib/timeline';
  import type { ProjectEntry } from '../lib/load';
  import { mergedTasks, projectProgress, weeklyVelocity } from '../lib/overview-data';
  import { dueText, NO_SPRINTS } from '../lib/sprints';
  import { dashboard } from '../lib/state.svelte';
  import { DEFAULT_VOCABULARY } from '../lib/vocab';
  import BurndownChart from './BurndownChart.svelte';
  import ProjectLogo from './ProjectLogo.svelte';

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
  const velocity = $derived(weeklyVelocity(merged.tasks, now));
  const maxWeek = $derived(Math.max(1, ...velocity.weeks.map((w) => w.done)));
  const height = (n: number) => `${(n / maxWeek) * 100}%`;
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
      <h3 class="burndown-title">
        Velocity <span class="velocity-sub"
          >tasks done per week, the last {velocity.weeks.length} · average {average}</span
        >
      </h3>
      <div
        class="velocity-plot"
        role="img"
        aria-label="Tasks done per week: {velocity.weeks.map((w) => w.done).join(', ')}; average {average}"
      >
        <div class="velocity-average" style:bottom={height(velocity.average)} aria-hidden="true">
          <span>avg {average}</span>
        </div>
        {#each velocity.weeks as w (w.start)}
          <div class="velocity-column" title="The week from {formatDate(w.start)}: {w.done} done">
            <span class="velocity-bar" style:height={height(w.done)}></span>
            <span class="velocity-value" style:bottom={height(w.done)}>{w.done}</span>
          </div>
        {/each}
      </div>
      <div class="velocity-names" aria-hidden="true">
        {#each velocity.weeks as w (w.start)}<span>{formatDate(w.start)}</span>{/each}
      </div>
    </section>
  {/if}
</div>
