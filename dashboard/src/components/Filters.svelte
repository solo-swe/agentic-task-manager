<script lang="ts">
  import { tick } from 'svelte';
  import {
    FILTER_GROUPS,
    FILTER_LABELS,
    facetCounts,
    filterOptions,
    isActive,
    shownSprintOptions,
    type FilterGroup,
    type FilterOption,
  } from '../lib/filters';
  import { hasStage } from '../lib/stages';
  import { dashboard } from '../lib/state.svelte';
  import Icon from './Icon.svelte';
  import SprintMarker from './SprintMarker.svelte';

  // In the title bar (T-323): a compact search box ("/" focuses it) and a Filters button whose badge counts the groups
  // that narrow the board. The button opens a popup: how many tasks match and what is selected, then one line per
  // group, its most used values first and the rest behind "+N", and Include archived and Reset view at the foot. The
  // Sprint choice is the scope bar's; what else narrows the board shows in its "filtered" note.
  const PLURALS: Record<FilterGroup, string> = {
    sprint: 'sprints',
    release: 'releases',
    component: 'components',
    type: 'types',
    status: 'statuses',
    priority: 'priorities',
    stage: 'stages',
    label: 'labels',
    agent: 'agents',
  };
  /** Values a group shows before its "+N": the rest open on demand, and a selected one always shows */
  const SHOWN = 10;
  const options = $derived(filterOptions(dashboard.base, dashboard.vocab, dashboard.sprints));
  // Sprint, Release, Component and Labels only when some task has one; Stage only when some task is in a PR or
  // merged; Started by only when more than one actor started tasks
  const anyStage = $derived(dashboard.base.some(hasStage));
  const groups = $derived(
    FILTER_GROUPS.filter(
      (g) =>
        (!['sprint', 'release', 'component', 'label'].includes(g) || options[g].length > 0) &&
        (g !== 'stage' || anyStage) &&
        (g !== 'agent' || options.agent.length > 1),
    ),
  );
  // With many sprints, the Sprint group lists the open ones (active, planned, the backlog) and any selected one; a
  // toggle shows the closed ones too
  const closedSprints = $derived(
    new Set(dashboard.sprints.sprints.filter((s) => s.state === 'closed').map((s) => s.id)),
  );
  const sprintOptions = $derived(
    shownSprintOptions(options.sprint, closedSprints, dashboard.filters.sprint, dashboard.filterClosedSprints),
  );
  // How many tasks each button would show, given the search and the selections in the other groups
  const counts = $derived(facetCounts(dashboard.searched, dashboard.filters, dashboard.vocab, dashboard.sprints));
  // The Sprint filter as it applies: "the active sprint" while none is active selects nothing, so it shows no chip
  const applied = $derived({ ...dashboard.filters, sprint: dashboard.sprintSelection });
  const selected = $derived(
    FILTER_GROUPS.filter((g) => applied[g].size > 0).map((g) => ({
      group: g,
      labels: options[g].filter((o) => applied[g].has(o.value)).map((o) => o.label),
    })),
  );
  // The badge: the groups that narrow the board within its sprints (the scope bar shows the Sprint choice)
  const badge = $derived(selected.filter((s) => s.group !== 'sprint').length);

  let open = $state(false);
  let root = $state<HTMLElement>();
  let search = $state<HTMLInputElement>();
  // Groups showing all their values, and the text the Labels group is narrowed by
  let expanded = $state<FilterGroup[]>([]);
  let find = $state('');

  function countOf(group: FilterGroup, value: string): number {
    return value === 'all' ? counts[group].all : (counts[group].values.get(value) ?? 0);
  }

  /** A group's values in the popup: All, then the most used first (sprints keep their order), the rest behind "+N" */
  function valuesOf(group: FilterGroup): { shown: FilterOption[]; more: number } {
    let values = group === 'sprint' ? sprintOptions.shown : options[group];
    if (group === 'label' && find.trim()) {
      const q = find.trim().toLowerCase();
      return { shown: values.filter((o) => o.label.toLowerCase().includes(q)), more: 0 };
    }
    if (group === 'label' || group === 'component' || group === 'release' || group === 'agent') {
      values = [...values].sort((a, b) => countOf(group, b.value) - countOf(group, a.value));
    }
    if (group === 'sprint' || expanded.includes(group) || values.length <= SHOWN) return { shown: values, more: 0 };
    const shown = values.filter((o, i) => i < SHOWN || applied[group].has(o.value));
    return { shown, more: values.length - shown.length };
  }

  function ariaLabel(group: FilterGroup, value: string, label: string, n: number): string {
    const name = value === 'all' ? `All ${PLURALS[group]}` : label;
    return `${name}, ${n} ${n === 1 ? 'task' : 'tasks'}`;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && open) {
      open = false;
      return;
    }
    // "/" focuses the search, unless the user is typing somewhere already
    const target = e.target as HTMLElement;
    if (e.key === '/' && !e.metaKey && !e.ctrlKey && !target.closest('input, textarea, select, [contenteditable]')) {
      e.preventDefault();
      search?.focus();
    }
  }

  async function toggle() {
    open = !open;
    if (!open) return;
    await tick();
    root?.querySelector<HTMLElement>('.filter-btn')?.focus();
  }
</script>

<!-- Outside or not by the click's path, as it was dispatched: "clear", "+N" and a chip's × remove themselves when
     clicked, so by now they would no longer be inside the popup -->
<svelte:window
  onkeydown={onKeydown}
  onclick={(e) => open && root && !e.composedPath().includes(root) && (open = false)}
/>

