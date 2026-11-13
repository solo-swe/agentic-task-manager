<script lang="ts" generics="T extends { id: string }">
  // A strip of tabs for a desktop window's title bar, as Notion's or a browser's (T-513, T-514): every tab one fixed
  // width, the bar's full height, between thin lines, the active one in the page's background so it joins the page
  // under it; a close button on each tab when the app gives `onclose`, and a + after them when it gives `onnew`. What a
  // tab shows is the app's (`label`); this component knows nothing about atm, so other apps can use it as it is.
  //
  // A WAI-ARIA tablist: the arrows, Home and End move between the tabs and select them, Enter and Space select the one
  // with focus. With `onmove`, a tab is dragged along the strip with the pointer, or moved a place with ⌃⇧ (⌘⇧) and an
  // arrow (T-532). Each tab is a button (role tab) filling the tab, its label drawn over it: a click on the label falls
  // through to it, and controls of the label's own (a chooser, a link) keep their clicks, beside the button, not in it.
  import type { Snippet } from 'svelte';

  interface Props {
    /** The tabs, in order */
    tabs: T[];
    /** The selected tab's ID */
    active: string;
    /** A tab's label: its icon, its name, whatever else it shows */
    label: Snippet<[T]>;
    /** Select a tab (a click, the keyboard) */
    onselect: (id: string) => void;
    /** Close a tab: each tab gets a × when given */
    onclose?: (id: string) => void;
    /** Open a new tab: a + after the tabs when given */
    onnew?: () => void;
    /** Move a tab to another place: the tabs can be dragged when given */
    onmove?: (id: string, to: number) => void;
    /** Every tab's width, in pixels */
    width?: number;
    /** The strip's accessible name */
    ariaLabel?: string;
    /** Each tab's element ID is this and the tab's ID; the + is `{idPrefix}new` */
    idPrefix?: string;
  }
  let {
    tabs,
    active,
    label,
    onselect,
    onclose,
    onnew,
    onmove,
    width = 220,
    ariaLabel = 'Tabs',
    idPrefix = 'title-bar-tab-',
  }: Props = $props();

  const elements: Record<string, HTMLElement> = {};

  // Dragging a tab (T-532): with the pointer, not HTML5 drag and drop, which WebKit (the app's webview on macOS and
  // Linux) never starts from a button. The tab follows the pointer along the strip; the others make room as it passes,
  // and it lands where it is let go. A press that doesn't move past THRESHOLD is a click.
  const THRESHOLD = 4;
  let press: { id: string; from: number; x: number; pointer: number } | null = null;
  let dragging = $state<string | null>(null);
  let offset = $state(0);
  /** A drag just ended: the click the browser sends after it isn't a select */
  let dropped = false;

  /** Where the dragged tab would land now: its place moved by how many tab widths it went */
  const target = $derived.by(() => {
    if (dragging === null || !press) return -1;
    return Math.max(0, Math.min(tabs.length - 1, press.from + Math.round(offset / width)));
  });

  /** How far a tab moves aside while another is dragged past it */
  function shift(index: number, id: string): number {
    if (dragging === null || !press || id === dragging) return 0;
    if (press.from < index && index <= target) return -width;
    if (target <= index && index < press.from) return width;
    return 0;
  }

  function onpointerdown(e: PointerEvent, id: string, index: number) {
    // A browser may send no click after a drag (Chromium doesn't, WebKit does): a new press starts afresh
    dropped = false;
    if (!onmove || e.button !== 0) return;
    press = { id, from: index, x: e.clientX, pointer: e.pointerId };
  }

  function onpointermove(e: PointerEvent) {
    if (!press || e.pointerId !== press.pointer) return;
    const dx = e.clientX - press.x;
    if (dragging === null && Math.abs(dx) < THRESHOLD) return;
    if (dragging === null) {
      dragging = press.id;
      (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    }
    offset = dx;
  }

  function onpointerup(e: PointerEvent) {
    if (!press || e.pointerId !== press.pointer) return;
    const moved = dragging !== null;
    const { id, from } = press;
    const to = target;
    press = null;
    dragging = null;
    offset = 0;
    if (!moved) return;
    dropped = true;
    if (to !== from) onmove?.(id, to);
  }

  function onclick(id: string) {
    if (dropped) {
      dropped = false;
      return;
    }
    onselect(id);
  }

  /** A middle click closes a tab, as in a browser */
  function onauxclick(e: MouseEvent, id: string) {
    if (e.button === 1 && onclose) {
      e.preventDefault();
      onclose(id);
    }
  }

  function onkeydown(e: KeyboardEvent, index: number) {
    const last = tabs.length - 1;
    // ⌃⇧ (⌘⇧ on macOS) with an arrow moves the focused tab one place, as a drag would
    if (onmove && e.shiftKey && (e.ctrlKey || e.metaKey) && (e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
      e.preventDefault();
      const to = e.key === 'ArrowLeft' ? index - 1 : index + 1;
      if (to >= 0 && to <= last) {
        const id = tabs[index].id;
        onmove(id, to);
        queueMicrotask(() => elements[id]?.focus());
      }
      return;
    }
    const to =
      e.key === 'ArrowRight'
        ? Math.min(index + 1, last)
        : e.key === 'ArrowLeft'
          ? Math.max(index - 1, 0)
          : e.key === 'Home'
            ? 0
            : e.key === 'End'
              ? last
              : null;
    if (to !== null) {
      e.preventDefault();
      onselect(tabs[to].id);
      elements[tabs[to].id]?.focus();
    }
  }
</script>

<div class="title-bar-tabs" role="tablist" aria-label={ariaLabel} style:--tab-width="{width}px">
  {#each tabs as tab, index (tab.id)}
    <div
      class="title-bar-tab"
      class:selected={tab.id === active}
      class:dragging={dragging === tab.id}
      class:sliding={dragging !== null && dragging !== tab.id}
      style:transform={dragging === tab.id
        ? `translateX(${offset}px)`
        : shift(index, tab.id)
          ? `translateX(${shift(index, tab.id)}px)`
          : undefined}
    >
      <!-- The tab itself (Enter and Space select it, as a button's click), named by its label; dragged to move it -->
      <button
        bind:this={elements[tab.id]}
        class="title-bar-tab-select"
        role="tab"
        id="{idPrefix}{tab.id}"
        aria-selected={tab.id === active}
        aria-labelledby="{idPrefix}{tab.id}-label"
        aria-keyshortcuts={onmove ? 'Control+Shift+ArrowLeft Control+Shift+ArrowRight' : undefined}
        tabindex={tab.id === active ? 0 : -1}
        onclick={() => onclick(tab.id)}
        onauxclick={(e) => onauxclick(e, tab.id)}
        onkeydown={(e) => onkeydown(e, index)}
        onpointerdown={(e) => onpointerdown(e, tab.id, index)}
        {onpointermove}
        {onpointerup}
        onpointercancel={onpointerup}
      ></button>
      <div class="title-bar-tab-label" id="{idPrefix}{tab.id}-label">{@render label(tab)}</div>
      {#if onclose}
        <button
          class="title-bar-tab-close"
          title="Close Tab"
          aria-label="Close Tab"
          tabindex="-1"
          onclick={(e) => {
            e.stopPropagation();
            onclose(tab.id);
          }}>×</button
        >
      {/if}
    </div>
  {/each}
  {#if onnew}
    <button class="title-bar-tab-new" id="{idPrefix}new" title="New Tab" aria-label="New Tab" onclick={onnew}>+</button>
  {/if}
</div>

<style>
  .title-bar-tabs {
    align-self: stretch;
    display: flex;
    min-width: 0;
  }
  .title-bar-tab {
    align-items: center;
    border-left: 1px solid var(--border);
    border-right: 1px solid var(--border);
    cursor: default;
    display: flex;
    flex: 0 0 var(--tab-width);
    gap: var(--space-1);
    --hairline: 1px;
    margin-bottom: calc(-1 * var(--hairline));
    min-width: 0;
    padding: 0 var(--space-2) 0 var(--space-4);
    position: relative;
  }
  .title-bar-tab + .title-bar-tab {
    border-left: none;
  }
  .title-bar-tab.selected {
    background: var(--bg);
  }
  /* The dragged tab follows the pointer, over the others; they slide aside to make room */
  .title-bar-tab.dragging {
    box-shadow: 0 2px 8px color-mix(in srgb, var(--overlay) 35%, transparent);
    z-index: 2;
  }
  .title-bar-tab.sliding {
    transition: transform 0.15s ease;
  }
  @media (prefers-reduced-motion: reduce) {
    .title-bar-tab.sliding {
      transition: none;
    }
  }
  /* The tab's button fills it, under the label */
  .title-bar-tab-select {
    background: none;
    border: none;
    inset: 0;
    position: absolute;
  }
  .title-bar-tab-select:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  /* The label takes the room left; whatever its end holds stays at the tab's right. A click on it goes through to the
     tab's button, except on a control of its own */
  .title-bar-tab-label {
    align-items: center;
    display: flex;
    flex: 1;
    gap: var(--space-2);
    min-width: 0;
    pointer-events: none;
    position: relative;
  }
  .title-bar-tab-label :global(:is(button, a, input, select, [role='button'])) {
    pointer-events: auto;
  }
  .title-bar-tab-close {
    position: relative;
  }
  .title-bar-tab-close,
  .title-bar-tab-new {
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    flex: 0 0 auto;
  }
  .title-bar-tab-close {
    font-size: var(--text-md);
    height: 20px;
    visibility: hidden;
    width: 20px;
  }
  .title-bar-tab:hover .title-bar-tab-close,
  .title-bar-tab.selected .title-bar-tab-close {
    visibility: visible;
  }
  .title-bar-tab-new {
    align-self: center;
    flex-basis: 28px;
    font-size: var(--text-lg);
    height: 28px;
    margin-left: var(--space-1);
  }
  .title-bar-tab-close:hover,
  .title-bar-tab-new:hover {
    background: var(--surface);
    color: var(--text);
  }
</style>
