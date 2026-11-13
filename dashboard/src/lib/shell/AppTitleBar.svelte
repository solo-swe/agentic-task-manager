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
  import AppMenus from './AppMenus.svelte';
  import SidebarToggle from './SidebarToggle.svelte';

  let { title, end }: { title: string; end?: Snippet } = $props();

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
</script>

<!-- A mouse-down on the bar's free space drags the window (a pointer gesture, as the system's title bar's) -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<header
  class="app-titlebar"
  class:system-buttons={shell?.system_window_buttons && !dashboard.sidebarOpen}
  class:flush={dashboard.sidebarOpen && shell?.system_menu_bar !== false}
  id="app-titlebar"
  onmousedown={onBarMouseDown}
>
  {#if shell && !shell.system_menu_bar}<AppMenus />{/if}
  <!-- Show the sidebar (T-515; View ▸ Show Sidebar): here while it is closed; open, its own top strip has the button
       (T-535) -->
  {#if !dashboard.sidebarOpen}<SidebarToggle />{/if}
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
      <!-- Every tab, chosen or not, by the project in its own address (T-525): its logo and its name as its page
           titles it (kept once shown), so a tab's label doesn't change when it is chosen, while the page still shows
           the tab before -->
      {@const id = tabProject(tab.address)}
      {@const listed = id === null ? undefined : dashboard.projects.find((p) => p.id === id)}
      {@const name = listed ? (dashboard.nameOf(listed.id) ?? listed.name) : id === null ? 'Projects Overview' : title}
      {#if listed}<ProjectLogo project={listed} size={16} />{:else}<Logo size={16} />{/if}
      {#if tab.id === appTabs.active}
        <!-- The tab shown: its name is the page's heading, and the project chooser -->
        <h1 id="dashboard-title">{name}</h1>
        {#if dashboard.projects.length > 0}<span class="app-tab-chooser"><ProjectSelector badge={false} /></span>{/if}
      {:else}
        <span class="app-tab-name">{name}</span>
      {/if}
    {/snippet}
  </TitleBarTabs>
  <!-- At the bar's right end, on the tab's line: the workspace picker, the charts, the ⋯ -->
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
  /* The window's own title bar, from its very top, edge to edge of the area right of the sidebar (out of the page's
     padding, body in app.css): the sidebar is a column of the window's full height, as Notion's, so the tabs move
     with this area and need no animation of their own (T-535). Everything on its middle line, macOS's traffic lights
     too (trafficLightPosition's y, 22, centres them at 20 px, as measured on macOS 26) */
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
  /* With the sidebar open (and no menus of the page's before the tabs), the first tab starts at the sidebar's edge:
     the sidebar's border is its left line, not a second one beside it */
  .app-titlebar.flush {
    padding-left: 0;
  }
  .app-titlebar.flush :global(.title-bar-tab:first-child) {
    border-left: none;
  }
  /* macOS's traffic lights sit over the window's left end (trafficLightPosition in tauri.conf.json): over this bar
     while the sidebar is closed, over the sidebar's top strip while it is open */
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
