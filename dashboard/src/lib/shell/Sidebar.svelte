<script lang="ts">
  // The atm app's sidebar (T-515, docs/app-desktop-ui-design.md §4), the app only: Projects Overview, then the
  // workspace's projects, each with its logo, name, its progress as a thin bar and what needs you as a badge; the one
  // shown marked, missing ones dimmed. A click opens a project in this tab; ⌘-click (Ctrl-click, a middle click) in a
  // new one. The title bar's sidebar button and View ▸ Show Sidebar show and hide it; its edge resizes it; both are kept
  // in the preferences. The project chooser, search and the workspace picker stay in the title bar (Hakan).
  import type { ProjectEntry } from '../load';
  import { getPref, setPref } from '../prefs';
  import { needsYou } from '../projects';
  import { dashboard } from '../state.svelte';
  import { tabActions, wantsNewTab } from '../tab-actions';
  import Icon from '../ui/Icon.svelte';
  import ProjectLogo from '../ui/ProjectLogo.svelte';

  const MIN = 180;
  const MAX = 420;
  const tabs = tabActions();

  let width = $state(Math.min(MAX, Math.max(MIN, getPref('sidebarWidth', 240))));
  const current = $derived(dashboard.route.page === 'project' ? dashboard.route.id : null);
  const onOverview = $derived(dashboard.route.page === 'overview');

  // The page leaves room for it: the title bar and the page shift by its width (app.css, --sidebar-offset)
  $effect(() => {
    const offset = dashboard.sidebarOpen ? `${width}px` : '0px';
    document.documentElement.style.setProperty('--sidebar-offset', offset);
    return () => document.documentElement.style.removeProperty('--sidebar-offset');
  });

  /** A project's progress as its overview card counts it (T-533): the active sprint's, else the whole project's (the
   * skipped tasks counted as neither), with what it counts in words */
  function progressOf(project: ProjectEntry): { percent: number; text: string } {
    const s = project.summary;
    if (!s) return { percent: 0, text: 'Progress not known yet' };
    const sprint = s.active_sprint;
    if (sprint) {
      return {
        percent: sprint.percent,
        text: sprint.total
          ? `${sprint.id}: ${sprint.done} of ${sprint.total} done (${sprint.percent}%)`
          : `${sprint.id}: no tasks yet`,
      };
    }
    const counted = s.total - s.skipped;
    return {
      percent: s.percent,
      text: counted ? `${s.done} of ${counted} done (${s.percent}%)` : 'No tasks yet',
    };
  }

  /** A project (the overview for null): in a new tab for ⌘-click, Ctrl-click or a middle click, else in this one */
  function open(e: MouseEvent, id: string | null) {
    if (wantsNewTab(e)) {
      e.preventDefault();
      tabs.open(dashboard.projectAddress(id));
    } else if (e.button === 0) dashboard.openProject(id);
  }

  // The edge: dragged, or the arrows with it focused, resizes the sidebar
  function resize(to: number) {
    width = Math.min(MAX, Math.max(MIN, Math.round(to)));
    setPref('sidebarWidth', width);
  }
  function onpointerdown(e: PointerEvent) {
    const edge = e.currentTarget as HTMLElement;
    edge.setPointerCapture(e.pointerId);
    const move = (m: PointerEvent) => resize(m.clientX);
    const up = () => {
      edge.removeEventListener('pointermove', move);
      edge.removeEventListener('pointerup', up);
    };
    edge.addEventListener('pointermove', move);
    edge.addEventListener('pointerup', up);
  }
  function onkeydown(e: KeyboardEvent) {
    const step = e.key === 'ArrowLeft' ? -16 : e.key === 'ArrowRight' ? 16 : 0;
    if (step) {
      e.preventDefault();
      resize(width + step);
    }
  }
</script>

