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
  import { DropdownMenu } from 'bits-ui';
  import { untrack, type Snippet } from 'svelte';

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
    /** A tab's name in words: with it, when the tabs don't all fit, a **More ⌄** menu lists those out of view */
    name?: (tab: T) => string;
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
    name,
  }: Props = $props();

  const elements: Record<string, HTMLElement> = {};

  // More tabs than fit (ATM-605): they scroll in a strip of their own, between the bar's other controls, which stay
  // put; the wheel or trackpad over it scrolls the strip sideways and nothing else; ‹ and › at its ends when the
  // tabs don't fit, each disabled at its end; the active tab is scrolled into view.
  let scroller = $state<HTMLElement>();
  let overflow = $state({ more: false, left: false, right: false });
  /** The part of the strip in view, in pixels from its start: which tabs the More menu lists (ATM-685) */
  let view = $state({ from: 0, to: 0 });

  function measure() {
    const el = scroller;
    if (!el) return;
    const more = el.scrollWidth > el.clientWidth + 1;
    const next = {
      more,
      left: more && el.scrollLeft > 1,
      right: more && el.scrollLeft + el.clientWidth < el.scrollWidth - 1,
    };
    if (next.more !== overflow.more || next.left !== overflow.left || next.right !== overflow.right) overflow = next;
    const seen = { from: el.scrollLeft, to: el.scrollLeft + el.clientWidth };
    if (seen.from !== view.from || seen.to !== view.to) view = seen;
  }

  /** The tabs not wholly in view, in order (every tab is `width` wide): what More ⌄ offers (ATM-685) */
  const hidden = $derived(
    overflow.more ? tabs.filter((_, i) => i * width < view.from - 1 || (i + 1) * width > view.to + 1) : [],
  );
  let moreOpen = $state(false);

  /** ‹ or ›: the strip scrolled by about a tab */
  function step(direction: -1 | 1) {
    scroller?.scrollBy({ left: direction * width, behavior: 'smooth' });
  }

  /** A wheel's turn over the strip, up and down or sideways, scrolls it sideways; the page under it never moves */
  function wheelSideways(el: HTMLElement) {
    const onwheel = (e: WheelEvent) => {
      if (e.ctrlKey) return; // a pinch: the page's zoom
      e.preventDefault();
      if (el.scrollWidth <= el.clientWidth) return;
      const by = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.deltaY;
      el.scrollLeft += e.deltaMode === 1 ? by * 16 : by;
    };
    el.addEventListener('wheel', onwheel, { passive: false });
    // Its room changed (the window, the arrows coming or going): the active tab still in view
    const resized = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(() => reveal(active));
    resized?.observe(el);
    return () => {
      el.removeEventListener('wheel', onwheel);
      resized?.disconnect();
    };
  }

  // Tabs opened, closed or moved: measured again, and the active one brought into view (its own strip only)
  $effect(() => {
    void tabs.length;
    const id = active;
    const el = scroller;
    if (!el) return;
    // Only on those (and on the strip's size changing, below): a scroll since, by the wheel or ‹ ›, stays
    untrack(() => reveal(id));
  });

  /** The tab `id` brought into the strip's view, the strip scrolled as little as that takes */
  function reveal(id: string) {
    const el = scroller;
    const tab = elements[id]?.parentElement;
    if (el && tab) {
      const start = tab.offsetLeft;
      const end = start + tab.offsetWidth;
      if (start < el.scrollLeft) el.scrollLeft = start;
      else if (end > el.scrollLeft + el.clientWidth) el.scrollLeft = end - el.clientWidth;
    }
    measure();
  }

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

