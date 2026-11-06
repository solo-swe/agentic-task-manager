<script lang="ts">
  import { BACKLOG } from '../filters';
  import { buildBoard, buildLanes } from '../kanban';
  import { dashboard } from '../state.svelte';
  import { phaseColor } from '../theme';
  import KanbanColumns from './KanbanColumns.svelte';
  import SprintMarker from '../progress/SprintMarker.svelte';

  const showAllDone = $derived(dashboard.kanbanShowAllDone);
  // Both options are part of the project's saved view (dashboard state); the scope bar holds them (T-323), and
  // grouping applies only where the board shows more than one sprint
  const groupBySprint = $derived(
    dashboard.kanbanGroupBySprint && (dashboard.scope.kind === 'all' || dashboard.scope.kind === 'several'),
  );
  const showSkipped = $derived(dashboard.kanbanShowSkipped);
  // Cards name their sprint only when the board shows more than one (SR-7.3); a lane names its own
  const showSprint = $derived(
    !groupBySprint && new Set(dashboard.filtered.map((t) => dashboard.sprints.sprintOf(t) ?? BACKLOG)).size > 1,
  );
  // A closed sprint's board is mostly Done (T-323): its empty columns fold so the cards get the width
  const foldEmpty = $derived(dashboard.scope.kind === 'sprint' && dashboard.scope.sprint.state === 'closed');
  const laneColor = (index: number) => (index < 0 ? 'var(--text-muted)' : phaseColor(index));

  const options = $derived({
    showSkipped,
    statusFilter: dashboard.filters.status,
    vocab: dashboard.vocab,
    sprints: dashboard.sprints,
    // Next up is atm's own answer (T-321): the open project's summary, which every poll reloads with the tasks
    nextUp: dashboard.projects.find((p) => p.id === dashboard.projectId)?.summary?.next?.[0]?.id ?? null,
  });
  const board = $derived(groupBySprint ? [] : buildBoard(dashboard.filtered, dashboard.tasks, options));
  const lanes = $derived(groupBySprint ? buildLanes(dashboard.filtered, dashboard.tasks, options) : []);
</script>

<div class="kanban" class:grouped={groupBySprint} id="kanban">
  {#if dashboard.scope.kind === 'none'}
    <!-- Without sprints there is no scope bar to hold the option -->
    <div class="kanban-toolbar">
      <label class="kanban-option"
        ><input
          type="checkbox"
          id="kanban-show-skipped"
          checked={showSkipped}
          onchange={(e) => (dashboard.kanbanShowSkipped = e.currentTarget.checked)}
        /> Show skipped</label
      >
    </div>
  {/if}
  {#if groupBySprint}
    {#each lanes as lane (lane.key)}
      {@const color = laneColor(lane.index)}
      <div class="kanban-lane" data-sprint={lane.key}>
        <div class="kanban-lane-header">
          <span class="phase-badge" style="background:color-mix(in srgb, {color} 12.5%, transparent);color:{color}"
            ><SprintMarker state={dashboard.sprints.sprints[lane.index]?.state} />{lane.key === BACKLOG
              ? 'Backlog'
              : lane.key}</span
          >
          <span class="kanban-lane-progress">{lane.done}/{lane.total} done</span>
        </div>
        <KanbanColumns columns={lane.columns} {showAllDone} onshowall={(all) => (dashboard.kanbanShowAllDone = all)} />
      </div>
    {/each}
  {:else}
    <KanbanColumns
      columns={board}
      {showAllDone}
      {showSprint}
      {foldEmpty}
      onshowall={(all) => (dashboard.kanbanShowAllDone = all)}
    />
  {/if}
</div>
