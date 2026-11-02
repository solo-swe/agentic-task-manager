<script lang="ts">
  // The All projects board (T-259, docs/all-projects-kanban-proposal.md §3): what's happening now, everywhere. Cards
  // like a project's Kanban cards, each with its project's chip; a card opens the task in its project, the chip the
  // project, "+ N more" the project's Kanban. Grouped by project, a lane per project instead of the chips.
  import {
    allBoard,
    allBoardLanes,
    boardOf,
    type AllBoardCard,
    type AllBoardColumn,
    type BoardMatch,
  } from '../lib/allboard';
  import { formatAgo, shortAgent } from '../lib/claims';
  import { searchProjects, type ProjectEntry } from '../lib/load';
  import { dashboard } from '../lib/state.svelte';
  import { kindStyle, priorityStyle, typeStyle } from '../lib/theme';
  import { DEFAULT_VOCABULARY, priorityRank } from '../lib/vocab';
  import { blockSummary, reviewSummary } from '../lib/waiting';
  import KanbanScroll from './KanbanScroll.svelte';
  import ProjectLogo from './ProjectLogo.svelte';

  let { projects }: { projects: ProjectEntry[] } = $props();

  // The search (T-261): the server searches every project's tasks (api/search, their descriptions and notes too,
  // which the board's cards don't carry), and the board keeps its cards among the results. Asked once per pause in
  // typing, and again when the board's cards change (not on every poll)
  const SEARCH_PAUSE_MS = 300;
  let match = $state<BoardMatch>(null);
  // The active sprints' board or the whole one (T-340)
  const scope = $derived(dashboard.boardScope);
  const boardCards = $derived(
    projects
      .flatMap((p) =>
        Object.values(boardOf(p, scope) ?? {}).flatMap((c) => c.cards.map((card) => `${p.id}/${card.id}`)),
      )
      .join(' '),
  );
  $effect(() => {
    const query = dashboard.boardQuery.trim();
    void boardCards;
    if (!query) {
      match = null;
      return;
    }
    let current = true;
    const timer = setTimeout(
      () =>
        searchProjects(query)
          .then((found) => current && (match = found))
          .catch((e) => console.error('Searching the projects failed:', e)),
      SEARCH_PAUSE_MS,
    );
    return () => {
      current = false;
      clearTimeout(timer);
    };
  });

  // Grouped by project (T-260): a lane per project with something on the board, the most recently active first
  const columns = $derived(dashboard.boardGroupByProject ? [] : allBoard(projects, undefined, match, scope));
  const lanes = $derived(dashboard.boardGroupByProject ? allBoardLanes(projects, undefined, match, scope) : []);
  const KINDS = { todo: 'todo', active: 'active', review: 'review', blocked: 'blocked', done: 'done' } as const;
  const LABELS_SHOWN = 2;

  const minutesSince = (iso: string | null | undefined) => {
    const t = Date.parse(iso ?? '');
    return Number.isNaN(t) ? null : Math.max(0, (dashboard.loadedAt - t) / 60_000);
  };

  /** The card's last line, by column: who works on it, how long it waits, or when it was finished */
  function status({ card }: AllBoardCard, column: string): string | null {
    if (column === 'active' && card.by)
      return [shortAgent(card.by), card.since && formatAgo(card.since, dashboard.loadedAt)].filter(Boolean).join(' · ');
    if (column === 'review') return reviewSummary({ since: 0, minutes: minutesSince(card.since) ?? 0 });
    if (column === 'blocked') {
      const on = card.waiting_on === 'user' || card.waiting_on === 'agent' ? card.waiting_on : null;
      const questions = Array.from({ length: card.questions ?? 0 }, () => '');
      return blockSummary({ on, minutes: minutesSince(card.since), questions });
    }
    if (column === 'done' && card.completed_at) return `Done ${formatAgo(card.completed_at, dashboard.loadedAt)}`;
    return null;
  }

  function open(item: AllBoardCard) {
    dashboard.openProject(item.project.id, { task: item.card.id });
  }

  function onKeydown(e: KeyboardEvent, item: AllBoardCard) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      open(item);
    }
  }
</script>

