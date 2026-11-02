<script lang="ts">
  // The title bar of the current page: a project's (counts, charts, filters), or a page without a project (the
  // overview of all projects, how to add a project, an unknown project; lib/routes.ts)
  import { dashboard } from '../lib/state.svelte';
  import { needsYouItems } from '../lib/waiting';
  import Filters from './Filters.svelte';
  import NeedsYouMenu from './NeedsYouMenu.svelte';
  import ActivityWidget from './ActivityWidget.svelte';
  import OverviewActivity from './OverviewActivity.svelte';
  import ProgressWidget from './ProgressWidget.svelte';
  import TitleBar from './TitleBar.svelte';

  const onProject = $derived(dashboard.route.page === 'project');
  // A project's page is titled by its name alone (T-271); while loading, and without a name, "Task Dashboard"
  const title = $derived(
    onProject
      ? dashboard.loaded
        ? dashboard.projectName
        : 'Task Dashboard'
      : dashboard.route.page === 'overview'
        ? 'Projects Overview'
        : 'Task Dashboard',
  );
  // The sprints the tasks are in (an unmigrated project's phases, read as sprints) and the backlog (SR-7.6)
  const inSprint = $derived(dashboard.tasks.map((t) => dashboard.sprints.sprintOf(t)));
  const sprintCount = $derived(new Set(inSprint.filter(Boolean)).size);
  const backlogCount = $derived(inSprint.filter((s) => !s).length);
  const counts = $derived(
    `${dashboard.tasks.length} tasks` +
      (sprintCount ? ` across ${sprintCount} sprint${sprintCount !== 1 ? 's' : ''}` : '') +
      (backlogCount && sprintCount ? ` · ${backlogCount} in the backlog` : ''),
  );
  // Tasks that need the user: in review, or blocked on a question for the user (D-3.3)
  const needYou = $derived(needsYouItems(dashboard.tasks, dashboard.vocab));
  const present = $derived(dashboard.projects.filter((p) => !p.missing));
</script>

{#if onProject}
  <TitleBar {title}>
    {#snippet subtitle()}
      {#if dashboard.loaded}
        <!-- One text node when nothing is archived, as before the archive existed (screenshots compare glyphs exactly) -->
        {#if dashboard.archivedCount}
          {counts} · <span id="archived-count">{dashboard.archivedCount} archived</span> — loaded from tasks.json
        {:else}
          {counts} — loaded from tasks.json
        {/if}
        <NeedsYouMenu
          items={needYou}
          onopen={(id) => dashboard.showDetail(id)}
          chipId="attention-count"
          menuId="needs-you-menu"
        />
      {:else}
        Loading tasks.json…
      {/if}
    {/snippet}
    {#snippet charts()}
      {#if dashboard.loaded}
        <!-- Search and the Filters popup sit with the charts (T-323): no row of their own -->
        <div class="topbar-charts">
          <Filters />
          <ProgressWidget />
          <ActivityWidget />
        </div>
      {/if}
    {/snippet}
  </TitleBar>
{:else}
  <TitleBar {title} selector={dashboard.projects.length > 0}>
    {#snippet subtitle()}
      {#if !dashboard.projectsLoaded}
        Loading projects…
      {:else}
        {present.length} project{present.length === 1 ? '' : 's'}
      {/if}
    {/snippet}
    {#snippet charts()}
      {#if dashboard.route.page === 'overview'}
        <!-- Every project's activity together (T-328) -->
        <div class="topbar-charts">
          <OverviewActivity />
        </div>
      {/if}
    {/snippet}
  </TitleBar>
{/if}