<div class="title-bar-tabs" class:overflowing={overflow.more} style:--tab-width="{width}px">
  {#if overflow.more}
    <button
      class="title-bar-tabs-arrow"
      id="{idPrefix}left"
      title="Tabs to the left"
      aria-label="Tabs to the left"
      tabindex="-1"
      disabled={!overflow.left}
      onclick={() => step(-1)}>‹</button
    >
  {/if}
  <!-- Only this scrolls: the tabs, between the bar's other controls; never the page. It fills a box of the tabs' own
       width, which shrinks to the room there is (ATM-639): sized by the scroller itself, WebKit (the app's webview)
       took off what was scrolled out of view, so tabs closed while it was scrolled left it narrower than its room -->
  <div class="title-bar-tabs-room" style:width="{tabs.length * width}px">
    <div
      class="title-bar-tabs-scroller"
      role="tablist"
      aria-label={ariaLabel}
      bind:this={scroller}
      onscroll={measure}
      {@attach wheelSideways}
    >
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
    </div>
  </div>
  {#if overflow.more}
    <button
      class="title-bar-tabs-arrow"
      id="{idPrefix}right"
      title="Tabs to the right"
      aria-label="Tabs to the right"
      tabindex="-1"
      disabled={!overflow.right}
      onclick={() => step(1)}>›</button
    >
  {/if}
  {#if name && hidden.length > 0}
    <!-- More ⌄ (ATM-685): the tabs out of view, to choose one, which is then scrolled into view, as a browser's tab
         list; drawn over the page, so nothing of the bar clips it -->
    <DropdownMenu.Root bind:open={moreOpen}>
      <DropdownMenu.Trigger>
        {#snippet child({ props })}
          <button
            {...props}
            class="title-bar-tabs-more"
            id="{idPrefix}more"
            title="More tabs"
            aria-label="More tabs ({hidden.length})">More<span class="title-bar-tabs-more-caret">⌄</span></button
          >
        {/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content side="bottom" align="end" sideOffset={4} preventScroll={false} aria-label="More tabs">
          {#snippet child({ props, wrapperProps })}
            <div {...wrapperProps}>
              <div {...props} class="title-bar-tabs-menu" id="{idPrefix}more-menu">
                {#each hidden as tab (tab.id)}
                  <DropdownMenu.Item onSelect={() => onselect(tab.id)}>
                    {#snippet child({ props: item })}
                      <div {...item} class="title-bar-tabs-menu-item" data-tab={tab.id}>{name(tab)}</div>
                    {/snippet}
                  </DropdownMenu.Item>
                {/each}
              </div>
            </div>
          {/snippet}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  {/if}
  {#if onnew}
    <button class="title-bar-tab-new" id="{idPrefix}new" title="New Tab" aria-label="New Tab" onclick={onnew}>+</button>
  {/if}
</div>

<style>
  .title-bar-tabs {
    align-self: stretch;
    display: flex;
    flex: 0 1 auto;
    min-width: 0;
  }
  /* More ⌄ (ATM-685): after the arrows, a real width, as theirs (ATM-639) */
  .title-bar-tabs-more {
    align-items: center;
    align-self: center;
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    display: inline-flex;
    flex: 0 0 60px;
    font-size: var(--text-sm);
    gap: var(--space-1);
    height: 28px;
    justify-content: center;
    white-space: nowrap;
    width: 60px;
  }
  .title-bar-tabs-more:hover,
  .title-bar-tabs-more[aria-expanded='true'] {
    background: var(--surface);
    color: var(--text);
  }
  .title-bar-tabs-more-caret {
    line-height: 1;
    transform: translateY(-2px);
  }
  .title-bar-tabs-menu {
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
    /* Over the page, as the bar's other menus (app.css .menu): Bits UI's wrapper takes it from here */
    z-index: 50;
  }
  .title-bar-tabs-menu-item {
    border-radius: var(--radius-sm);
    color: var(--text);
    cursor: default;
    font-size: var(--text-sm);
    overflow: hidden;
    padding: var(--space-2) var(--space-4);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title-bar-tabs-menu-item[data-highlighted] {
    background: var(--surface2);
  }
  /* The tabs' own scroller: clipped to its room, its scroll bar hidden (the wheel and ‹ › scroll it), a scroll at its
     end never passed on to the page */
  .title-bar-tabs-room {
    flex: 0 1 auto;
    min-width: 0;
    position: relative;
  }
  /* It reaches a hairline below its room, over the bar's bottom line (ATM-688): the tabs fill it, so the selected one,
     in the page's background, covers the line and joins the page under it; the others, transparent, leave it showing.
     Clipped to its room, it had cut that pixel off, and the line ran under the selected tab too */
  .title-bar-tabs-scroller {
    --hairline: 1px;
    display: flex;
    inset: 0 0 calc(-1 * var(--hairline)) 0;
    overflow-x: auto;
    overflow-y: hidden;
    overscroll-behavior: contain;
    position: absolute;
    scrollbar-width: none;
  }
  .title-bar-tabs-scroller::-webkit-scrollbar {
    display: none;
  }
  .title-bar-tabs-arrow {
    align-self: center;
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    flex: 0 0 22px;
    font-size: var(--text-lg);
    height: 28px;
    line-height: 1;
    /* Its width too, not only its basis: WebKit sizes the strip's box by its content's widths (ATM-639) */
    width: 22px;
  }
  .title-bar-tabs-arrow:hover:not(:disabled) {
    background: var(--surface);
    color: var(--text);
  }
  .title-bar-tabs-arrow:disabled {
    opacity: 0.35;
  }
  .title-bar-tab {
    align-items: center;
    border-left: 1px solid var(--border);
    border-right: 1px solid var(--border);
    cursor: default;
    display: flex;
    flex: 0 0 var(--tab-width);
    gap: var(--space-1);
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
    width: 28px;
  }
  .title-bar-tab-close:hover,
  .title-bar-tab-new:hover {
    background: var(--surface);
    color: var(--text);
  }
</style>
