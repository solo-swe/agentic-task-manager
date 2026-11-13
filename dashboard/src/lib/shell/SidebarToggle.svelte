<script lang="ts">
  // Show or hide the app's sidebar (T-515; View ▸ Show Sidebar): in the sidebar's top strip while it is open, in the
  // title bar while it is closed (T-535). One at a time, so one ID.
  import { dashboard } from '../state.svelte';
  import Icon from '../ui/Icon.svelte';
</script>

<button
  class="app-sidebar-toggle"
  id="app-sidebar-toggle"
  title={dashboard.sidebarOpen ? 'Hide the sidebar' : 'Show the sidebar'}
  aria-label={dashboard.sidebarOpen ? 'Hide the sidebar' : 'Show the sidebar'}
  aria-pressed={dashboard.sidebarOpen}
  onclick={() => dashboard.toggleSidebar()}><Icon name="panel-left" size={16} /></button
>

<style>
  .app-sidebar-toggle {
    align-items: center;
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    display: inline-flex;
    flex: 0 0 28px;
    height: 28px;
    justify-content: center;
  }
  .app-sidebar-toggle:hover,
  .app-sidebar-toggle[aria-pressed='true'] {
    color: var(--text);
  }
  .app-sidebar-toggle:hover {
    background: var(--surface);
  }
</style>
