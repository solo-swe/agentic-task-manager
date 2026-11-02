<script lang="ts">
  // The overview's Activity chart and popup (T-328): every shown project's tasks together, as one project's would be,
  // with each project's count for the chosen period; a task opens in its project
  import { completionsByProject, mergedTasks } from '../lib/overview-data';
  import { isActive } from '../lib/projects';
  import { dashboard } from '../lib/state.svelte';
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

  function open(key: string) {
    const at = merged.origin.get(key);
    if (at) dashboard.openProject(at.project.id, { task: at.id });
  }
</script>

{#if sources.length}
  <ActivityWidget
    source={merged}
    view={dashboard.overviewActivity}
    onopen={open}
    byProject={(period) =>
      completionsByProject(sources, period).map(({ project, completed }) => ({
        label: project.name,
        count: completed,
        open: () => dashboard.openProject(project.id),
      }))}
  />
{/if}
