<script lang="ts">
  // The overview's Agents tab (T-333): every shown project's agents on one timeline, like a project's Agents tab; a
  // lane names the projects its agent worked in, a bar opens its task in its project
  import type { ProjectEntry } from '../lib/load';
  import { combinedTimeline } from '../lib/overview-data';
  import { dashboard } from '../lib/state.svelte';
  import AgentsTab from './AgentsTab.svelte';

  let { projects }: { projects: readonly ProjectEntry[] } = $props();

  // The projects the page shows ("Active only" applies here too)
  const shownIds = $derived(new Set(projects.map((p) => p.id)));
  const sources = $derived(dashboard.projectSources.filter((s) => shownIds.has(s.project.id)));
  const timeline = $derived(combinedTimeline(sources, dashboard.loadedAt));
</script>

<div class="overview-agents" id="overview-agents">
  {#if !dashboard.projectSourcesLoaded}
    <p class="agents-empty" id="overview-agents-loading">Loading every project's tasks…</p>
  {:else}
    <AgentsTab
      {timeline}
      view={dashboard.overviewAgents}
      onopen={(bar) => bar.project && dashboard.openProject(bar.project.id, { task: bar.task.id })}
    />
  {/if}
</div>
