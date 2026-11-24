<script lang="ts">
  // All projects at a glance (docs/multi-project-dashboard-proposal.md 3.5): a card per project, those that need you
  // first; or, on the Omniboard tab, every project's tasks in one board (T-259, T-266); or every project's agents
  // (T-333). "Hide inactive projects" leaves out projects with no open work and nothing done lately, on every tab; it
  // shows only when there is such a project.
  import { BOARD_SCOPES, withoutActiveSprint } from '../allboard';
  import { ACTIVE_DAYS, isActive, orderProjects } from '../projects';
  import type { Item } from '../address';
  import type { OverviewTab } from '../routes';
  import { dashboard } from '../state.svelte';
  import { ALL_ITEM_VIEWS, itemOfOverviewTab } from '../tabs';
  import AllProjectsBoard from './AllProjectsBoard.svelte';
  import AllProjectsList from './AllProjectsList.svelte';
  import Icon, { type IconName } from '../ui/Icon.svelte';
  import ItemTabs from '../shell/ItemTabs.svelte';
  import SubTabs from '../shell/SubTabs.svelte';
  import InfoTip from '../ui/InfoTip.svelte';
  import OverviewAgents from './OverviewAgents.svelte';
  import OverviewProgress from './OverviewProgress.svelte';
  import OverviewReviews from './OverviewReviews.svelte';
  import ProjectCard from './ProjectCard.svelte';

  const ordered = $derived(orderProjects(dashboard.projects));
  const active = $derived(ordered.filter((p) => isActive(p, dashboard.loadedAt)));
  const shown = $derived(dashboard.overviewActiveOnly ? active : ordered);
  // How many projects the option hides (or would): it shows only when there is one
  const inactive = $derived(ordered.length - active.length);
  const tab = $derived(dashboard.overviewTab);
  const noSprint = $derived(withoutActiveSprint(shown, dashboard.boardScope));
  // The page's items (ATM-752): Overview, Tasks, Code Reviews, Analysis; Tasks' and Analysis' views as sub-tabs
  const item = $derived(itemOfOverviewTab(tab));
  const items = Object.keys(ALL_ITEM_VIEWS) as Item[];

  // The board's id stays "board", so /board links and the saved tab still work (T-266: it was the Omniboard)
  const LABELS: Record<OverviewTab, string> = {
    projects: 'Projects',
    board: 'Board',
    list: 'List',
    progress: 'Progress',
    agents: 'Agents',
    reviews: 'Code Reviews',
  };
  const ICONS = {
    projects: 'folder',
    board: 'square-kanban',
    list: 'list-todo',
    progress: 'trending-up',
    agents: 'bot',
    reviews: 'file-diff',
  } as const satisfies Record<OverviewTab, IconName>;
  const subViews = $derived((ALL_ITEM_VIEWS[item] ?? []).map((id) => ({ id, label: LABELS[id], icon: ICONS[id] })));
  const onTasks = $derived(tab === 'board' || tab === 'list');
  const selectItem = (next: Item) => next !== item && dashboard.selectOverviewTab(ALL_ITEM_VIEWS[next]![0]);
</script>

