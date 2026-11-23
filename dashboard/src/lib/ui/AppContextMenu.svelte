<script lang="ts">
  // atm's right-click menus in the app (ATM-699, docs/tabs-and-navigation-design.md §4.5; the rules in
  // lib/context-menus.ts): one menu for the window, opened where it was right-clicked. WebKit's own menu is turned off,
  // except in a field you type in (the system's Cut, Copy and Paste) and where something already answered the click
  // (the Back and Forward arrows' history lists); selected text gets atm's menu, with Copy. Bits UI's menu (T-431): the arrows move, Enter chooses, Escape or a click outside
  // closes. The app only: the routes' layout mounts it there.
  import { DropdownMenu } from 'bits-ui';
  import { appTabs } from '../app-tabs.svelte';
  import { openWindow } from '../api/shell';
  import { menuItems, menuTargetAt, type MenuDeps, type MenuItem } from '../context-menus';
  import { pageActions } from '../menu-actions';
  import { currentAddress, navigate } from '../navigation';
  import { tabActions } from '../tab-actions';

  const tabs = tabActions();
  const deps: MenuDeps = {
    tabs: () => appTabs.tabs,
    go: (address) => navigate(address),
    openTab: (address, background) => tabs.open(address, background),
    closeTab: (id) => tabs.close(id),
    openWindow,
    copy: pageActions.copy,
    canGo: (delta) => appTabs.canGo(delta),
    back: pageActions.back,
    forward: pageActions.forward,
    reload: pageActions.reload,
  };

  let open = $state(false);
  let items = $state<MenuItem[]>([]);
  /** Where it was right-clicked: the menu's anchor, a point */
  let at = $state({ x: 0, y: 0 });
  const anchor = {
    getBoundingClientRect: () => DOMRect.fromRect({ x: at.x, y: at.y, width: 0, height: 0 }),
  };

  function oncontextmenu(e: MouseEvent) {
    // Something answered it already (a menu of its own): it stays theirs
    if (e.defaultPrevented) return;
    const target = menuTargetAt(e.target as Element | null, currentAddress(), getSelection()?.toString() ?? '');
    const listed = menuItems(target, deps);
    // In a field you type in: the system's menu, with its text items
    if (listed.length === 0) return;
    e.preventDefault();
    at = { x: e.clientX, y: e.clientY };
    items = listed;
    open = true;
  }
</script>

<svelte:window {oncontextmenu} />

<DropdownMenu.Root bind:open>
  <DropdownMenu.Portal>
    <DropdownMenu.Content customAnchor={anchor} side="bottom" align="start" preventScroll={false} aria-label="Menu">
      {#snippet child({ props, wrapperProps })}
        <div {...wrapperProps}>
          <div {...props} class="app-context-menu" id="app-context-menu">
            {#each items as item (item.id)}
              {#if item.separated}<div class="app-context-separator" role="separator"></div>{/if}
              <DropdownMenu.Item disabled={item.disabled} onSelect={item.run}>
                {#snippet child({ props: entry })}
                  <div {...entry} class="app-context-item" data-item={item.id}>{item.label}</div>
                {/snippet}
              </DropdownMenu.Item>
            {/each}
          </div>
        </div>
      {/snippet}
    </DropdownMenu.Content>
  </DropdownMenu.Portal>
</DropdownMenu.Root>

<style>
  /* As the history arrows' lists (HistoryArrows.svelte) */
  .app-context-menu {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: 0 8px 24px color-mix(in srgb, var(--overlay) 60%, transparent);
    display: flex;
    flex-direction: column;
    min-width: 200px;
    padding: var(--space-2);
    /* Above every layer, the details panel's (100) and the sheets' (110) too: it opens over whatever was right-clicked
       (Bits UI gives its wrapper this z-index) */
    z-index: 200;
  }
  .app-context-item {
    border-radius: var(--radius-sm);
    color: var(--text);
    cursor: default;
    font-size: var(--text-sm);
    padding: var(--space-2) var(--space-4);
    white-space: nowrap;
  }
  .app-context-item[data-highlighted] {
    background: var(--surface2);
  }
  .app-context-item[data-disabled] {
    color: var(--text-muted);
    opacity: 0.6;
  }
  .app-context-separator {
    border-top: 1px solid var(--border);
    margin: var(--space-2) 0;
  }
</style>
