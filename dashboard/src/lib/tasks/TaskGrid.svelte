<script lang="ts">
  import { BACKLOG, sortTasks } from '../filters';
  import { groupList, type ListGroup } from '../list-groups';
  import { countKinds } from '../scope';
  import { dashboard } from '../state.svelte';
  import Icon from '../ui/Icon.svelte';
  import ListOptions from './ListOptions.svelte';
  import SprintMarker from '../progress/SprintMarker.svelte';
  import TaskCard from './TaskCard.svelte';

  // The order and the groups are part of the project's saved view (dashboard state). Grouped by sprint (T-323), the
  // List is the planning view: the active sprint, the planned ones in order, the backlog, the closed ones folded. That
  // applies where it shows more than one sprint; one sprint's tasks are listed as they are.
  const groupBy = $derived(dashboard.listGroups);
  const groups = $derived(
    groupList(dashboard.filtered, groupBy, dashboard.sprints, dashboard.vocab, dashboard.components).map((g) => ({
      ...g,
      tasks: sortTasks(g.tasks, dashboard.listSort, dashboard.vocab),
    })),
  );
  // A closed sprint still holding open tasks (reopened after it closed) doesn't fold away with the others (T-323)
  const leftover = (g: ListGroup) => {
    const c = countKinds(g.tasks, dashboard.vocab);
    return c.total > c.done;
  };
  const open = $derived(groups.filter((g) => g.sprint?.state !== 'closed' || leftover(g)));
  const closed = $derived(groups.filter((g) => g.sprint?.state === 'closed' && !leftover(g)));
  // Cards name their sprint when the list shows several and doesn't group by them
  const showSprint = $derived(
    groupBy !== 'sprint' && new Set(dashboard.filtered.map((t) => dashboard.sprints.sprintOf(t) ?? BACKLOG)).size > 1,
  );
  const tasksText = (n: number) => `${n} ${n === 1 ? 'task' : 'tasks'}`;
</script>

{#snippet group(g: ListGroup)}
  {@const counts = countKinds(g.tasks, dashboard.vocab)}
  <section class="list-group" data-group={g.key}>
    {#if groupBy !== 'none'}
      <header class="list-group-header">
        {#if g.sprint}<SprintMarker state={g.sprint.state} />{/if}
        <strong class="list-group-name">{g.label}</strong>
        {#if g.sprint?.state === 'planned'}<span class="sprint-state">planned</span>{/if}
        <span class="list-group-count">{tasksText(g.tasks.length)} · {counts.done} of {counts.total} done</span>
        {#if g.sprint?.goal}<span class="list-group-goal" title={g.sprint.goal}>{g.sprint.goal}</span>{/if}
        {#if g.sprint?.state === 'closed' && counts.total > counts.done}
          <span class="scope-leftovers list-group-leftovers"
            >{counts.total - counts.done} open after closing · move them with <code>/atm:plan-sprint</code></span
          >
        {/if}
        {#if groupBy === 'sprint' && g.key === BACKLOG}
          <span class="list-group-hint">plan them into a sprint with <code>/atm:plan-sprint</code></span>
        {/if}
      </header>
    {/if}
    <div class="task-grid">
      {#each g.tasks as task (task.id)}
        <TaskCard {task} {showSprint} />
      {/each}
    </div>
  </section>
{/snippet}

<div id="phases">
  {#if dashboard.loaded}
    {#if dashboard.scope.kind === 'none'}
      <!-- Without sprints there is no scope bar to hold the options -->
      <div class="kanban-toolbar"><ListOptions /></div>
    {/if}
    <div class="list-groups">
      {#each open as g (g.key)}{@render group(g)}{/each}
      {#if closed.length && (dashboard.narrowed || dashboard.sprints.derived)}
        <!-- A search or filter looks for something: what it finds in closed sprints shows, not folded. Nor in a project
             not migrated yet, whose finished phases are read as closed sprints nobody closed -->
        {#each closed as g (g.key)}{@render group(g)}{/each}
      {:else if closed.length}
        <button
          class="finished-chip list-closed-toggle"
          id="list-closed-toggle"
          aria-expanded={dashboard.listShowClosed}
          onclick={() => (dashboard.listShowClosed = !dashboard.listShowClosed)}
          ><SprintMarker state="closed" />{closed.length === 1 ? closed[0].label : `${closed.length} closed sprints`}
          · {tasksText(closed.reduce((n, g) => n + g.tasks.length, 0))}
          <Icon name="chevron-down" size={12} /></button
        >
        {#if dashboard.listShowClosed}
          {#each closed as g (g.key)}{@render group(g)}{/each}
        {/if}
      {/if}
      {#if dashboard.filtered.length === 0}
        <div style="color:var(--text-muted);padding:12px;font-size:14px">No tasks match current filters</div>
      {/if}
      {#if dashboard.archivedCount && !dashboard.includeArchived}
        <!-- Where the list ends, the archive is one click away (T-354, found in T-277) -->
        <button
          class="finished-chip list-archived-toggle"
          id="list-archived-toggle"
          onclick={() => dashboard.setIncludeArchived(true)}
          >{dashboard.archivedCount} archived {dashboard.archivedCount === 1 ? 'task' : 'tasks'} hidden · show</button
        >
      {/if}
    </div>
  {/if}
</div>
