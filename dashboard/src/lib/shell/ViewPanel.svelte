<script lang="ts">
  // A project's page (ATM-752, docs/sidebar-overhaul-design.md §5): its items as tabs, and the views of the item shown
  // as sub-tabs under them where it has more than one (Tasks: Board · List · Dependencies; Analysis: Progress · Agents)
  import { ITEMS, type Item } from '../address';
  import { itemOfView, PROJECT_ITEM_VIEWS, VIEW_ICONS, VIEW_LABELS, type ViewId } from '../tabs';
  import { inApp } from '../api/http';
  import { reviewsWaiting } from '../projects';
  import { dashboard } from '../state.svelte';
  import AgentsTab from '../agents/AgentsTab.svelte';
  import DepGraph from '../graph/DepGraph.svelte';
  import ItemTabs from './ItemTabs.svelte';
  import KanbanBoard from '../kanban/KanbanBoard.svelte';
  import ProgressTab from '../progress/ProgressTab.svelte';
  import ProjectTab from './ProjectTab.svelte';
  import ReviewsTab from './ReviewsTab.svelte';
  import InboxPage from '../inbox/InboxPage.svelte';
  import ScopeBar from './ScopeBar.svelte';
  import SubTabs from './SubTabs.svelte';
  import TaskGrid from '../tasks/TaskGrid.svelte';

  const view = $derived(dashboard.tab);
  const item = $derived(itemOfView(view));
  const subViews = $derived(
    PROJECT_ITEM_VIEWS[item].map((id) => ({ id, label: VIEW_LABELS[id], icon: VIEW_ICONS[id] })),
  );
  /** The project's code reviews waiting on the user (ATM-606), from its summary: the Code Reviews item's badge */
  const reviewsForYou = $derived(reviewsWaiting(dashboard.projects.find((p) => p.id === dashboard.projectId)));
  const badges = $derived({
    reviews: {
      count: reviewsForYou,
      says: 'waiting for you: a decision, a round, or an answer to read',
      id: 'reviews-waiting',
    },
  });

  // The address is the source of truth (FR-2.2): selecting a tab writes it, back/forward and links read it
  // (state.svelte.ts onNavigate). An item opens on its first view
  const selectItem = (next: Item) => next !== item && dashboard.selectTab(PROJECT_ITEM_VIEWS[next][0]);
  const selectView = (next: ViewId) => dashboard.selectTab(next);
  // In the app the sidebar's items do the item row's job (ATM-753); with the sidebar hidden, the row is back, until the
  // new tab's chooser reaches every page (ATM-755)
  const showsItems = $derived(!inApp || !dashboard.sidebarOpen);
</script>

<div class="graph-section" id="graph-section">
  {#if showsItems}
    <div class="view-header">
      <ItemTabs items={ITEMS} current={item} onselect={selectItem} {badges} label="Project" />
    </div>
  {/if}
  {#if subViews.length > 1}
    <SubTabs
      views={subViews}
      current={view}
      onselect={selectView}
      idPrefix="tab-"
      panel={(id) => `panel-${id}`}
      label={item === 'tasks' ? 'Tasks' : 'Analysis'}
    />
  {/if}
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
      <!-- Mounted only while selected, like the Agents tab (T-525): a row per task would otherwise be built again on
           every reload and project switch for a view nobody sees; its folds and its sort are the dashboard state's -->
      {#if view === 'list'}<TaskGrid />{/if}
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
      aria-labelledby="item-overview"
      hidden={view !== 'project'}
    >
      <!-- Mounted only while selected, like the Agents tab: it counts the whole history -->
      {#if view === 'project'}<ProjectTab />{/if}
    </div>
    <div
      class="view-panel"
      id="panel-reviews"
      role="tabpanel"
      aria-labelledby="item-reviews"
      hidden={view !== 'reviews'}
    >
      <!-- Mounted only while selected: it asks the server for the reviews, and again now and then -->
      {#if view === 'reviews'}<ReviewsTab />{/if}
    </div>
    <div class="view-panel" id="panel-inbox" role="tabpanel" aria-labelledby="item-inbox" hidden={view !== 'inbox'}>
      <!-- Mounted only while selected: it asks the server for the inbox, and again now and then (ATM-762) -->
      {#if view === 'inbox'}<InboxPage />{/if}
    </div>
    <!-- The Documents page (docs/documents-design.md) comes with its own task -->
    <div
      class="view-panel"
      id="panel-documents"
      role="tabpanel"
      aria-labelledby="item-documents"
      hidden={view !== 'documents'}
    >
      {#if view === 'documents'}
        <p class="view-placeholder" id="documents-placeholder">
          Documents are coming: the project’s documents, listed and searchable, to open in the viewer.
        </p>
      {/if}
    </div>
  </div>
</div>
