<script lang="ts">
  // The app's menus drawn in its title bar where the platform has no menu bar of its own (Windows, Linux; T-513,
  // docs/app-desktop-ui-design.md §6): the same definition as macOS's menu bar (the app's `menu` command), Bits UI's
  // menubar. Each item does what the menu bar's would.
  import { Menubar } from 'bits-ui';
  import { menuDefinition, shortcutLabel, type MenuDef, type MenuEntry } from '../api/menu';
  import { closeWindow, minimizeWindow, openWindow } from '../api/shell';
  import { runDrawnAction, runSystemItem, type WindowActions } from '../menu-actions';
  import { dashboard } from '../state.svelte';

  const SYSTEM_LABELS: Record<string, string> = {
    undo: 'Undo',
    redo: 'Redo',
    cut: 'Cut',
    copy: 'Copy',
    paste: 'Paste',
    select_all: 'Select All',
    minimize: 'Minimize',
  };

  const windowActions: WindowActions = {
    openWindow,
    closeWindow,
    minimize: minimizeWindow,
    // The webview's own editing commands, as the system's Edit menu runs them
    exec: (command) => void document.execCommand(command),
  };

  let menus = $state<MenuDef[]>([]);
  // Asked when it shows, and again when what they show checked changes on the page
  $effect(() => {
    void [dashboard.tab, dashboard.filtersOpen, dashboard.includeArchived, dashboard.workspace, dashboard.route.page];
    void menuDefinition().then((defs) => (menus = defs));
  });
</script>

{#snippet entries(list: MenuEntry[])}
  {#each list as entry, i (i)}
    {#if entry.type === 'separator'}
      <Menubar.Separator class="app-menu-separator" />
    {:else if entry.type === 'system'}
      {#if SYSTEM_LABELS[entry.item]}
        <Menubar.Item class="app-menu-item" onSelect={() => runSystemItem(entry.item, windowActions)}>
          {SYSTEM_LABELS[entry.item]}
        </Menubar.Item>
      {/if}
    {:else if entry.type === 'submenu'}
      <Menubar.Sub>
        <Menubar.SubTrigger class="app-menu-item">{entry.label} ▸</Menubar.SubTrigger>
        <Menubar.SubContent class="app-menu-content">{@render entries(entry.entries)}</Menubar.SubContent>
      </Menubar.Sub>
    {:else if entry.checked !== null}
      <Menubar.CheckboxItem
        class="app-menu-item"
        checked={entry.checked}
        onSelect={() => runDrawnAction(entry.action, dashboard, windowActions)}
      >
        <span class="app-menu-check" aria-hidden="true">{entry.checked ? '✓' : ''}</span>{entry.label}
        <span class="app-menu-key">{shortcutLabel(entry.accelerator)}</span>
      </Menubar.CheckboxItem>
    {:else}
      <Menubar.Item class="app-menu-item" onSelect={() => runDrawnAction(entry.action, dashboard, windowActions)}>
        <span class="app-menu-check" aria-hidden="true"></span>{entry.label}
        <span class="app-menu-key">{shortcutLabel(entry.accelerator)}</span>
      </Menubar.Item>
    {/if}
  {/each}
{/snippet}

<Menubar.Root class="app-menus" id="app-menus">
  {#each menus as def (def.label)}
    <Menubar.Menu>
      <Menubar.Trigger class="app-menu-trigger">{def.label}</Menubar.Trigger>
      <Menubar.Content class="app-menu-content" align="start" sideOffset={4}>
        {@render entries(def.entries)}
      </Menubar.Content>
    </Menubar.Menu>
  {/each}
</Menubar.Root>

<style>
  :global(.app-menus) {
    display: flex;
    gap: var(--space-1);
  }
  :global(.app-menu-trigger) {
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text);
    font-size: var(--text-sm);
    padding: var(--space-1) var(--space-3);
  }
  :global(.app-menu-trigger:hover),
  :global(.app-menu-trigger[data-state='open']) {
    background: var(--surface2);
  }
  :global(.app-menu-content) {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: 0 6px 18px color-mix(in srgb, var(--overlay) 45%, transparent);
    min-width: 220px;
    padding: var(--space-1);
    z-index: 100;
  }
  :global(.app-menu-item) {
    align-items: center;
    border-radius: var(--radius-sm);
    color: var(--text);
    cursor: default;
    display: flex;
    font-size: var(--text-sm);
    gap: var(--space-2);
    padding: var(--space-1) var(--space-3);
  }
  :global(.app-menu-item[data-highlighted]) {
    background: var(--accent);
    color: var(--bg);
  }
  :global(.app-menu-check) {
    display: inline-block;
    width: 1em;
  }
  :global(.app-menu-key) {
    color: var(--text-muted);
    margin-left: auto;
    padding-left: var(--space-5);
  }
  :global(.app-menu-separator) {
    background: var(--border);
    height: 1px;
    margin: var(--space-1) 0;
  }
</style>
