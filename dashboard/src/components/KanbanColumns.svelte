<script lang="ts">
  import type { KanbanColumn } from '../lib/kanban';
  import { doneSections, workflowType } from '../lib/stages';
  import { dashboard } from '../lib/state.svelte';
  import { kindStyle } from '../lib/theme';
  import KanbanCard from './KanbanCard.svelte';
  import KanbanScroll from './KanbanScroll.svelte';

  const DONE_LIMIT = 20;

  let {
    columns,
    showAllDone,
    showSprint = false,
    foldEmpty = false,
    onshowall,
  }: {
    columns: KanbanColumn[];
    showAllDone: boolean;
    /** Cards name their sprint (the board shows more than one) */
    showSprint?: boolean;
    /** A closed sprint's board (T-323): empty columns fold into slim strips, and Done lays its cards out in a grid */
    foldEmpty?: boolean;
    onshowall: (all: boolean) => void;
  } = $props();

  const columnColor = (column: KanbanColumn) => (column.kind ? kindStyle(column.kind).color : 'var(--text-muted)');
  // In a pull-request project the Done column is split by stage: In PR (per PR), Done, Merged (D-4.2)
  const sectioned = $derived(workflowType(dashboard.meta) === 'pull-request');
  // The board takes the panel's height and each column scrolls inside it (T-262, app.css), so the page itself never
  // scrolls on the Kanban
</script>

<div class="kanban-board">
  {#each columns as column (column.id)}
    {@const isDone = column.kind === 'done'}
    {@const limited = isDone && !showAllDone && column.cards.length > DONE_LIMIT}
    {@const folded = foldEmpty && !isDone && column.cards.length === 0}
    <section
      class="kanban-column"
      class:folded
      class:wide={foldEmpty && isDone}
      data-status={column.id}
      aria-label="{column.label} ({column.cards.length})"
    >
      <header class="kanban-column-header">
        <span class="kanban-column-title" style:--column-color={columnColor(column)}>{column.label}</span>
        <span class="kanban-count">{column.cards.length}</span>
      </header>
      {#if !folded}<KanbanScroll>
          {#if isDone && sectioned && column.cards.length}
            {#each doneSections(column.cards, dashboard.vocab) as section (section.id)}
              {@const cut = section.stage === 'merged' && !showAllDone && section.cards.length > DONE_LIMIT}
              <div class="kanban-section" data-section={section.id}>
                <div class="kanban-section-header">
                  {#if section.url}
                    <a href={section.url} target="_blank" rel="noopener noreferrer">{section.label}</a>
                  {:else}
                    <span>{section.label}</span>
                  {/if}
                  <span class="kanban-section-count">{section.cards.length}</span>
                </div>
                {#each cut ? section.cards.slice(0, DONE_LIMIT) : section.cards as card (card.task.id)}
                  <KanbanCard {card} {showSprint} />
                {/each}
                {#if cut}
                  <button class="kanban-show-all" onclick={() => onshowall(true)}
                    >Show all ({section.cards.length})</button
                  >
                {:else if section.stage === 'merged' && showAllDone && section.cards.length > DONE_LIMIT}
                  <button class="kanban-show-all" id="kanban-show-fewer" onclick={() => onshowall(false)}
                    >Show the latest {DONE_LIMIT}</button
                  >
                {/if}
              </div>
            {/each}
          {:else}
            {#each limited ? column.cards.slice(0, DONE_LIMIT) : column.cards as card (card.task.id)}
              <KanbanCard {card} {showSprint} />
            {:else}
              <div class="kanban-empty">No tasks</div>
            {/each}
            {#if limited}
              <button class="kanban-show-all" onclick={() => onshowall(true)}>Show all ({column.cards.length})</button>
            {:else if isDone && showAllDone && column.cards.length > DONE_LIMIT}
              <button class="kanban-show-all" id="kanban-show-fewer" onclick={() => onshowall(false)}
                >Show the latest {DONE_LIMIT}</button
              >
            {/if}
          {/if}
        </KanbanScroll>{/if}
    </section>
  {/each}
</div>
