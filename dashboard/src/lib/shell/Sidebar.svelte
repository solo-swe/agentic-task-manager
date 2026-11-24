<script lang="ts">
  // The atm app's sidebar (ATM-753, docs/sidebar-overhaul-design.md §2; T-515 before it), the app only: a scope
  // selector, All projects or one of the workspace's projects, and that scope's items under it. An item always opens a
  // new tab, right of the active one, and goes to it (⌘-click, Ctrl-click or a middle click: in the background); each is
  // a real link, so the right-click link menu (ATM-699) and Copy Link work. The scope follows the active tab's page; a
  // choice by hand holds until the active tab or its page's scope changes, and is kept in the preferences. The title
  // bar's sidebar button and View ▸ Show Sidebar show and hide it; its edge resizes it; both are kept in the preferences.
  import { DropdownMenu } from 'bits-ui';
  import { onMount, untrack } from 'svelte';
  import type { Item } from '../address';
  import { appShell, onBarMouseDown, type AppShell } from '../api/shell';
  import { appTabs } from '../app-tabs.svelte';
  import type { ProjectEntry } from '../load';
  import { ITEM_NAMES } from '../places';
  import { getPref, setPref } from '../prefs';
  import { inboxOpen, needsYou, reviewsWaiting } from '../projects';
  import {
    itemAddress,
    itemsOf,
    scopeFromPref,
    scopePref,
    sidebarScope,
    type Scope,
    type ScopeChoice,
  } from '../sidebar';
  import { dashboard } from '../state.svelte';
  import { inBackground, tabActions, wantsNewTab } from '../tab-actions';
  import { ITEM_ICONS, itemOfOverviewTab, itemOfView } from '../tabs';
  import Icon from '../ui/Icon.svelte';
  import ProjectLogo from '../ui/ProjectLogo.svelte';
  import HistoryArrows from './HistoryArrows.svelte';
  import SidebarToggle from './SidebarToggle.svelte';

  const MIN = 180;
  const MAX = 420;
  const tabs = tabActions();

  let width = $state(Math.min(MAX, Math.max(MIN, getPref('sidebarWidth', 240))));
  let menuOpen = $state(false);

  // The active tab's page's scope: a project's, All's, or none (the landing page's choice, the new tab)
  const pageScope = $derived<Scope | undefined>(
    dashboard.route.page === 'project' ? dashboard.route.id : dashboard.route.page === 'overview' ? null : undefined,
  );
  // The scope chosen by hand, and on what (the preferences' at the start: it holds on the first tab until it changes)
  let choice = $state<ScopeChoice | null>(null);
  // The scope shown last: a page of no scope keeps it (a plain variable: only the scope's own derivation writes it)
  let last: Scope = null;
  let started = false;
  $effect(() => {
    if (started || !dashboard.projectsLoaded) return;
    started = true;
    const saved = getPref('sidebarScope', '');
    if (saved !== '' || pageScope === undefined)
      choice = { scope: scopeFromPref(saved, dashboard.projects), tab: appTabs.active, pageScope };
  });
  const scope = $derived.by(() => (last = sidebarScope(pageScope, appTabs.active, choice, last)));
  // A switch of tab ends a choice by hand (§2.1): back on its tab, the scope follows the page again
  $effect(() => {
    const active = appTabs.active;
    untrack(() => {
      if (choice && choice.tab !== active) choice = null;
    });
  });
  const scoped = $derived(scope === null ? null : dashboard.projects.find((p) => p.id === scope));
  // The item the active tab shows, when its page is the sidebar's scope's
  const currentItem = $derived(
    pageScope !== scope || pageScope === undefined
      ? null
      : pageScope === null
        ? itemOfOverviewTab(dashboard.overviewTab)
        : itemOfView(dashboard.tab),
  );

  function choose(next: Scope) {
    choice = { scope: next, tab: appTabs.active, pageScope };
    setPref('sidebarScope', scopePref(next));
  }

  /** The item's badge: Code Reviews, the reviews waiting on you (the scope's: All's sums its projects'); Inbox, the
   * items waiting in it (a project's, ATM-762) */
  function badgeOf(item: Item): number {
    if (item === 'inbox') return scope === null ? 0 : inboxOpen(scoped ?? undefined);
    if (item !== 'reviews') return 0;
    return scope === null
      ? dashboard.projects.reduce((n, p) => n + reviewsWaiting(p), 0)
      : reviewsWaiting(scoped ?? undefined);
  }

  // Whether macOS's traffic lights sit over its top strip (they are at the window's left end)
  let shell = $state<AppShell | null>(null);
  onMount(() => {
    void appShell().then((s) => (shell = s));
  });

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

  /** An item: always a new tab, shown; ⌘-click, Ctrl-click or a middle click, in the background (⇧ brings it forward) */
  function open(e: MouseEvent, address: string) {
    if (e.button !== 0 && e.button !== 1) return;
    e.preventDefault();
    tabs.open(address, wantsNewTab(e) && inBackground(e));
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

<!-- A scope's logo, name and what needs you, in the selector's button and in its menu -->
{#snippet scopeLabel(project: ProjectEntry | null | undefined, inMenu: boolean)}
  {#if project}
    {@const waiting = needsYou(project)}
    <ProjectLogo {project} size={18} />
    <span class="app-sidebar-name">{project.name}</span>
    {#if waiting}<span class="app-sidebar-badge" title="{waiting} need you">{waiting}</span>{/if}
    {#if !project.missing}
      {@const progress = progressOf(project)}
      <span
        class="project-option-bar app-sidebar-bar"
        class:in-menu={inMenu}
        role="img"
        aria-label={progress.text}
        title={progress.text}
        data-percent={progress.percent}
        ><span class="project-option-fill" style:width="{progress.percent}%"></span></span
      >
    {/if}
  {:else}
    {@const waiting = dashboard.projects.reduce((n, p) => n + needsYou(p), 0)}
    <span class="app-sidebar-icon"><Icon name="layout-grid" size={18} /></span>
    <span class="app-sidebar-name">All projects</span>
    {#if waiting}<span class="app-sidebar-badge" title="{waiting} need you">{waiting}</span>{/if}
  {/if}
{/snippet}

{#if dashboard.sidebarOpen}
  <nav class="app-sidebar" id="app-sidebar" aria-label="Sidebar" style:width="{width}px">
    <!-- Its top strip, the title bar's height (T-535): room for macOS's traffic lights, then the sidebar button; its
         free space drags the window, as the title bar's does -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="app-sidebar-top"
      class:system-buttons={shell?.system_window_buttons}
      id="app-sidebar-top"
      onmousedown={onBarMouseDown}
    >
      <!-- Back and Forward left of the toggle (ATM-695), as in the title bar while the sidebar is closed -->
      <HistoryArrows />
      <SidebarToggle />
    </div>
    <!-- The scope (§2.1): All projects, then the workspace's projects; missing ones dimmed and not chosen. Its list
         is placed under the button (`.menu` is placed under its positioned parent: without this one, the sidebar's
         whole height, below the window) -->
    <div class="menu-anchor app-sidebar-scope-anchor">
      <DropdownMenu.Root bind:open={menuOpen}>
        <DropdownMenu.Trigger>
          {#snippet child({ props })}
            <button
              {...props}
              class="app-sidebar-item app-sidebar-scope"
              id="app-sidebar-scope"
              data-scope={scope ?? ''}
              aria-label="Scope: {scope === null ? 'All projects' : (scoped?.name ?? scope)}; choose another"
              >{@render scopeLabel(scope === null ? null : scoped, false)}<span class="app-sidebar-chevron"
                ><Icon name="chevron-down" size={14} /></span
              ></button
            >
          {/snippet}
        </DropdownMenu.Trigger>
        <DropdownMenu.ContentStatic preventScroll={false} aria-label="Scopes">
          {#snippet child({ props })}
            <span {...props} class="menu app-sidebar-scopes" id="app-sidebar-scopes">
              <DropdownMenu.Item onSelect={() => choose(null)}>
                {#snippet child({ props })}
                  <button
                    {...props}
                    class="app-sidebar-item"
                    class:current={scope === null}
                    aria-current={scope === null ? 'true' : undefined}
                    data-scope="">{@render scopeLabel(null, true)}</button
                  >
                {/snippet}
              </DropdownMenu.Item>
              {#each dashboard.projects as project (project.id)}
                <DropdownMenu.Item disabled={project.missing} onSelect={() => choose(project.id)}>
                  {#snippet child({ props })}
                    <button
                      {...props}
                      class="app-sidebar-item"
                      class:current={scope === project.id}
                      class:missing={project.missing}
                      aria-current={scope === project.id ? 'true' : undefined}
                      title={project.missing ? `${project.path}: not found` : project.path}
                      data-scope={project.id}>{@render scopeLabel(project, true)}</button
                    >
                  {/snippet}
                </DropdownMenu.Item>
              {/each}
            </span>
          {/snippet}
        </DropdownMenu.ContentStatic>
      </DropdownMenu.Root>
    </div>
    <!-- The scope's items (§2.2): each a link to its page, opened in a new tab -->
    <ul class="app-sidebar-list" aria-label="{scope === null ? 'All projects' : (scoped?.name ?? scope)}: pages">
      {#each itemsOf(scope) as item (item)}
        {@const address = itemAddress(dashboard.workspace, scope, item)}
        {@const badge = badgeOf(item)}
        {#if address}
          <li>
            <a
              class="app-sidebar-item app-sidebar-link"
              class:current={currentItem === item}
              aria-current={currentItem === item ? 'page' : undefined}
              href={address}
              data-item={item}
              onclick={(e) => open(e, address)}
              onauxclick={(e) => open(e, address)}
              ><span class="app-sidebar-icon"><Icon name={ITEM_ICONS[item]} size={16} /></span>
              <span class="app-sidebar-name">{ITEM_NAMES[item]}</span>
              {#if badge}<span class="app-sidebar-badge" title="{badge} waiting for you">{badge}</span>{/if}</a
            >
          </li>
        {/if}
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
  /* A column of the window's full height, as Notion's (T-535): the title bar is only over the area right of it */
  .app-sidebar {
    background: var(--surface2);
    border-right: 1px solid var(--border);
    bottom: 0;
    left: 0;
    overflow-y: auto;
    padding: 0 var(--space-2) var(--space-3);
    position: fixed;
    top: 0;
    z-index: 30;
  }
  /* Its top strip: the title bar's height and colour, the button on its middle line, pinned while the list scrolls */
  .app-sidebar-top {
    align-items: center;
    background: var(--surface2);
    display: flex;
    height: 40px;
    margin: 0 calc(-1 * var(--space-2)) var(--space-3);
    padding: 0 var(--space-3);
    position: sticky;
    top: 0;
    user-select: none;
    z-index: 1;
  }
  /* macOS's traffic lights sit over the strip's left end (trafficLightPosition in tauri.conf.json) */
  .app-sidebar-top.system-buttons {
    padding-left: 88px;
  }
  .app-sidebar-list {
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    list-style: none;
    margin: var(--space-3) 0 0;
    padding: var(--space-3) 0 0;
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
    text-decoration: none;
    width: 100%;
  }
  .app-sidebar-item:hover:not(:disabled),
  .app-sidebar-item[data-highlighted] {
    background: var(--surface);
  }
  .app-sidebar-item.current {
    background: var(--surface);
    font-weight: 600;
  }
  .app-sidebar-item.missing {
    opacity: 0.5;
  }
  /* The scope: a little larger, the chevron at its right end */
  .app-sidebar-scope {
    font-size: var(--text-md);
    font-weight: 600;
    grid-template-columns: auto 1fr auto auto;
  }
  .app-sidebar-chevron {
    color: var(--text-muted);
    display: inline-flex;
    grid-column: -2;
    grid-row: 1;
  }
  /* Above the list of items it opens over */
  .app-sidebar-scope-anchor {
    z-index: 2;
  }
  .app-sidebar-scopes {
    display: flex;
    left: 0;
    right: 0;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 220px;
    padding: var(--space-2);
  }
  .app-sidebar-icon {
    color: var(--text-muted);
    display: inline-flex;
  }
  .app-sidebar-link.current .app-sidebar-icon {
    color: var(--accent);
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
