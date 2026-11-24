<script lang="ts">
  // The app's new tab (`/new`, ATM-755, docs/sidebar-overhaul-design.md §3): Open, a chooser of scope and item. The
  // scopes on the left (All projects, then the workspace's projects, with their logos and what needs you), the chosen
  // scope's items on the right (with their badges). An item opens in this tab, in place of the chooser, which stays
  // its first entry: Back returns to it. Keys: ↑↓ in a column, ←→ between them, typing filters the projects, Enter
  // opens (lib/new-tab.ts). The sidebar's scope is chosen when it opens.
  import { onMount } from 'svelte';
  import type { Item } from '../address';
  import { keyed, scopesOf, type Chooser } from '../new-tab';
  import { navigate } from '../navigation';
  import { ITEM_NAMES } from '../places';
  import { getPref } from '../prefs';
  import { inboxOpen, needsYou, reviewsWaiting } from '../projects';
  import { itemAddress, itemsOf, scopeFromPref, type Scope } from '../sidebar';
  import { dashboard } from '../state.svelte';
  import { ITEM_ICONS } from '../tabs';
  import Icon from '../ui/Icon.svelte';
  import ProjectLogo from '../ui/ProjectLogo.svelte';

  const present = $derived(dashboard.projects.filter((p) => !p.missing));
  let chooser = $state<Chooser>({
    scope: scopeFromPref(getPref('sidebarScope', ''), dashboard.projects),
    item: 0,
    column: 0,
    filter: '',
  });
  const scopes = $derived(scopesOf(present, chooser.filter));
  const items = $derived(itemsOf(chooser.scope));
  const address = (scope: Scope, item: Item) => itemAddress(dashboard.workspace, scope, item) ?? '/';
  const projectOf = (id: string) => present.find((p) => p.id === id);

  /** An item's badge: Code Reviews, the reviews waiting on you (All's sums its projects'); Inbox, the items in it */
  function badgeOf(item: Item): number {
    if (item === 'inbox') return chooser.scope === null ? 0 : inboxOpen(projectOf(chooser.scope));
    if (item !== 'reviews') return 0;
    return chooser.scope === null
      ? present.reduce((n, p) => n + reviewsWaiting(p), 0)
      : reviewsWaiting(projectOf(chooser.scope));
  }

  let root = $state<HTMLElement>();
  onMount(() => root?.focus());

  function onkeydown(e: KeyboardEvent) {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    const done = keyed(chooser, present, e.key);
    if (!done) return;
    e.preventDefault();
    if ('open' in done) navigate(address(chooser.scope, done.open));
    else chooser = done.state;
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (the keys move the chooser while it has focus) -->
<section class="new-tab" id="new-tab" aria-label="Open" tabindex="0" bind:this={root} {onkeydown}>
  <h2 class="new-tab-title">Open</h2>
  {#if chooser.filter}<p class="new-tab-filter" id="new-tab-filter">Projects matching “{chooser.filter}”</p>{/if}
  <div class="new-tab-columns">
    <ul class="new-tab-column" id="new-tab-scopes" aria-label="Scopes" class:keyed={chooser.column === 0}>
      {#each scopes as scope (scope ?? '')}
        {@const project = scope === null ? undefined : projectOf(scope)}
        {@const waiting = project ? needsYou(project) : present.reduce((n, p) => n + needsYou(p), 0)}
        <li>
          <button
            class="new-tab-row"
            class:chosen={chooser.scope === scope}
            data-scope={scope ?? ''}
            aria-pressed={chooser.scope === scope}
            onclick={() => (chooser = { ...chooser, scope, item: 0, column: 0 })}
          >
            {#if project}<ProjectLogo {project} size={18} />{:else}<Icon name="layout-grid" size={18} />{/if}
            <span class="new-tab-name">{project?.name ?? 'All projects'}</span>
            {#if waiting}<span class="new-tab-badge" title="{waiting} need you">{waiting}</span>{/if}
          </button>
        </li>
      {:else}
        <li class="new-tab-none">No project matches</li>
      {/each}
    </ul>
    <ul class="new-tab-column" id="new-tab-items" aria-label="Pages" class:keyed={chooser.column === 1}>
      {#each items as item, i (item)}
        {@const badge = badgeOf(item)}
        <li>
          <!-- A link: in place in this tab (⌘-click, a tab of its own, as every link in the app) -->
          <a class="new-tab-row" class:chosen={chooser.item === i} data-item={item} href={address(chooser.scope, item)}
            ><Icon name={ITEM_ICONS[item]} size={18} /><span class="new-tab-name">{ITEM_NAMES[item]}</span
            >{#if badge}<span class="new-tab-badge" title="{badge} waiting for you">{badge}</span>{/if}</a
          >
        </li>
      {/each}
    </ul>
  </div>
  <p class="new-tab-hint">↑↓ choose · ←→ between the columns · type to find a project · Enter opens</p>
</section>

<style>
  .new-tab {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    max-width: 640px;
    margin: var(--space-9) auto;
    padding: var(--space-7);
    outline: none;
  }
  .new-tab-title {
    margin: 0;
    font-size: var(--text-lg);
  }
  .new-tab-filter,
  .new-tab-hint,
  .new-tab-none {
    margin: 0;
    color: var(--text-muted);
    font-size: var(--text-sm);
  }
  .new-tab-columns {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-6);
  }
  .new-tab-column {
    list-style: none;
    margin: 0;
    padding: var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .new-tab-column.keyed {
    border-color: var(--accent);
  }
  .new-tab-row {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    width: 100%;
    padding: var(--space-4) var(--space-5);
    border: none;
    border-radius: var(--radius-md);
    background: none;
    color: var(--text);
    font: inherit;
    text-align: left;
    text-decoration: none;
    cursor: pointer;
  }
  .new-tab-row:hover {
    background: var(--surface2);
  }
  .new-tab-row.chosen {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .new-tab-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .new-tab-badge {
    padding: 0 var(--space-3);
    border-radius: var(--radius-pill);
    background: var(--purple);
    color: var(--on-accent);
    font-size: var(--text-xs);
    font-weight: 600;
  }
</style>
