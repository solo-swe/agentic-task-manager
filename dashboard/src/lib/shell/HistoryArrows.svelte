<script lang="ts">
  // The app's Back and Forward arrows (ATM-695, docs/tabs-and-navigation-design.md §4.5): left of the sidebar toggle,
  // in the title bar while the sidebar is closed and in its top strip while it is open (Hakan, CR-63), as Safari,
  // Linear and Notion have them. They walk the active tab's own history (ATM-694), each disabled at its end; a
  // right-click or a long press lists the entries that way, nearest first, to jump to one. The app only: the browser
  // has its own buttons.
  import { ContextMenu } from 'bits-ui';
  import { appTabs } from '../app-tabs.svelte';
  import { historyItems } from '../history-list';
  import { back, forward } from '../navigation';
  import { dashboard } from '../state.svelte';
  import Icon from '../ui/Icon.svelte';

  /** How long a press lasts before it lists the entries instead of stepping (a mouse's; Bits UI times a touch's) */
  const LONG_PRESS_MS = 500;

  const history = $derived(appTabs.current?.history);
  const projectName = (id: string) =>
    dashboard.nameOf(id) ?? dashboard.projects.find((p) => p.id === id)?.name ?? undefined;
  const lists = $derived({
    back: historyItems(history, -1, projectName),
    forward: historyItems(history, 1, projectName),
  });

  // A long press with the mouse opens the list where it was pressed, as a right-click does; the click it ends with
  // then steps nowhere
  let pressTimer: ReturnType<typeof setTimeout> | null = null;
  let pressed = false;

  function onpointerdown(e: PointerEvent) {
    pressed = false;
    if (e.pointerType !== 'mouse' || e.button !== 0) return;
    const target = e.currentTarget as HTMLElement;
    const { clientX, clientY } = e;
    pressTimer = setTimeout(() => {
      pressTimer = null;
      pressed = true;
      target.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX, clientY }));
    }, LONG_PRESS_MS);
  }

  function endPress() {
    if (pressTimer !== null) clearTimeout(pressTimer);
    pressTimer = null;
  }

  function onclick(step: () => boolean) {
    if (pressed) {
      pressed = false;
      return;
    }
    step();
  }
</script>

{#snippet arrow(id: string, label: string, icon: 'chevron-left' | 'chevron-right', which: 'back' | 'forward')}
  {@const items = lists[which]}
  <ContextMenu.Root>
    <ContextMenu.Trigger disabled={items.length === 0}>
      {#snippet child({ props })}
        <button
          {...props}
          class="app-history-arrow"
          {id}
          title={label}
          aria-label={label}
          disabled={items.length === 0}
          onclick={() => onclick(which === 'back' ? () => back() : forward)}
          onpointerdown={(e) => {
            (props.onpointerdown as ((e: PointerEvent) => void) | undefined)?.(e);
            onpointerdown(e);
          }}
          onpointerup={(e) => {
            (props.onpointerup as ((e: PointerEvent) => void) | undefined)?.(e);
            endPress();
          }}
          onpointerleave={endPress}
          onpointercancel={(e) => {
            (props.onpointercancel as ((e: PointerEvent) => void) | undefined)?.(e);
            endPress();
          }}><Icon name={icon} size={16} /></button
        >
      {/snippet}
    </ContextMenu.Trigger>
    <ContextMenu.Portal>
      <ContextMenu.Content preventScroll={false} aria-label="{label} to">
        {#snippet child({ props, wrapperProps })}
          <div {...wrapperProps}>
            <div {...props} class="app-history-menu" id="{id}-menu">
              {#each items as item, i (i)}
                <ContextMenu.Item onSelect={() => back(item.steps)}>
                  {#snippet child({ props: entry })}
                    <div {...entry} class="app-history-menu-item" data-address={item.address}>{item.name}</div>
                  {/snippet}
                </ContextMenu.Item>
              {/each}
            </div>
          </div>
        {/snippet}
      </ContextMenu.Content>
    </ContextMenu.Portal>
  </ContextMenu.Root>
{/snippet}

<div class="app-history" data-no-drag>
  {@render arrow('app-back', 'Back', 'chevron-left', 'back')}
  {@render arrow('app-forward', 'Forward', 'chevron-right', 'forward')}
</div>

<style>
  .app-history {
    align-items: center;
    display: inline-flex;
    flex: 0 0 auto;
  }
  /* As the sidebar toggle beside them (SidebarToggle.svelte) */
  .app-history-arrow {
    align-items: center;
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    display: inline-flex;
    flex: 0 0 28px;
    height: 28px;
    justify-content: center;
    width: 28px;
  }
  .app-history-arrow:hover:not(:disabled) {
    background: var(--surface);
    color: var(--text);
  }
  .app-history-arrow:disabled {
    opacity: 0.35;
  }
  .app-history-menu {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: 0 8px 24px color-mix(in srgb, var(--overlay) 60%, transparent);
    display: flex;
    flex-direction: column;
    max-height: 60vh;
    max-width: 320px;
    min-width: 200px;
    overflow-y: auto;
    padding: var(--space-2);
    z-index: 50;
  }
  .app-history-menu-item {
    border-radius: var(--radius-sm);
    color: var(--text);
    cursor: default;
    font-size: var(--text-sm);
    overflow: hidden;
    padding: var(--space-2) var(--space-4);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .app-history-menu-item[data-highlighted] {
    background: var(--surface2);
  }
</style>