<!-- On the board, the page doesn't scroll: the board takes the window's height and its columns scroll (T-265) -->
<section class="projects-overview" class:on-board={tab === 'board'} id="projects-overview">
  <ItemTabs {items} current={item} onselect={selectItem} label="All projects" />
  {#if subViews.length > 1}
    <SubTabs
      views={subViews}
      current={tab}
      onselect={(next) => dashboard.selectOverviewTab(next)}
      idPrefix="overview-tab-"
      panel={() => 'overview-panel'}
      label={item === 'tasks' ? 'Tasks' : 'Analysis'}
    />
  {/if}
  <div class="projects-toolbar">
    {#if onTasks}
      <!-- Over every project's tasks (T-261): ID, name, description, type, labels, notes and the project's name -->
      <label class="task-search board-search">
        <Icon name="search" size={14} />
        <input
          type="search"
          id="board-search"
          placeholder="Search all projects’ tasks"
          aria-label="Search all projects' tasks by ID, name, description, type, label, note or project"
          value={dashboard.boardQuery}
          oninput={(e) => dashboard.setBoardQuery(e.currentTarget.value)}
        />
        {#if dashboard.boardQuery}
          <button class="search-clear" aria-label="Clear search" onclick={() => dashboard.setBoardQuery('')}
            ><Icon name="x" size={12} /></button
          >
        {/if}
      </label>
      <InfoTip label="all projects' tasks">
        <p>
          What's happening now across your projects: the tasks <strong>to do</strong> (ready ones first),
          <strong>in progress</strong>,
          <strong>waiting for a review</strong> or <strong>blocked</strong>, and those <strong>done</strong>. On the
          Board, at most 30 cards per column; <em>+ N more</em> says how many a project has beyond them. The List shows the
          same tasks, a row each.
        </p>
        <p>
          <em>Active sprints</em> shows each project's active sprint, all of its tasks, done ones too;
          <em>All tasks</em>, every sprint and the backlog, with what was done in the last 24 hours.
        </p>
        <p>
          A task opens its details here, over the page. Each project's full board is its own: the project chip or
          <em>+ N more</em> opens the project.
        </p>
      </InfoTip>
      <!-- The active sprints' tasks (the default) or every task, as before (T-340) -->
      <span class="board-scope" role="group" aria-label="Which tasks the board shows" id="board-scope">
        {#each BOARD_SCOPES as scope (scope)}
          <button
            class:active={dashboard.boardScope === scope}
            aria-pressed={dashboard.boardScope === scope}
            data-scope={scope}
            title={scope === 'active'
              ? "Only the tasks in each project's active sprint"
              : 'Every task: all sprints and the backlog'}
            onclick={() => dashboard.setBoardScope(scope)}>{scope === 'active' ? 'Active sprints' : 'All tasks'}</button
          >
        {/each}
      </span>
      {#if tab === 'board'}
        <label class="projects-active-only"
          ><input
            type="checkbox"
            id="board-group-by-project"
            checked={dashboard.boardGroupByProject}
            onchange={(e) => dashboard.setBoardGroupByProject(e.currentTarget.checked)}
          /> Group by project</label
        >
      {/if}
    {/if}
    {#if inactive}
      <label
        class="projects-active-only"
        title="Hide the projects with no open tasks and no activity in the last {ACTIVE_DAYS} days"
        ><input
          type="checkbox"
          id="active-projects-only"
          checked={dashboard.overviewActiveOnly}
          onchange={(e) => dashboard.setOverviewActiveOnly(e.currentTarget.checked)}
        />
        Hide inactive projects <span class="projects-hidden" id="projects-hidden">({inactive})</span></label
      >
    {/if}
    {#if onTasks && noSprint.length}
      <!-- So the board isn't empty without a word: these projects have nothing in the active sprints view -->
      <span class="projects-hidden" id="board-no-active-sprint" title={noSprint.map((p) => p.name).join(', ')}
        >{noSprint.length} project{noSprint.length === 1 ? ' has' : 's have'} no active sprint</span
      >
    {/if}
  </div>
  <div
    id="overview-panel"
    role="tabpanel"
    aria-labelledby={subViews.length > 1 ? `overview-tab-${tab}` : `item-${item}`}
  >
    {#if tab === 'board'}
      <AllProjectsBoard projects={shown} />
    {:else if tab === 'list'}
      <AllProjectsList projects={shown} />
    {:else if tab === 'agents'}
      <OverviewAgents projects={shown} />
    {:else if tab === 'progress'}
      <OverviewProgress projects={shown} />
    {:else if tab === 'reviews'}
      <OverviewReviews projects={shown} />
    {:else}
      <div class="projects-grid" id="projects-grid">
        {#each shown as project (project.id)}
          <ProjectCard {project} />
        {/each}
      </div>
    {/if}
  </div>
</section>
