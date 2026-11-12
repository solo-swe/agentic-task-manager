<script lang="ts">
  // In the atm app (T-513): the controls the browser's title bar has, inside the tab instead, at the right of the
  // page's own header: on a project's page search and Filters and the needs-you chip; on every page the ⋯ settings
  // (until they become a sheet, T-517).
  import { dashboard } from '../state.svelte';
  import Filters from '../tasks/Filters.svelte';
  import { needsYouItems } from '../waiting';
  import NeedsYouMenu from './NeedsYouMenu.svelte';
  import SettingsMenu from './SettingsMenu.svelte';

  const needYou = $derived(needsYouItems(dashboard.tasks, dashboard.vocab, { model: dashboard.sprints }));
</script>

<div class="app-page-controls" id="app-page-controls">
  {#if dashboard.onProject && dashboard.loaded}
    <NeedsYouMenu
      items={needYou}
      onopen={(id) => dashboard.showDetail(id)}
      chipId="attention-count"
      menuId="needs-you-menu"
    />
    <Filters />
  {/if}
  <SettingsMenu />
</div>

<style>
  .app-page-controls {
    align-items: center;
    display: flex;
    gap: var(--space-3);
    margin-left: auto;
  }
</style>
