<script lang="ts">
  // The overview's Activity chart and popup (T-328): every shown project's tasks together, as one project's would be,
  // with each project's count for the chosen period; a task opens in its project
  import { completionsByProject, mergedTasks, projectOfWorker, projectTasks } from '../lib/overview-data';
  import { isActive } from '../lib/projects';
  import { dashboard } from '../lib/state.svelte';
  import { phaseColor } from '../lib/theme';
  import { timeUse } from '../lib/timeuse';
  import ActivityWidget from './ActivityWidget.svelte';

  // The projects the overview shows ("Hide inactive projects" applies here too)
  const shownIds = $derived(
    new Set(
      dashboard.projects
        .filter((p) => !dashboard.overviewActiveOnly || isActive(p, dashboard.loadedAt))
        .map((p) => p.id),
    ),
  );
  const sources = $derived(dashboard.projectSources.filter((s) => shownIds.has(s.project.id)));
  const merged = $derived(mergedTasks(sources));
  // A colour per project, in the sources' order: the same as the Progress tab's velocity (T-328 review)
  const colorOf = $derived(new Map(sources.map((s, i) => [s.project.id, phaseColor(i)])));
  const source = $derived({
    ...merged,
    byProject: projectTasks(merged),
    projects: sources.map((s) => ({ name: s.project.name, color: colorOf.get(s.project.id)! })),
    of: projectOfWorker,
  });

  function open(key: string) {
    const at = merged.origin.get(key);
    if (at) dashboard.openProject(at.project.id, { task: at.id });
  }
</script>

{#if sources.length}
  <ActivityWidget
    {source}
    view={dashboard.overviewActivity}
    onopen={open}
    byProject={(period) =>
      completionsByProject(sources, period).map(({ project, completed }) => {
        const own = sources.find((s) => s.project.id === project.id)!;
        return {
          label: project.name,
          color: colorOf.get(project.id)!,
          count: completed,
          use: timeUse(own.tasks, period, dashboard.loadedAt, own.vocab),
          open: () => dashboard.openProject(project.id),
        };
      })}
  />
{/if}
