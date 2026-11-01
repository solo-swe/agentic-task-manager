<script lang="ts">
  import {
    FILTER_GROUPS,
    facetCounts,
    filterOptions,
    isActive,
    shownSprintOptions,
    type FilterGroup,
  } from '../lib/filters';
  import { hasStage } from '../lib/stages';
  import { dashboard } from '../lib/state.svelte';
  import Icon from './Icon.svelte';
  import SprintMarker from './SprintMarker.svelte';

  // Compact: a search box and what is selected, as chips. The ⌄ opens every filter, one group per line.
  const LABELS: Record<FilterGroup, string> = {
    sprint: 'Sprint',
    release: 'Release',
    component: 'Component',
    type: 'Type',
    status: 'Status',
    priority: 'Priority',
    stage: 'Stage',
    label: 'Labels',
    agent: 'Started by',
  };
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
  const optionsOf = (group: FilterGroup) => (group === 'sprint' ? sprintOptions.shown : options[group]);
  // How many tasks each button would show, given the search and the selections in the other groups
  const counts = $derived(facetCounts(dashboard.searched, dashboard.filters, dashboard.vocab, dashboard.sprints));
  let open = $state(false);

  // The Sprint filter as it applies: "the active sprint" while none is active selects nothing, so it shows no chip
  const applied = $derived({ ...dashboard.filters, sprint: dashboard.sprintSelection });
  const selected = $derived(
    FILTER_GROUPS.filter((g) => applied[g].size > 0).map((g) => ({
      group: g,
      labels: options[g].filter((o) => applied[g].has(o.value)).map((o) => o.label),
    })),
  );

  function countOf(group: FilterGroup, value: string): number {
    return value === 'all' ? counts[group].all : (counts[group].values.get(value) ?? 0);
  }

  function ariaLabel(group: FilterGroup, value: string, label: string, n: number): string {
    const name = value === 'all' ? `All ${PLURALS[group]}` : label;
    return `${name}, ${n} ${n === 1 ? 'task' : 'tasks'}`;
  }
</script>

<div class="filters" id="filters">
  {#if dashboard.loaded}
    <div class="filter-bar" class:open>
      <button
        class="filters-toggle"
        id="filters-toggle"
        aria-expanded={open}
        aria-controls="filter-panel"
        onclick={() => (open = !open)}
        ><Icon name="sliders-horizontal" size={14} /> Filters <Icon name="chevron-down" size={14} /></button
      >
      <label class="task-search">
        <Icon name="search" size={14} />
        <input
          type="search"
          id="task-search"
          placeholder="Search tasks"
          aria-label="Search tasks by ID, name, description, type or label"
          value={dashboard.query}
          oninput={(e) => (dashboard.query = e.currentTarget.value)}
        />
        {#if dashboard.query}
          <button class="search-clear" aria-label="Clear search" onclick={() => (dashboard.query = '')}
            ><Icon name="x" size={12} /></button
          >
        {/if}
      </label>
      <div class="active-filters" id="active-filters">
        {#each selected as s (s.group)}
          <span class="filter-chip" data-filter={s.group}
            ><span class="chip-group">{LABELS[s.group]}:</span>
            {s.labels.join(', ')}<button
              class="chip-clear"
              aria-label="Clear {LABELS[s.group]} filter"
              onclick={() => dashboard.toggleFilter(s.group, 'all')}><Icon name="x" size={12} /></button
            ></span
          >
        {:else}
          <span class="filter-none">No filters</span>
        {/each}
      </div>
      {#if dashboard.archivedCount > 0}
        <label class="include-archived" title="Search, list and graph the archived tasks too (tasks-archive.json)">
          <input
            type="checkbox"
            id="include-archived"
            checked={dashboard.includeArchived}
            onchange={(e) => dashboard.setIncludeArchived(e.currentTarget.checked)}
          />
          Include archived
        </label>
      {/if}
      <span class="match-count" id="match-count" aria-live="polite"
        ><strong>{dashboard.filtered.length}</strong> of {dashboard.base.length} tasks</span
      >
      <button
        class="reset-view-btn"
        id="reset-view"
        title="Forget this project's saved filters, search, tab and options"
        onclick={() => dashboard.resetView()}>Reset view</button
      >
    </div>
    {#if open}
      <div class="filter-panel" id="filter-panel">
        {#each groups as group (group)}
          <div class="filter-group" data-group={group}>
            <div class="filter-group-label">{LABELS[group]}</div>
            <div class="filter-group-buttons">
              {#each [{ value: 'all', label: 'All' }, ...optionsOf(group)] as option (option.value)}
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
                  >{#if 'state' in option}<SprintMarker state={option.state} />{/if}<span class="filter-label"
                    >{option.label}</span
                  ><span class="filter-count" aria-hidden="true">{n}</span></button
                >
              {/each}
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
      </div>
    {/if}
  {/if}
</div>
