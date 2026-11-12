<script lang="ts">
  // The title bar every page shares: title, project selector and version chip, the subtitle under them, and the
  // reload button and the ⋯ settings menu pinned at the right. The project page adds its charts (before the controls) and
  // its filters (a second row) through snippets.
  import type { Snippet } from 'svelte';
  import { inApp } from '../api/http';
  import { setWindowTitle } from '../api/shell';
  import { dashboardVersion } from '../release-notes';
  import { compactAt } from '../scroll';
  import { dashboard } from '../state.svelte';
  import ProjectLogo from '../ui/ProjectLogo.svelte';
  import ProjectSelector from './ProjectSelector.svelte';
  import RefreshMenu from './RefreshMenu.svelte';
  import SettingsMenu from './SettingsMenu.svelte';
  import WorkspaceSelector from './WorkspaceSelector.svelte';

  interface Props {
    title: string;
    /** Show the project selector (the landing page hides it while no project is registered) */
    selector?: boolean;
    subtitle: Snippet;
    /** Between the title block and the controls */
    charts?: Snippet;
    /** The second row, below everything else */
    below?: Snippet;
    /** In the atm app, inside the tab (T-513): the tab carries the title, the app's title bar the window's title,
     * and View ▸ Reload the reload button; the subtitle, the charts and the ⋯ stay */
    inTab?: boolean;
  }
  let { title, selector = true, subtitle, charts, below, inTab = false }: Props = $props();

  const version = dashboardVersion();
  // The open project, as api/projects lists it: its logo goes next to the title
  const project = $derived.by(() => {
    const route = dashboard.route;
    return route.page === 'project' ? dashboard.projects.find((p) => p.id === route.id) : undefined;
  });

  $effect(() => {
    if (inTab) return; // the app's title bar sets them
    document.title = title;
    // In the app, the window's own title too: the window switcher's and the taskbar's (T-511)
    if (inApp) void setWindowTitle(title);
  });

  // The title bar stays pinned at the top; once the page scrolls it compacts and casts a shadow
  let scrolled = $state(false);
</script>

<svelte:window onscroll={() => (scrolled = compactAt(window.scrollY))} />

<header class="topbar" class:scrolled class:in-tab={inTab}>
  <div class="title-block">
    <div class="title-row">
      <!-- In the app the tab is labelled with them (T-513); the pickers stay until the sidebar (T-515) -->
      {#if !inTab}
        {#if project}<ProjectLogo {project} size={26} />{/if}
        <h1 id="dashboard-title">{title}</h1>
      {/if}
      {#if selector}<ProjectSelector />{/if}
      <WorkspaceSelector />
      {#if version}
        <button
          class="version-chip"
          class:unseen={dashboard.releaseNotesUnseen}
          id="version-chip"
          title="What's new"
          onclick={() => dashboard.openWhatsNew()}
          >v{version}{#if dashboard.releaseNotesUnseen}<span class="version-new">new</span>{/if}</button
        >
      {/if}
    </div>
    <p class="subtitle" id="subtitle">{@render subtitle()}</p>
  </div>
  {@render charts?.()}
  <div class="topbar-controls">
    {#if !inTab}<RefreshMenu />{/if}
    <SettingsMenu />
  </div>
  {@render below?.()}
</header>
