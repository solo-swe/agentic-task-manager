<script lang="ts">
  import { tick } from 'svelte';
  import { ACTIVE, BACKLOG, FILTER_GROUPS, FILTER_LABELS, filterOptions } from '../filters';
  import { formatWhen } from '../format';
  import { readReleases } from '../releases';
  import { countKinds, scopeTasks } from '../scope';
  import { backlog, dueText, sprintSummary, sprintTasks, type Sprint } from '../sprints';
  import { dashboard } from '../state.svelte';
  import { kindOf } from '../vocab';
  import Icon from '../ui/Icon.svelte';
  import ListOptions from '../tasks/ListOptions.svelte';
  import SprintMarker from '../progress/SprintMarker.svelte';

  // Above the Kanban, List and Graph (T-323, SR-7.2): which tasks they show, by sprint, and that scope's progress. The
  // menu sets the Sprint filter to one choice; several, picked in the filters, show as "Filtered". Hints name the
  // skill to run next (the dashboard only reads): close the sprint once its work is done, plan or start one.
  const CLOSED_IN_MENU = 5;
  const scope = $derived(dashboard.scope);
  const model = $derived(dashboard.sprints);
  const own = $derived(scopeTasks(dashboard.history, model, dashboard.sprintSelection));
  const counts = $derived(countKinds(own, dashboard.vocab));
  const percent = $derived(counts.total ? Math.round((counts.done / counts.total) * 100) : 0);
  const summary = $derived(
    scope.kind === 'sprint'
      ? sprintSummary(model, dashboard.history, scope.sprint, dashboard.vocab, dashboard.loadedAt)
      : undefined,
  );
  const open = (tasks: typeof dashboard.tasks) =>
    tasks.filter((t) => !['done', 'skipped'].includes(kindOf(dashboard.vocab, t.status) ?? '')).length;
  const backlogOpen = $derived(open(backlog(model, dashboard.tasks)));
  const sizeOf = (sprint: Sprint) => sprintTasks(model, dashboard.tasks, sprint.id).length;
  // The active sprint with nothing left to start: all done, or the rest waiting on a review or a decision
  const sprintEnded = $derived(
    scope.kind === 'sprint' &&
      scope.sprint.state === 'active' &&
      counts.total > 0 &&
      counts.todo === 0 &&
      counts.active === 0,
  );
  const tasks = (n: number) => `${n} ${n === 1 ? 'task' : 'tasks'}`;
  // What narrows the board within its sprints (T-323): the other filters, by group, and the search
  const options = $derived(filterOptions(dashboard.base, dashboard.vocab, model, dashboard.components));
  const narrowing = $derived(
    [
      ...FILTER_GROUPS.filter((g) => g !== 'sprint' && dashboard.filters[g].size > 0).map(
        (g) =>
          `${FILTER_LABELS[g]} ${options[g]
            .filter((o) => dashboard.filters[g].has(o.value))
            .map((o) => o.label)
            .join(', ')}`,
      ),
      ...(dashboard.query.trim() ? [`“${dashboard.query.trim()}”`] : []),
    ].join(' · '),
  );
  // The closed sprints holding open tasks, in the sprints' order (T-323)
  const leftoverSprints = $derived([
    ...new Set(dashboard.leftovers.map((t) => model.sprintOf(t)).filter((s): s is string => s !== undefined)),
  ]);
  const them = $derived(dashboard.leftovers.length === 1 ? 'it' : 'them');
  // Releases already shipped: a closed sprint's says "released" (T-323)
  const releasedIds = $derived(
    new Set(
      readReleases(dashboard.meta)
        .filter((r) => r.state !== 'planned')
        .map((r) => r.id),
    ),
  );
  const dateOf = (iso: string | undefined) =>
    iso ? new Date(iso).toLocaleDateString(undefined, { month: 'short', day: 'numeric' }) : undefined;

  // The menu: the active sprint, the planned ones, the backlog, all tasks, then the last closed ones
  const planned = $derived(model.sprints.filter((s) => s.state === 'planned'));
  const closed = $derived(
    model.sprints
      .filter((s) => s.state === 'closed')
      .reverse()
      .slice(0, CLOSED_IN_MENU),
  );
  const active = $derived(model.sprints.find((s) => s.state === 'active'));
  const selected = $derived(dashboard.sprintSelection);
  const isChoice = (values: string[]) => values.length === selected.size && values.every((v) => selected.has(v));
  const label = $derived(
    scope.kind === 'sprint'
      ? scope.sprint.id
      : scope.kind === 'backlog'
        ? 'Backlog'
        : scope.kind === 'several'
          ? `Filtered: ${scope.names.length} selected`
          : 'All tasks',
  );

  let menuOpen = $state(false);
  let root = $state<HTMLElement>();
  let trigger = $state<HTMLButtonElement>();
  let menu = $state<HTMLElement>();
  const items = () => [...(menu?.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]') ?? [])];

  async function toggleMenu() {
    menuOpen = !menuOpen;
    if (!menuOpen) return;
    await tick();
    (items().find((i) => i.getAttribute('aria-checked') === 'true') ?? items()[0])?.focus();
  }
  function close(refocus = true) {
    menuOpen = false;
    if (refocus) trigger?.focus();
  }
  function choose(values: string[]) {
    dashboard.setSprintScope(values);
    close();
  }
  function onKey(e: KeyboardEvent) {
    const all = items();
    const i = all.indexOf(document.activeElement as HTMLButtonElement);
    const move = (to: number) => {
      e.preventDefault();
      all[Math.max(0, Math.min(all.length - 1, to))]?.focus();
    };
    if (e.key === 'ArrowDown') move(i + 1);
    else if (e.key === 'ArrowUp') move(i - 1);
    else if (e.key === 'Home') move(0);
    else if (e.key === 'End') move(all.length - 1);
    else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close();
    }
  }
