<script lang="ts">
  // All projects at a glance (docs/multi-project-dashboard-proposal.md 3.5): a card per project, those that need you
  // first; or, on the Omniboard tab, every project's tasks in one board (T-259, T-266). "Active only" hides projects with no open
  // work and nothing done lately, on both tabs.
  import { BOARD_SCOPES, withoutActiveSprint } from '../lib/allboard';
  import { ACTIVE_DAYS, isActive, orderProjects } from '../lib/projects';
  import { OVERVIEW_TABS, type OverviewTab } from '../lib/routes';
  import { dashboard } from '../lib/state.svelte';
  import AllProjectsBoard from './AllProjectsBoard.svelte';
  import Icon from './Icon.svelte';
  import InfoTip from './InfoTip.svelte';
  import OverviewAgents from './OverviewAgents.svelte';
  import ProjectCard from './ProjectCard.svelte';

  const ordered = $derived(orderProjects(dashboard.projects));
  const shown = $derived(
    dashboard.overviewActiveOnly ? ordered.filter((p) => isActive(p, dashboard.loadedAt)) : ordered,
  );
  const hidden = $derived(ordered.length - shown.length);
  const tab = $derived(dashboard.overviewTab);
  const noSprint = $derived(withoutActiveSprint(shown, dashboard.boardScope));

  // The board tab is called Omniboard (T-266); its id stays "board", so #view=board links and the saved tab still work
  const LABELS: Record<OverviewTab, string> = { projects: 'Projects', board: 'Omniboard', agents: 'Agents' };
  const ICONS = { projects: 'folder', board: 'square-kanban', agents: 'bot' } as const;
  const buttons: Partial<Record<OverviewTab, HTMLButtonElement>> = {};

  function onTabKeydown(e: KeyboardEvent) {
    const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const i = OVERVIEW_TABS.indexOf(tab);
    const next = OVERVIEW_TABS[(i + step + OVERVIEW_TABS.length) % OVERVIEW_TABS.length];
    dashboard.selectOverviewTab(next);
    buttons[next]?.focus();
  }
</script>

<!-- On the board, the page doesn't scroll: the board takes the window's height and its columns scroll (T-265) -->
<section class="projects-overview" class:on-board={tab === 'board'} id="projects-overview">
  <div class="view-tabs overview-tabs" role="tablist" aria-label="Projects Overview">
    {#each OVERVIEW_TABS as id (id)}
      <button
        bind:this={buttons[id]}
        class="view-tab"
        id="overview-tab-{id}"
        role="tab"
        aria-selected={tab === id}
        aria-controls="overview-panel"
        tabindex={tab === id ? 0 : -1}
        onclick={() => dashboard.selectOverviewTab(id)}
        onkeydown={onTabKeydown}><Icon name={ICONS[id]} size={15} />{LABELS[id]}</button
      >
    {/each}
  </div>
  <div class="projects-toolbar">
    {#if tab === 'board'}
      <!-- Over every project's tasks (T-261): ID, name, description, type, labels, notes and the project's name -->
      <label class="task-search board-search">
        <Icon name="search" size={14} />
        <input
          type="search"
          id="board-search"
          placeholder="Search the Omniboard"
          aria-label="Search the Omniboard's tasks by ID, name, description, type, label, note or project"
          value={dashboard.boardQuery}
          oninput={(e) => dashboard.setBoardQuery(e.currentTarget.value)}
        />
        {#if dashboard.boardQuery}
          <button class="search-clear" aria-label="Clear search" onclick={() => dashboard.setBoardQuery('')}
            ><Icon name="x" size={12} /></button
          >
        {/if}
      </label>
      <InfoTip label="the Omniboard">
        <p>
          What's happening now across your projects: the tasks <strong>ready to start</strong>,
          <strong>in progress</strong>,
          <strong>waiting for a review</strong> or <strong>blocked</strong>, and those
          <strong>done in the last 24 hours</strong>. At most 30 cards per column; <em>+ N more</em> says how many a project
          has beyond them.
        </p>
        <p>
          <em>Active sprints</em> shows only the tasks in each project's active sprint; <em>All tasks</em>, every sprint
          and the backlog.
        </p>
        <p>
          Each project's full details are on its own Kanban board: a card opens its task there, the project chip or
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
      <label class="projects-active-only"
        ><input
          type="checkbox"
          id="board-group-by-project"
          checked={dashboard.boardGroupByProject}
          onchange={(e) => dashboard.setBoardGroupByProject(e.currentTarget.checked)}
        /> Group by project</label
      >
    {/if}
    <label
      class="projects-active-only"
      title="Hide projects with no open tasks and no activity in the last {ACTIVE_DAYS} days"
      ><input
        type="checkbox"
        id="active-projects-only"
        checked={dashboard.overviewActiveOnly}
        onchange={(e) => dashboard.setOverviewActiveOnly(e.currentTarget.checked)}
      /> Active only</label
    >
    {#if hidden}
      <span class="projects-hidden" id="projects-hidden">{hidden} inactive project{hidden === 1 ? '' : 's'} hidden</span
      >
    {/if}
    {#if tab === 'board' && noSprint.length}
      <!-- So the board isn't empty without a word: these projects have nothing in the active sprints view -->
      <span class="projects-hidden" id="board-no-active-sprint" title={noSprint.map((p) => p.name).join(', ')}
        >{noSprint.length} project{noSprint.length === 1 ? ' has' : 's have'} no active sprint</span
      >
    {/if}
  </div>
  <div id="overview-panel" role="tabpanel" aria-labelledby="overview-tab-{tab}">
    {#if tab === 'board'}
      <AllProjectsBoard projects={shown} />
    {:else if tab === 'agents'}
      <OverviewAgents projects={shown} />
    {:else}
      <div class="projects-grid" id="projects-grid">
        {#each shown as project (project.id)}
          <ProjectCard {project} />
        {/each}
      </div>
    {/if}
  </div>
</section>
