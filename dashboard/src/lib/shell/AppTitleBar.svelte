<script lang="ts">
  // The atm app's title bar (T-513, docs/app-desktop-ui-design.md §2): one row, drawn by the page, the same on every
  // desktop, as a browser's tab strip. Room for macOS's window buttons, or the page's menus and its own buttons on
  // Windows and Linux, and the tabs: for now the one tab, the page shown, labelled with its project's logo and name
  // (T-514 adds the others). Everything else (the views, search and Filters, the needs-you chip, the ⋯ settings) is
  // inside the tab, the page's own header (AppPageBar). Dragging the bar moves the window, a double click maximizes it.
  // The browser keeps its own title bar.
  import { onMount } from 'svelte';
  import {
    appShell,
    closeWindow,
    minimizeWindow,
    onBarMouseDown,
    setWindowTitle,
    toggleMaximizeWindow,
    type AppShell,
  } from '../api/shell';
  import { dashboard } from '../state.svelte';
  import ProjectLogo from '../ui/ProjectLogo.svelte';
  import AppMenus from './AppMenus.svelte';

  let { title }: { title: string } = $props();

  // Until the app says, neither the platform's buttons nor the page's: nothing drawn twice
  let shell = $state<AppShell | null>(null);
  onMount(() => {
    void appShell().then((s) => (shell = s));
  });

  $effect(() => {
    document.title = title;
    void setWindowTitle(title);
  });

  const project = $derived.by(() => {
    const route = dashboard.route;
    return route.page === 'project' ? dashboard.projects.find((p) => p.id === route.id) : undefined;
  });
</script>

<!-- A mouse-down on the bar's free space drags the window (a pointer gesture, as the system's title bar's) -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<header
  class="app-titlebar"
  class:system-buttons={shell?.system_window_buttons}
  id="app-titlebar"
  onmousedown={onBarMouseDown}
>
  {#if shell && !shell.system_menu_bar}<AppMenus />{/if}
  <div class="app-tabs" role="tablist" aria-label="Tabs">
    <div class="app-tab" role="tab" aria-selected="true" id="app-tab-current">
      {#if project}<ProjectLogo {project} size={16} />{/if}
      <h1 id="dashboard-title">{title}</h1>
    </div>
  </div>
  {#if shell && !shell.system_window_buttons}
    <div class="window-buttons" id="window-buttons">
      <button class="window-button" id="window-minimize" title="Minimize" onclick={() => void minimizeWindow()}
        >─</button
      >
      <button class="window-button" id="window-maximize" title="Maximize" onclick={() => void toggleMaximizeWindow()}
        >□</button
      >
      <button class="window-button close" id="window-close" title="Close" onclick={() => void closeWindow()}>✕</button>
    </div>
  {/if}
</header>

<style>
  /* The window's own title bar: everything on its middle line, macOS's traffic lights too (trafficLightPosition's
     y, 26, centres them at 20 px: macOS makes their strip the buttons' height plus y, and centres them in it) */
  .app-titlebar {
    align-items: center;
    background: var(--surface2);
    border-bottom: 1px solid var(--border);
    display: flex;
    gap: var(--space-3);
    height: 40px;
    padding: 0 var(--space-2) 0 var(--space-3);
    position: sticky;
    top: 0;
    user-select: none;
    z-index: 20;
  }
  /* macOS's traffic lights sit over the bar's left end (trafficLightPosition in tauri.conf.json) */
  .app-titlebar.system-buttons {
    --traffic-lights: 80px;
    padding-left: var(--traffic-lights);
  }
  .app-tabs {
    align-items: center;
    display: flex;
    gap: var(--space-1);
    min-width: 0;
  }
  /* A tab in the title bar, as Notion's: a rounded chip on the bar's middle line, the active one lighter */
  .app-tab {
    align-items: center;
    background: var(--surface);
    border-radius: var(--radius-md);
    display: flex;
    gap: var(--space-2);
    height: 28px;
    max-width: 240px;
    min-width: 0;
    padding: 0 var(--space-3);
  }
  .app-tab h1 {
    font-size: var(--text-sm);
    font-weight: 600;
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .window-buttons {
    align-self: stretch;
    align-items: center;
    display: flex;
    margin-left: auto;
  }
  .window-button {
    background: none;
    border: none;
    color: var(--text);
    font-size: var(--text-sm);
    height: 32px;
    width: 44px;
  }
  .window-button:hover {
    background: var(--surface);
  }
  .window-button.close:hover {
    background: var(--red);
    color: var(--on-accent);
  }
</style>
