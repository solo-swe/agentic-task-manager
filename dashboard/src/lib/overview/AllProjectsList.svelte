<script lang="ts">
  // All's List (ATM-752, docs/sidebar-overhaul-design.md §1: the Tasks item's List for All projects): the Omniboard's
  // tasks as a list, grouped by their column, a row per task with its project. The same tasks as the board (each
  // project's summary: what is ready, at work, waiting and done lately, in the active sprints or all of them), the same
  // search and options, but no cap per column: every card the summaries carry, and "+ N more" for what they leave out
  // (they carry a project's first cards of each column). A row opens the task's details over the list, as a card does.
  import { allBoard, type AllBoardCard } from '../allboard';
  import { boardSearch } from './board-search.svelte';
  import type { ProjectEntry } from '../load';
  import { dashboard } from '../state.svelte';
  import { followInPlace } from '../new-tab-clicks';
  import { withTask } from '../routes';
  import { priorityStyle, typeStyle } from '../theme';
  import { DEFAULT_VOCABULARY, priorityRank } from '../vocab';
  import ProjectLogo from '../ui/ProjectLogo.svelte';
  import TaskKey from '../ui/TaskKey.svelte';

  let { projects }: { projects: ProjectEntry[] } = $props();

  const search = boardSearch(() => projects);
  // The board's columns without its cap: each a group of rows, with what the summaries leave out ("+ N more")
  const groups = $derived(
    allBoard(projects, Infinity, search.match, dashboard.boardScope).filter((g) => g.cards.length || g.more.length),
  );
  const tasksText = (n: number) => `${n} ${n === 1 ? 'task' : 'tasks'}`;

  function open(row: AllBoardCard) {
    void dashboard.openOnBoard(row.project.id, row.card.id);
  }
</script>

<div class="all-list" id="all-list">
  {#each groups as group (group.id)}
    <section class="list-group" data-group={group.id}>
      <header class="list-group-header">
        <strong class="list-group-name">{group.label}</strong>
        <span class="list-group-count"
          >{search.match ? `${group.cards.length} of ${group.total}` : tasksText(group.total)}</span
        >
      </header>
      <ul class="all-list-rows">
        {#each group.cards as row (row.project.id + '/' + row.card.id)}
          {@const card = row.card}
          {@const ts = typeStyle(card.type ?? '')}
          <li class="all-list-row" data-id={card.id} data-project={row.project.id}>
            <button
              class="board-project-chip"
              title="Open {row.project.name}"
              onclick={() => dashboard.openProject(row.project.id)}
              ><ProjectLogo project={row.project} size={14} /><span class="board-project-name">{row.project.name}</span
              ></button
            >
            <span class="kanban-card-id"><TaskKey task={card} /></span>
            <a
              class="all-list-name card-link"
              href={withTask(dashboard.projectAddress(row.project.id), card.key ?? card.id)}
              onclick={(e) => followInPlace(e, () => open(row))}>{card.name}</a
            >
            {#if card.type}<span class="badge" style="background:{ts.bg};color:{ts.color}">{card.type}</span>{/if}
            {#if card.priority}
              {@const ps = priorityStyle(priorityRank(DEFAULT_VOCABULARY, card.priority))}
              <span
                class="badge priority-badge"
                data-priority={card.priority}
                style="background:{ps.bg};color:{ps.color}">{card.priority}</span
              >
            {/if}
            {#if card.sprint}<span class="all-list-sprint" title="Sprint: {card.sprint}">{card.sprint}</span>{/if}
          </li>
        {/each}
      </ul>
      {#each group.more as more (more.project.id)}
        <button
          class="kanban-show-all board-more"
          data-project={more.project.id}
          title="Open {more.project.name}'s board"
          onclick={() => dashboard.openProject(more.project.id, { view: 'kanban' })}
          >+ {more.count} more in {more.project.name}</button
        >
      {/each}
    </section>
  {:else}
    <p class="kanban-empty" id="all-list-empty">
      {search.match
        ? 'No task matches the search'
        : dashboard.boardScope === 'active'
          ? 'Nothing in the active sprints: no task ready, at work, waiting or done lately'
          : 'Nothing to list: no task ready, at work, waiting or done lately'}
    </p>
  {/each}
</div>

<style>
  .all-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
  }
  .all-list-rows {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface);
  }
  .all-list-row {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-3) var(--space-5);
    min-width: 0;
  }
  .all-list-row + .all-list-row {
    border-top: 1px solid var(--border);
  }
  .all-list-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .all-list-sprint {
    color: var(--text-muted);
    font-size: var(--text-sm);
    white-space: nowrap;
  }
</style>
