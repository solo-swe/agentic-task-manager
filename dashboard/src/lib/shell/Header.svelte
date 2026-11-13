<script lang="ts">
  // The title bar of the current page: a project's (counts, charts, filters), or a page without a project (the
  // overview of all projects, how to add a project, an unknown project; lib/routes.ts)
  import { dashboard } from '../state.svelte';
  import { needsYouItems } from '../waiting';
  import Filters from '../tasks/Filters.svelte';
  import NeedsYouMenu from './NeedsYouMenu.svelte';
  import ActivityWidget from '../progress/ActivityWidget.svelte';
  import OverviewActivity from '../overview/OverviewActivity.svelte';
  import ProgressWidget from '../progress/ProgressWidget.svelte';
  import TitleBar from './TitleBar.svelte';
  import AppTitleBar from './AppTitleBar.svelte';
  import SettingsMenu from './SettingsMenu.svelte';
  import WorkspaceSelector from './WorkspaceSelector.svelte';
  import { inApp } from '../api/http';

  const onProject = $derived(dashboard.route.page === 'project');
  // A project's page is titled by its name alone (T-271); while loading, the name it had (kept, or as the workspace
  // lists it: T-525, the title no longer flips to "Task Dashboard" and back on a switch), and without one "Task
  // Dashboard"
  const route = $derived(dashboard.route);
  const title = $derived(
    route.page === 'project'
      ? dashboard.loaded
        ? dashboard.projectName
        : (dashboard.nameOf(route.id) ?? 'Task Dashboard')
      : route.page === 'overview'
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
  // With you: only the active sprint's, ready to start (T-347), as atm's summary counts them
  const needYou = $derived(needsYouItems(dashboard.tasks, dashboard.vocab, { model: dashboard.sprints }));
  const present = $derived(dashboard.projects.filter((p) => !p.missing));
</script>

<!-- A project's search and Filters, and its Progress and Activity pills: the browser's title bar has them together, the
     app's in two places (search and Filters before the tabs) -->
{#snippet projectSearch()}
  <Filters />
{/snippet}
{#snippet projectPills()}
  <ProgressWidget />
  <ActivityWidget />
{/snippet}
{#snippet projectCharts()}
  {#if dashboard.loaded}
    <!-- Search and the Filters popup sit with the charts (T-323): no row of their own -->
    <div class="topbar-charts">
      {@render projectSearch()}
      {@render projectPills()}
    </div>
  {/if}
{/snippet}
{#snippet overviewCharts()}
  {#if dashboard.route.page === 'overview'}
    <!-- Every project's activity together (T-328) -->
    <div class="topbar-charts">
      <OverviewActivity />
    </div>
  {/if}
{/snippet}

<!-- The atm app's own title bar (T-513): the window's, its tabs; at its right end the workspace picker, the overview's
     activity and the ⋯; inside the tab, the subtitle line, a project's search and Filters before it (Hakan) and its
     Progress and Activity at its right end: they are the tab's project's (T-530) -->
{#snippet tabSearch()}
  {#if dashboard.loaded}{@render projectSearch()}{/if}
{/snippet}
{#snippet tabPills()}
  {#if dashboard.loaded}{@render projectPills()}{/if}
{/snippet}
{#snippet appEnd()}
  <WorkspaceSelector />
  {#if !onProject}{@render overviewCharts()}{/if}
  <SettingsMenu />
{/snippet}
{#if inApp}<AppTitleBar {title} end={appEnd} />{/if}
{#if onProject}
  <TitleBar {title} inTab={inApp} charts={projectCharts} lead={tabSearch} trail={inApp ? tabPills : undefined}>
    {#snippet subtitle()}
      {#if dashboard.loaded}
        <!-- One text node when nothing is archived, as before the archive existed (screenshots compare glyphs exactly) -->
        {#if dashboard.archivedCount}
          <!-- The archive one click away (T-354, found in T-277): it toggles Include archived -->
          {counts} ·
          <button
            class="archived-toggle"
            id="archived-count"
            aria-pressed={dashboard.includeArchived}
            title={dashboard.includeArchived
              ? 'Hide the archived tasks again'
              : 'Show the archived tasks too (search, List, Graph, Agents)'}
            onclick={() => dashboard.setIncludeArchived(!dashboard.includeArchived)}
            >{dashboard.includeArchived ? 'showing ' : ''}{dashboard.archivedCount} archived</button
          > — loaded from tasks.json
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
  </TitleBar>
{:else}
  <TitleBar {title} selector={dashboard.projects.length > 0} inTab={inApp} charts={overviewCharts}>
    {#snippet subtitle()}
      {#if !dashboard.projectsLoaded}
        Loading projects…
      {:else}
        {present.length} project{present.length === 1 ? '' : 's'}
      {/if}
    {/snippet}
  </TitleBar>
{/if}
