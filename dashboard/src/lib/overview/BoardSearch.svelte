<script lang="ts">
  // The All projects search (T-261) over every project's tasks: ID, name, description, type, labels, notes and the
  // project's name. It sits where a project's search does (ATM-737, docs/sidebar-overhaul-design.md §6): in the title
  // bar, and in the app at the left of the tab's line; the Board and the List search with it (board-search.svelte.ts)
  import { dashboard } from '../state.svelte';
  import Icon from '../ui/Icon.svelte';
</script>

<label class="task-search board-search">
  <Icon name="search" size={14} />
  <input
    type="search"
    id="board-search"
    placeholder="Search all projects’ tasks"
    aria-label="Search all projects' tasks by ID, name, description, type, label, note or project"
    value={dashboard.boardQuery}
    oninput={(e) => dashboard.setBoardQuery(e.currentTarget.value)}
  />
  {#if dashboard.boardQuery}
    <button class="search-clear" aria-label="Clear search" onclick={() => dashboard.setBoardQuery('')}
      ><Icon name="x" size={12} /></button
    >
  {/if}
</label>