{#if dashboard.sidebarOpen}
  <nav class="app-sidebar" id="app-sidebar" aria-label="Projects" style:width="{width}px">
    <ul class="app-sidebar-list">
      <li>
        <button
          class="app-sidebar-item"
          class:current={onOverview}
          aria-current={onOverview ? 'page' : undefined}
          data-id="overview"
          onclick={(e) => open(e, null)}
          onauxclick={(e) => open(e, null)}
          ><span class="app-sidebar-icon"><Icon name="layout-grid" size={16} /></span>
          <span class="app-sidebar-name">Projects Overview</span></button
        >
      </li>
      {#each dashboard.projects as project (project.id)}
        {@const waiting = needsYou(project)}
        <li>
          <button
            class="app-sidebar-item"
            class:current={current === project.id}
            class:missing={project.missing}
            aria-current={current === project.id ? 'page' : undefined}
            disabled={project.missing}
            title={project.missing ? `${project.path}: not found` : project.path}
            data-id={project.id}
            onclick={(e) => open(e, project.id)}
            onauxclick={(e) => open(e, project.id)}
          >
            <ProjectLogo {project} size={16} />
            <span class="app-sidebar-name">{project.name}</span>
            {#if waiting}<span class="app-sidebar-badge" title="{waiting} need you">{waiting}</span>{/if}
            {#if !project.missing}
              {@const progress = progressOf(project)}
              <span
                class="project-option-bar app-sidebar-bar"
                role="img"
                aria-label={progress.text}
                title={progress.text}
                data-percent={progress.percent}
                ><span class="project-option-fill" style:width="{progress.percent}%"></span></span
              >
            {/if}
          </button>
        </li>
      {/each}
    </ul>
    <!-- The edge: drag it, or focus it and use the arrows, to resize -->
    <button
      class="app-sidebar-edge"
      id="app-sidebar-edge"
      aria-label="Resize the sidebar ({width} pixels; the arrow keys change it)"
      title="Drag to resize"
      {onpointerdown}
      {onkeydown}
    ></button>
  </nav>
{/if}

<style>
  .app-sidebar {
    background: var(--surface2);
    border-right: 1px solid var(--border);
    bottom: 0;
    left: 0;
    overflow-y: auto;
    padding: var(--space-3) var(--space-2);
    position: fixed;
    /* Under the app's title bar (40px) */
    top: 40px;
    z-index: 30;
  }
  .app-sidebar-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .app-sidebar-item {
    align-items: center;
    background: none;
    border: none;
    border-radius: var(--radius-md);
    color: var(--text);
    display: grid;
    font-size: var(--text-sm);
    gap: var(--space-1) var(--space-2);
    grid-template-columns: auto 1fr auto;
    padding: var(--space-2) var(--space-3);
    text-align: left;
    width: 100%;
  }
  .app-sidebar-item:hover:not(:disabled) {
    background: var(--surface);
  }
  .app-sidebar-item.current {
    background: var(--surface);
    font-weight: 600;
  }
  .app-sidebar-item.missing {
    opacity: 0.5;
  }
  .app-sidebar-icon {
    color: var(--text-muted);
    display: inline-flex;
  }
  .app-sidebar-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .app-sidebar-badge {
    background: var(--accent);
    border-radius: var(--radius-pill);
    color: var(--on-accent);
    font-size: var(--text-2xs);
    font-weight: 600;
    padding: 0 var(--space-2);
  }
  /* The progress bar, the project chooser's, on its own line under the name. Its track is drawn in the border's colour:
     the chooser's (--surface2) is the sidebar's own background, so a project at 0% (a sprint just started) showed no
     bar at all (T-533) */
  .app-sidebar-bar {
    background: var(--border);
    grid-column: 2 / -1;
    width: 100%;
  }
  .app-sidebar-edge {
    background: none;
    border: none;
    bottom: 0;
    padding: 0;
    cursor: col-resize;
    position: absolute;
    right: 0;
    top: 0;
    width: 6px;
  }
  .app-sidebar-edge:hover,
  .app-sidebar-edge:focus-visible {
    background: var(--accent);
    outline: none;
  }
</style>
