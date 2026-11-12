<script lang="ts">
  // The atm app's title bar (T-513, docs/app-desktop-ui-design.md §2): one row, drawn by the page, the same on every
  // desktop, as a browser's tab strip. Room for macOS's window buttons, or the page's menus and its own buttons on
  // Windows and Linux, and the tabs: for now the one tab, the page shown, labelled with its project's logo and name
  // (T-514 adds the others), and a +. Everything else is inside the tab: the page's own header (TitleBar, `inTab`)
  // with its subtitle, search and Filters, Progress and Activity, and the ⋯ settings. Dragging the bar moves the
  // window, a double click maximizes it. The browser keeps its own title bar.
  import { onMount, type Snippet } from 'svelte';
  import ProjectSelector from './ProjectSelector.svelte';
  import {
    appShell,
    closeWindow,
    minimizeWindow,
    onBarMouseDown,
    openWindow,
    setWindowTitle,
    toggleMaximizeWindow,
    type AppShell,
  } from '../api/shell';
  import { dashboard } from '../state.svelte';
  import Logo from '../ui/Logo.svelte';
  import ProjectLogo from '../ui/ProjectLogo.svelte';
  import AppMenus from './AppMenus.svelte';

  let { title, end }: { title: string; end?: Snippet } = $props();

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
      <!-- A project's logo; on the overview and the other pages, atm's -->
      {#if project}<ProjectLogo {project} size={16} />{:else}<Logo size={16} id="app-tab-logo" />{/if}
      <h1 id="dashboard-title">{title}</h1>
      <!-- The project picker, in the tab's label -->
      {#if dashboard.projects.length > 0}<ProjectSelector badge={false} />{/if}
    </div>
    <!-- A new tab (T-514); until tabs exist, a new window at the overview -->
    <button class="app-tab-new" id="app-tab-new" title="New Tab" onclick={() => void openWindow('/')}>+</button>
  </div>
  <!-- At the bar's right end, on the tab's line: search and Filters, the charts, the ⋯ -->
  <div class="app-titlebar-end" id="app-titlebar-end" data-no-drag>{@render end?.()}</div>
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
  /* The window's own title bar, from its very top and edge to edge (out of the page's padding, body in app.css):
     everything on its middle line, macOS's traffic lights too (trafficLightPosition's y, 22, centres them at 20 px,
     as measured on macOS 26) */
  .app-titlebar {
    align-items: center;
    background: var(--surface2);
    border-bottom: 1px solid var(--border);
    display: flex;
    gap: var(--space-3);
    height: 40px;
    margin: calc(-1 * var(--space-9)) calc(-1 * var(--space-9)) 0;
    padding: 0 var(--space-2) 0 var(--space-3);
    position: sticky;
    top: 0;
    user-select: none;
    /* Over the page's header pinned under it */
    z-index: 50;
  }
  .app-titlebar-end {
    align-items: center;
    display: flex;
    gap: var(--space-3);
    margin-left: auto;
    min-width: 0;
  }
  .app-tab-new {
    align-self: center;
    flex: 0 0 28px;
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    font-size: var(--text-lg);
    height: 28px;
    margin-left: var(--space-1);
    width: 28px;
  }
  .app-tab-new:hover {
    background: var(--surface);
    color: var(--text);
  }
  /* macOS's traffic lights sit over the bar's left end (trafficLightPosition in tauri.conf.json) */
  .app-titlebar.system-buttons {
    --traffic-lights: 88px;
    padding-left: var(--traffic-lights);
  }
  .app-tabs {
    align-self: stretch;
    display: flex;
    min-width: 0;
  }
  /* A tab in the title bar, as Notion's: one fixed width for every tab, the bar's full height, between thin lines;
     the active one in the page's background, over the bar's bottom line, so it joins the page under it. The name
     takes the room left (cut short with …), the project chooser sits at the tab's right end */
  .app-tab {
    align-items: center;
    border-left: 1px solid var(--border);
    border-right: 1px solid var(--border);
    display: flex;
    flex: 0 0 220px;
    gap: var(--space-2);
    --hairline: 1px;
    margin-bottom: calc(-1 * var(--hairline));
    padding: 0 var(--space-2) 0 var(--space-4);
  }
  .app-tab[aria-selected='true'] {
    background: var(--bg);
  }
  .app-tab h1 {
    flex: 1;
    font-size: var(--text-sm);
    font-weight: 600;
    margin: 0;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .window-buttons {
    align-self: stretch;
    align-items: center;
    display: flex;
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
