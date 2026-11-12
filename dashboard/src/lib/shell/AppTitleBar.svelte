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
    setWindowTitle,
    toggleMaximizeWindow,
    type AppShell,
  } from '../api/shell';
  import { dashboard } from '../state.svelte';
  import Logo from '../ui/Logo.svelte';
  import ProjectLogo from '../ui/ProjectLogo.svelte';
  import { appTabs, tabProject } from '../app-tabs.svelte';
  import { tabActions } from '../tab-actions';
  import TitleBarTabs from '../ui/TitleBarTabs.svelte';
  import Icon from '../ui/Icon.svelte';
  import AppMenus from './AppMenus.svelte';

  let { title, start, end }: { title: string; start?: Snippet; end?: Snippet } = $props();

  const tabs = tabActions();

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
  <!-- Show or hide the sidebar (T-515; View ▸ Show Sidebar) -->
  <button
    class="app-sidebar-toggle"
    id="app-sidebar-toggle"
    title={dashboard.sidebarOpen ? 'Hide the sidebar' : 'Show the sidebar'}
    aria-label={dashboard.sidebarOpen ? 'Hide the sidebar' : 'Show the sidebar'}
    aria-pressed={dashboard.sidebarOpen}
    onclick={() => dashboard.toggleSidebar()}><Icon name="panel-left" size={16} /></button
  >
  <!-- After the sidebar's button, before the tabs: a project's search and Filters (Hakan) -->
  {#if start}<div class="app-titlebar-start" id="app-titlebar-start" data-no-drag>{@render start()}</div>{/if}
  <!-- The window's tabs (T-514), each an address (lib/app-tabs.svelte.ts), in the reusable strip (ui/TitleBarTabs) -->
  <TitleBarTabs
    tabs={appTabs.tabs}
    active={appTabs.active}
    idPrefix="app-tab-"
    onselect={tabs.select}
    onclose={(id) => tabs.close(id)}
    onnew={() => tabs.open()}
    onmove={tabs.move}
  >
    {#snippet label(tab)}
      {#if tab.id === appTabs.active}
        <!-- The page shown: its project's logo (atm's on the other pages), its title, and the project chooser -->
        {#if project}<ProjectLogo {project} size={16} />{:else}<Logo size={16} />{/if}
        <h1 id="dashboard-title">{title}</h1>
        {#if dashboard.projects.length > 0}<span class="app-tab-chooser"><ProjectSelector badge={false} /></span>{/if}
      {:else}
        {@const other = dashboard.projects.find((p) => p.id === tabProject(tab.address))}
        {#if other}<ProjectLogo project={other} size={16} />{:else}<Logo size={16} />{/if}
        <span class="app-tab-name">{other?.name ?? 'Projects Overview'}</span>
      {/if}
    {/snippet}
  </TitleBarTabs>
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
    /* Over the sidebar too, from the window's left edge */
    margin-left: calc(-1 * (var(--space-9) + var(--sidebar-offset, 0px)));
    padding: 0 var(--space-2) 0 var(--space-3);
    position: sticky;
    top: 0;
    user-select: none;
    /* Over the page's header pinned under it */
    z-index: 50;
  }
  .app-titlebar-start {
    align-items: center;
    display: flex;
    flex: 0 0 auto;
  }
  .app-titlebar-start:empty {
    display: none;
  }
  .app-titlebar-end {
    align-items: center;
    display: flex;
    gap: var(--space-3);
    margin-left: auto;
    min-width: 0;
  }
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
  /* macOS's traffic lights sit over the bar's left end (trafficLightPosition in tauri.conf.json) */
  .app-titlebar.system-buttons {
    --traffic-lights: 88px;
    padding-left: var(--traffic-lights);
  }
  /* atm's tab label: the name takes the room left (cut short with …), the chooser sits at its right end */
  h1,
  .app-tab-name {
    flex: 1;
    font-size: var(--text-sm);
    font-weight: 600;
    margin: 0;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* The project chooser in the tab: a quiet chevron, no border or background (the browser's keeps its own) */
  .app-tab-chooser :global(.project-switch) {
    background: none;
    border: none;
    height: 22px;
    width: 22px;
  }
  .app-tab-chooser :global(.project-switch:hover),
  .app-tab-chooser :global(.project-switch[aria-expanded='true']) {
    background: var(--surface);
    border-radius: var(--radius-sm);
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
