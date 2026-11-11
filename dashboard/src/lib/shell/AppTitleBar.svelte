<script lang="ts">
  // The atm app's title bar (T-513, docs/app-desktop-ui-design.md §2): two rows, drawn by the page, the same on every
  // desktop. The title bar: room for macOS's window buttons, or the page's menus and its own buttons on Windows and
  // Linux, and the page's title (the active tab's once tabs exist, T-514). The toolbar: the view switcher, search and
  // Filters, the needs-you chip, and the ⋯ settings until they become a sheet (T-517). No reload button: View ▸
  // Reload. Dragging either bar moves the window, a double click maximizes it. The browser keeps its own title bar.
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
  import { adjacentView, VIEW_ICONS, VIEW_LABELS, VIEWS, type ViewId } from '../tabs';
  import Filters from '../tasks/Filters.svelte';
  import Icon from '../ui/Icon.svelte';
  import ProjectLogo from '../ui/ProjectLogo.svelte';
  import { needsYouItems } from '../waiting';
  import AppMenus from './AppMenus.svelte';
  import NeedsYouMenu from './NeedsYouMenu.svelte';
  import SettingsMenu from './SettingsMenu.svelte';

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
  const needYou = $derived(needsYouItems(dashboard.tasks, dashboard.vocab, { model: dashboard.sprints }));

  const switches: Partial<Record<ViewId, HTMLButtonElement>> = {};
  function onSwitchKeydown(e: KeyboardEvent) {
    const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = adjacentView(dashboard.tab, step);
    dashboard.selectTab(next);
    switches[next]?.focus();
  }
</script>

<header class="app-header" id="app-header">
  <!-- A mouse-down on the bar's free space drags the window (a pointer gesture, as the system's title bar's) -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="app-titlebar"
    class:system-buttons={shell?.system_window_buttons}
    id="app-titlebar"
    onmousedown={onBarMouseDown}
  >
    {#if shell && !shell.system_menu_bar}<AppMenus />{/if}
    <div class="app-title">
      {#if project}<ProjectLogo {project} size={18} />{/if}
      <h1 id="dashboard-title">{title}</h1>
    </div>
    {#if shell && !shell.system_window_buttons}
      <div class="window-buttons" id="window-buttons">
        <button class="window-button" id="window-minimize" title="Minimize" onclick={() => void minimizeWindow()}
          >─</button
        >
        <button class="window-button" id="window-maximize" title="Maximize" onclick={() => void toggleMaximizeWindow()}
          >□</button
        >
        <button class="window-button close" id="window-close" title="Close" onclick={() => void closeWindow()}>✕</button
        >
      </div>
    {/if}
  </div>
  <!-- The toolbar's free space drags the window too -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="app-toolbar" id="app-toolbar" onmousedown={onBarMouseDown}>
    {#if dashboard.onProject}
      <div class="view-switcher" role="tablist" aria-label="Views">
        {#each VIEWS as id (id)}
          <button
            bind:this={switches[id]}
            class="view-switch"
            id="tab-{id}"
            role="tab"
            aria-selected={dashboard.tab === id}
            aria-controls="panel-{id}"
            tabindex={dashboard.tab === id ? 0 : -1}
            title={VIEW_LABELS[id]}
            onclick={() => dashboard.selectTab(id)}
            onkeydown={onSwitchKeydown}><Icon name={VIEW_ICONS[id]} size={14} />{VIEW_LABELS[id]}</button
          >
        {/each}
      </div>
      {#if dashboard.loaded}
        <Filters />
        <NeedsYouMenu
          items={needYou}
          onopen={(id) => dashboard.showDetail(id)}
          chipId="attention-count"
          menuId="needs-you-menu"
        />
      {/if}
    {/if}
    <div class="app-toolbar-end"><SettingsMenu /></div>
  </div>
</header>

<style>
  .app-header {
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    position: sticky;
    top: 0;
    user-select: none;
    z-index: 20;
  }
  .app-titlebar {
    align-items: center;
    display: flex;
    gap: var(--space-3);
    height: 40px;
    padding: 0 var(--space-2) 0 var(--space-3);
  }
  /* macOS's traffic lights sit over the bar's left end (trafficLightPosition in tauri.conf.json) */
  .app-titlebar.system-buttons {
    --traffic-lights: 80px;
    padding-left: var(--traffic-lights);
  }
  .app-title {
    align-items: center;
    display: flex;
    gap: var(--space-2);
    min-width: 0;
  }
  .app-title h1 {
    font-size: var(--text-md);
    font-weight: 600;
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .window-buttons {
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
    background: var(--surface2);
  }
  .window-button.close:hover {
    background: var(--red);
    color: var(--on-accent);
  }
  .app-toolbar {
    align-items: center;
    display: flex;
    gap: var(--space-3);
    min-height: 40px;
    padding: var(--space-1) var(--space-3);
  }
  .view-switcher {
    background: var(--surface2);
    border-radius: var(--radius-md);
    display: flex;
    gap: var(--space-1);
    padding: var(--space-1);
  }
  .view-switch {
    align-items: center;
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    display: flex;
    font-size: var(--text-sm);
    gap: var(--space-1);
    padding: var(--space-1) var(--space-3);
  }
  .view-switch[aria-selected='true'] {
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 1px 2px color-mix(in srgb, var(--overlay) 30%, transparent);
  }
  .app-toolbar-end {
    margin-left: auto;
  }
</style>
