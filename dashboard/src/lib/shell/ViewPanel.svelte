<script lang="ts">
  import { adjacentView, GROUP_END, VIEW_ICONS, VIEW_LABELS, VIEWS, type ViewId } from '../tabs';
  import { dashboard } from '../state.svelte';
  import AgentsTab from '../agents/AgentsTab.svelte';
  import DepGraph from '../graph/DepGraph.svelte';
  import Icon from '../ui/Icon.svelte';
  import KanbanBoard from '../kanban/KanbanBoard.svelte';
  import ProgressTab from '../progress/ProgressTab.svelte';
  import ProjectTab from './ProjectTab.svelte';
  import ScopeBar from './ScopeBar.svelte';
  import TaskGrid from '../tasks/TaskGrid.svelte';

  const view = $derived(dashboard.tab);
  const tabs: Partial<Record<ViewId, HTMLButtonElement>> = {};

  // The address is the source of truth (FR-2.2): selecting a tab writes it, back/forward and links read it
  // (state.svelte.ts onNavigate).
  function select(next: ViewId, focus = false) {
    dashboard.selectTab(next);
    if (focus) tabs[next]?.focus();
  }

  function onTabKeydown(e: KeyboardEvent) {
    const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    select(adjacentView(view, step), true);
  }
</script>

<div class="graph-section" id="graph-section">
  <div class="view-header">
    <div class="view-tabs" role="tablist" aria-label="Views">
      {#each VIEWS as id (id)}
        <button
          bind:this={tabs[id]}
          class="view-tab"
          id="tab-{id}"
          role="tab"
          aria-selected={view === id}
          aria-controls="panel-{id}"
          tabindex={view === id ? 0 : -1}
          onclick={() => select(id)}
          onkeydown={onTabKeydown}><Icon name={VIEW_ICONS[id]} size={15} />{VIEW_LABELS[id]}</button
        >
        {#if id === GROUP_END}<span class="view-tab-divider" aria-hidden="true"></span>{/if}
      {/each}
    </div>
  </div>
  <div class="view-panels">
    {#if view === 'kanban' || view === 'list' || view === 'graph'}<ScopeBar />{/if}
    <!-- Inactive panels stay mounted (hidden) so each view keeps its state, e.g. the graph's pan/zoom (FR-2.4) -->
    <div class="view-panel" id="panel-graph" role="tabpanel" aria-labelledby="tab-graph" hidden={view !== 'graph'}>
      <DepGraph />
    </div>
    <div class="view-panel" id="panel-kanban" role="tabpanel" aria-labelledby="tab-kanban" hidden={view !== 'kanban'}>
      <KanbanBoard />
    </div>
    <div class="view-panel" id="panel-list" role="tabpanel" aria-labelledby="tab-list" hidden={view !== 'list'}>
      <TaskGrid />
    </div>
    <div class="view-panel" id="panel-agents" role="tabpanel" aria-labelledby="tab-agents" hidden={view !== 'agents'}>
      <!-- Mounted only while selected: its ~2 elements per task would otherwise re-render on every reload, and its
           state (zoom, Live) lives in the dashboard state anyway -->
      {#if view === 'agents'}<AgentsTab />{/if}
    </div>
    <div
      class="view-panel"
      id="panel-progress"
      role="tabpanel"
      aria-labelledby="tab-progress"
      hidden={view !== 'progress'}
    >
      <ProgressTab />
    </div>
    <div
      class="view-panel"
      id="panel-project"
      role="tabpanel"
      aria-labelledby="tab-project"
      hidden={view !== 'project'}
    >
      <!-- Mounted only while selected, like the Agents tab: it counts the whole history -->
      {#if view === 'project'}<ProjectTab />{/if}
    </div>
  </div>
</div>
