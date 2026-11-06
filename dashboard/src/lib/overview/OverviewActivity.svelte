<script lang="ts">
  // The overview's Activity chart and popup (T-328): every shown project's tasks together, as one project's would be,
  // stacked and coloured by project; a task opens in its project
  import { mergedTasks, projectOfWorker, projectTasks } from '../overview-data';
  import { isActive } from '../projects';
  import { dashboard } from '../state.svelte';
  import { phaseColor } from '../theme';
  import ActivityWidget from '../progress/ActivityWidget.svelte';

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
  <ActivityWidget {source} view={dashboard.overviewActivity} onopen={open} />
{/if}