<div class="filters" id="filters">
  {#if dashboard.loaded}
    <label class="task-search">
      <Icon name="search" size={14} />
      <input
        type="search"
        id="task-search"
        placeholder="Search tasks"
        aria-label="Search tasks by ID, name, description, type or label"
        title="Search tasks (/)"
        bind:this={search}
        value={dashboard.query}
        oninput={(e) => (dashboard.query = e.currentTarget.value)}
      />
      {#if dashboard.query}
        <button class="search-clear" aria-label="Clear search" onclick={() => (dashboard.query = '')}
          ><Icon name="x" size={12} /></button
        >
      {:else}
        <kbd class="search-key" aria-hidden="true">/</kbd>
      {/if}
    </label>
    <span class="menu-anchor filter-anchor" bind:this={root}>
      <button
        class="filters-toggle"
        class:filtered={badge > 0}
        id="filters-toggle"
        aria-expanded={open}
        aria-controls="filter-panel"
        title="Filter the tasks"
        onclick={toggle}
        ><Icon name="sliders-horizontal" size={14} /> Filters{#if badge}<span class="filters-badge" id="filters-badge"
            >{badge}</span
          >{/if}
        <Icon name="chevron-down" size={14} /></button
      >
      <!-- Always in the page (hidden while closed), so what it holds stays readable: the counts, the chips -->
      <div class="filter-panel" id="filter-panel" role="dialog" aria-label="Filters" hidden={!open}>
        <div class="filter-panel-head">
          <span class="match-count" id="match-count" aria-live="polite"
            ><strong>{dashboard.filtered.length}</strong> of {dashboard.base.length} tasks</span
          >
          <div class="active-filters" id="active-filters">
            {#each selected as s (s.group)}
              <span class="filter-chip" data-filter={s.group}
                ><span class="chip-group">{FILTER_LABELS[s.group]}:</span>
                {s.labels.join(', ')}<button
                  class="chip-clear"
                  aria-label="Clear {FILTER_LABELS[s.group]} filter"
                  onclick={() => dashboard.toggleFilter(s.group, 'all')}><Icon name="x" size={12} /></button
                ></span
              >
            {:else}
              <span class="filter-none">No filters</span>
            {/each}
          </div>
          <!-- The other filters and the search; the sprints are the scope bar's choice -->
          <button
            class="panel-button"
            id="clear-filters"
            disabled={badge === 0 && !dashboard.query}
            title="Clear the filters and the search (the sprint stays)"
            onclick={() => dashboard.clearNarrowing()}>Clear filters</button
          >
        </div>
        {#each groups as group (group)}
          {@const values = valuesOf(group)}
          <div class="filter-group" data-group={group}>
            <div class="filter-group-label">
              {FILTER_LABELS[group]}
              <!-- A group with a selection can be cleared here (it then shows every value: "all") -->
              {#if applied[group].size > 0}
                <button
                  class="filter-btn filter-all"
                  data-filter={group}
                  data-value="all"
                  aria-label={ariaLabel(group, 'all', 'All', countOf(group, 'all'))}
                  title="Show all {PLURALS[group]}"
                  onclick={() => dashboard.toggleFilter(group, 'all')}>clear</button
                >
              {/if}
            </div>
            <div class="filter-group-buttons">
              {#if group === 'label' && options.label.length > SHOWN}
                <input
                  class="filter-find"
                  id="filter-find-label"
                  type="search"
                  placeholder="Find a label"
                  aria-label="Find a label"
                  bind:value={find}
                />
              {/if}
              {#each values.shown as option (option.value)}
                {@const n = countOf(group, option.value)}
                {@const active = isActive(applied, group, option.value)}
                <button
                  class="filter-btn"
                  class:active
                  class:empty={n === 0 && !active}
                  data-filter={group}
                  data-value={option.value}
                  aria-label={ariaLabel(group, option.value, option.label, n)}
                  aria-pressed={active}
                  onclick={() => dashboard.toggleFilter(group, option.value)}
                  >{#if option.state}<SprintMarker state={option.state} />{/if}<span class="filter-label"
                    >{option.label}</span
                  ><span class="filter-count" aria-hidden="true">{n}</span></button
                >
              {/each}
              {#if values.more}
                <button class="filter-more" data-more={group} onclick={() => (expanded = [...expanded, group])}
                  >+{values.more}</button
                >
              {:else if expanded.includes(group)}
                <button
                  class="filter-more"
                  data-more={group}
                  onclick={() => (expanded = expanded.filter((g) => g !== group))}>Fewer</button
                >
              {/if}
              {#if group === 'sprint' && (dashboard.filterClosedSprints ? sprintOptions.closed > 0 : sprintOptions.hidden > 0)}
                <button
                  class="filter-more"
                  id="filter-closed-sprints"
                  aria-expanded={dashboard.filterClosedSprints}
                  title="Closed sprints"
                  onclick={() => (dashboard.filterClosedSprints = !dashboard.filterClosedSprints)}
                  >{dashboard.filterClosedSprints ? 'Hide closed' : `+ ${sprintOptions.hidden} closed`}</button
                >
              {/if}
            </div>
          </div>
        {/each}
        {#if dashboard.archivedCount > 0}
          <div class="filter-panel-foot">
            <label class="include-archived" title="Search, list and graph the archived tasks too (tasks-archive.json)">
              <input
                type="checkbox"
                id="include-archived"
                checked={dashboard.includeArchived}
                onchange={(e) => dashboard.setIncludeArchived(e.currentTarget.checked)}
              />
              Include archived
            </label>
          </div>
        {/if}
      </div>
    </span>
  {/if}
</div>