<!-- The five columns, of every project or of one project's lane (no chips there: the lane says whose they are) -->
{#snippet boardColumns(columns: AllBoardColumn[], lane: boolean)}
  {#each columns as column (column.id)}
    <!-- Searching: how many of the column's cards match -->
    {@const count = match ? `${column.cards.length} of ${column.total}` : String(column.total)}
    <section class="kanban-column" data-status={column.id} aria-label="{column.label} ({count})">
      <header class="kanban-column-header">
        <span class="kanban-column-title" style:--column-color={kindStyle(KINDS[column.id]).color}>{column.label}</span>
        <span class="kanban-count">{count}</span>
      </header>
      <KanbanScroll>
        {#each column.cards as item (item.project.id + '/' + item.card.id)}
          {@const card = item.card}
          {@const ts = typeStyle(card.type)}
          {@const line = status(item, column.id)}
          <div
            class="kanban-card board-card"
            data-id={card.id}
            data-project={item.project.id}
            role="button"
            tabindex="0"
            onclick={() => open(item)}
            onkeydown={(e) => onKeydown(e, item)}
          >
            <div class="kanban-card-top">
              {#if !lane}
                <button
                  class="board-project-chip"
                  title="Open {item.project.name}"
                  onclick={(e) => {
                    e.stopPropagation();
                    dashboard.openProject(item.project.id);
                  }}
                  onkeydown={(e) => e.stopPropagation()}
                  ><ProjectLogo project={item.project} size={14} /><span class="board-project-name"
                    >{item.project.name}</span
                  ></button
                >
              {/if}
              <span class="kanban-card-id">{card.id}</span>
              <span class="badge" style="background:{ts.bg};color:{ts.color}">{card.type}</span>
              {#if card.priority}
                {@const ps = priorityStyle(priorityRank(DEFAULT_VOCABULARY, card.priority))}
                <span
                  class="badge priority-badge"
                  data-priority={card.priority}
                  style="background:{ps.bg};color:{ps.color}">{card.priority}</span
                >
              {/if}
              {#if card.labels.length}
                <span class="label-chips compact"
                  >{#each card.labels.slice(0, LABELS_SHOWN) as label (label)}<span
                      class="label-chip"
                      data-label={label}
                      title="Label: {label}">{label}</span
                    >{/each}{#if card.labels.length > LABELS_SHOWN}<span
                      class="label-chip label-more"
                      title={card.labels.join(', ')}>+{card.labels.length - LABELS_SHOWN}</span
                    >{/if}</span
                >
              {/if}
            </div>
            <div class="kanban-card-name">{card.name}</div>
            {#if line}
              <div
                class="kanban-waiting board-card-status"
                class:on-user={column.id === 'blocked' && card.waiting_on === 'user'}
              >
                {line}
              </div>
            {/if}
          </div>
        {:else}
          <div class="kanban-empty">{column.empty}</div>
        {/each}
        {#each column.more as more (more.project.id)}
          <button
            class="kanban-show-all board-more"
            data-project={more.project.id}
            title="Open {more.project.name}'s Kanban board"
            onclick={() => dashboard.openProject(more.project.id, { view: 'kanban' })}
            >+ {more.count} more{lane ? '' : ` in ${more.project.name}`}</button
          >
        {/each}
      </KanbanScroll>
    </section>
  {/each}
{/snippet}

{#if dashboard.boardGroupByProject}
  <div class="all-board-lanes" id="all-board">
    {#each lanes as lane (lane.project.id)}
      <div class="kanban-lane board-lane" data-project={lane.project.id}>
        <div class="kanban-lane-header">
          <button
            class="board-lane-name"
            title="Open {lane.project.name}"
            onclick={() => dashboard.openProject(lane.project.id)}
            ><ProjectLogo project={lane.project} size={18} />{lane.project.name}</button
          >
        </div>
        <div class="kanban-board all-board">{@render boardColumns(lane.columns, true)}</div>
      </div>
    {:else}
      <p class="kanban-empty" id="all-board-empty">
        {match
          ? 'No card on the board matches the search'
          : scope === 'active'
            ? 'Nothing on the board in the active sprints: no task ready, at work, waiting or done lately'
            : 'Nothing on the board: no task ready, at work, waiting or done lately'}
      </p>
    {/each}
  </div>
{:else}
  <div class="kanban-board all-board" id="all-board">{@render boardColumns(columns, false)}</div>
{/if}