</script>

<svelte:window onclick={(e) => menuOpen && root && !root.contains(e.target as Node) && close(false)} />

{#snippet choice(values: string[], text: string, state?: Sprint['state'], note?: string)}
  <button
    class="scope-choice"
    role="menuitemradio"
    aria-checked={isChoice(values)}
    data-scope={values.join(',') || 'all'}
    onclick={() => choose(values)}
    ><SprintMarker {state} /><span class="scope-choice-name">{text}</span>{#if note}<span class="scope-choice-note"
        >{note}</span
      >{/if}</button
  >
{/snippet}

{#snippet progress()}
  <span class="sprint-progress" title="{counts.done} of {counts.total} tasks done ({percent}%)">
    <span class="sprint-bar" aria-hidden="true"><span style:width="{percent}%"></span></span>
    {counts.done} of {counts.total} done
  </span>
  {#if dashboard.narrowed}
    <span class="scope-filtered" id="scope-filtered" title="Filtered by {narrowing}"
      >showing {dashboard.filtered.length}, filtered: <span class="scope-filters">{narrowing}</span><button
        class="chip-clear"
        aria-label="Clear the other filters and the search"
        title="Clear the other filters and the search"
        onclick={() => dashboard.clearNarrowing()}><Icon name="x" size={12} /></button
      ></span
    >
  {/if}
{/snippet}

<!-- Open tasks left in closed sprints (T-323): the link shows those sprints, where they are the only open cards -->
{#snippet leftoverNote()}
  {#if dashboard.leftovers.length}
    <span class="scope-leftovers" id="scope-leftovers"
      ><Icon name="info" size={13} />
      <button class="link-button" id="scope-show-leftovers" onclick={() => dashboard.setSprintScope(leftoverSprints)}
        >{tasks(dashboard.leftovers.length)} open in closed sprints</button
      >&nbsp;· no loop takes {them} · move {them} with <code>/atm:plan-sprint</code></span
    >
  {/if}
{/snippet}

{#if scope.kind !== 'none'}
  <section
    class="scope-bar"
    id="scope-bar"
    data-scope={scope.kind}
    data-state={scope.kind === 'sprint' ? scope.sprint.state : undefined}
    aria-label="Sprint"
  >
    <div class="scope-head">
      <span class="menu-anchor" bind:this={root}>
        <button
          class="scope-trigger"
          id="scope-trigger"
          bind:this={trigger}
          aria-haspopup="menu"
          aria-expanded={menuOpen}
          title="Choose the sprint to show"
          onclick={toggleMenu}
          >{#if scope.kind === 'sprint'}<SprintMarker state={scope.sprint.state} />{/if}<strong class="sprint-name"
            >{label}</strong
          ><Icon name="chevron-down" size={14} /></button
        >
        {#if menuOpen}
          <div
            class="menu scope-menu"
            id="scope-menu"
            role="menu"
            aria-label="Sprints"
            tabindex="-1"
            bind:this={menu}
            onkeydown={onKey}
          >
            {#if active}{@render choice([ACTIVE], active.id, 'active', tasks(sizeOf(active)))}{/if}
            {#if planned.length}
              <div class="menu-label">Planned</div>
              {#each planned as s (s.id)}{@render choice([s.id], s.id, 'planned', tasks(sizeOf(s)))}{/each}
            {/if}
            <div class="menu-label">Everything</div>
            {#if backlog(model, dashboard.tasks).length}{@render choice(
                [BACKLOG],
                'Backlog',
                undefined,
                tasks(backlog(model, dashboard.tasks).length),
              )}{/if}
            {@render choice([], 'All tasks')}
            {#if closed.length}
              <div class="menu-label">Closed</div>
              {#each closed as s (s.id)}{@render choice([s.id], s.id, 'closed', dateOf(s.end))}{/each}
            {/if}
            <div class="menu-note">Several at once: the Sprint filter</div>
          </div>
        {/if}
      </span>
      {#if scope.kind === 'sprint'}
        {#if scope.sprint.state !== 'active'}<span class="sprint-state">{scope.sprint.state}</span>{/if}
        {#if scope.sprint.goal}<span class="sprint-goal" title={scope.sprint.goal}>{scope.sprint.goal}</span>{/if}
      {:else if scope.kind === 'several'}
        <span class="sprint-goal" title={scope.names.join(', ')}>{scope.names.join(', ')}</span>
      {/if}
      {#if dashboard.tab === 'kanban'}
        <div class="scope-options">
          {#if scope.kind === 'all' || scope.kind === 'several'}
            <label class="kanban-option"
              ><input
                type="checkbox"
                id="kanban-group-by-sprint"
                checked={dashboard.kanbanGroupBySprint}
                onchange={(e) => (dashboard.kanbanGroupBySprint = e.currentTarget.checked)}
              /> Group by sprint</label
            >
          {/if}
          <label class="kanban-option"
            ><input
              type="checkbox"
              id="kanban-show-skipped"
              checked={dashboard.kanbanShowSkipped}
              onchange={(e) => (dashboard.kanbanShowSkipped = e.currentTarget.checked)}
            /> Show skipped</label
          >
        </div>
      {:else if dashboard.tab === 'list'}
        <div class="scope-options"><ListOptions /></div>
      {/if}
    </div>

    <div class="scope-facts">
      {#if scope.kind === 'sprint' && summary}
        {@const s = scope.sprint}
        {#if s.state === 'closed'}
          {#if s.start || s.end}<span class="sprint-dates">{dateOf(s.start) ?? '…'} – {dateOf(s.end) ?? '…'}</span>{/if}
          {#if s.release}<span class="sprint-release"
              >{releasedIds.has(s.release) ? 'released' : 'release'} {s.release}</span
            >{/if}
          {@render progress()}
          {#if counts.total - counts.done}<span class="scope-leftovers" id="scope-leftovers"
              ><Icon name="info" size={13} />
              {counts.total - counts.done} open after closing · no loop takes {counts.total - counts.done === 1
                ? 'it'
                : 'them'} · move {counts.total - counts.done === 1 ? 'it' : 'them'} with
              <code>/atm:plan-sprint</code></span
            >{/if}
          {#if s.report?.carried?.length}<span class="sprint-carried"
              >{s.report.carried.length} carried to {s.report.to === BACKLOG || !s.report.to
                ? 'the backlog'
                : s.report.to}</span
            >{/if}
          {#if summary.added}<span class="sprint-added">{summary.added} added mid-sprint</span>{/if}
        {:else if s.state === 'planned'}
          <span class="sprint-size">{tasks(counts.total)}</span>
          {#if s.release}<span class="sprint-release">release {s.release}</span>{/if}
          {#if s.due}<span class="sprint-due-date">due {dateOf(s.due + 'T12:00:00')}</span>{/if}
          <!-- A project not migrated yet reads its phases as planned sprints, some of them under way -->
          {#if counts.done + counts.active + counts.review + counts.blocked}{@render progress()}{/if}
          <!-- atm starts no sprint while another is active: that one closes first -->
          {#if !model.derived && active}<span class="scope-hint" id="scope-hint"
              >starts once {active.id} is closed · <code>/atm:close-sprint</code> then
              <code>/atm:plan-sprint</code></span
            >{:else if !model.derived}<span class="scope-hint" id="scope-hint"
              >start it with <code>/atm:plan-sprint</code></span
            >{/if}
        {:else if counts.total === 0}
          <span class="scope-hint" id="scope-hint"
            >No tasks yet · pull some from the backlog ({tasks(backlogOpen)}) with <code>/atm:plan-sprint</code></span
          >
        {:else if sprintEnded && counts.review + counts.blocked === 0}
          <span class="scope-done" id="scope-done"><Icon name="check" size={14} /> All {tasks(counts.total)} done</span>
          <span class="scope-hint" id="scope-hint"
            >Close it with <code>/atm:close-sprint</code>{#if s.release}&nbsp;· release {s.release} is ready to ship{/if}</span
          >
          {#if backlogOpen}<span class="scope-hint"
              >More work? Pull from the backlog ({tasks(backlogOpen)}) with <code>/atm:plan-sprint</code></span
            >{/if}
        {:else}
          {#if summary.dueIn !== undefined}
            <span class="sprint-due" class:overdue={summary.dueIn < 0} title="Due {s.due}" data-days={summary.dueIn}
              >{dueText(summary.dueIn)}</span
            >
          {/if}
          {#if s.release}<span class="sprint-release">release {s.release}</span>{/if}
          {@render progress()}
          {#if sprintEnded}
            <span class="scope-hint" id="scope-hint"
              >{[counts.review && `${counts.review} in review`, counts.blocked && `${counts.blocked} blocked`]
                .filter(Boolean)
                .join(', ')}: settle {counts.review + counts.blocked === 1 ? 'it' : 'them'} before closing</span
            >
          {/if}
          {#if summary.added}<span class="sprint-added" title="Pulled into the sprint after it started"
              >{summary.added} added mid-sprint</span
            >{/if}
          {#if summary.carried}<span class="sprint-carried" title="Carried over from an earlier sprint"
              >{summary.carried} carried in</span
            >{/if}
          {#if summary.reviewPending}<span
              class="sprint-review-pending"
              title="Done, waiting for the end-of-sprint review with you (/atm:close-sprint)"
              >{summary.reviewPending} waiting for the sprint review</span
            >{/if}
        {/if}
      {:else if scope.kind === 'backlog'}
        <span class="sprint-size">{tasks(counts.total)} in no sprint</span>
        {@render progress()}
        <span class="scope-hint" id="scope-hint">plan them into a sprint with <code>/atm:plan-sprint</code></span>
        {@render leftoverNote()}
      {:else if scope.kind === 'several'}
        {@render progress()}
      {:else if scope.kind === 'all'}
        {@render progress()}
        {@render leftoverNote()}
        {#if scope.active}
          <span class="scope-hint" id="scope-hint"
            ><SprintMarker state="active" />
            <button class="link-button" id="scope-show-active" onclick={() => dashboard.setSprintScope([ACTIVE])}
              >{scope.active.id}</button
            > is active</span
          >
        {:else if !model.derived}
          <!-- Not for a project not migrated yet: its phases, read as sprints, are never active -->
          <span class="scope-hint" id="scope-hint"
            >No active sprint{#if scope.last}&nbsp;· last: {scope.last.id}{#if scope.last.end}, closed {formatWhen(
                  Date.parse(scope.last.end),
                  dashboard.loadedAt,
                )}{/if}{/if}{#if scope.next}&nbsp;· next planned: {scope.next.id} ({tasks(sizeOf(scope.next))}) · start
              it with <code>/atm:plan-sprint</code>{:else}&nbsp;· plan one with <code>/atm:plan-sprint</code>{/if}</span
          >
        {/if}
      {/if}
    </div>
  </section>
{/if}
