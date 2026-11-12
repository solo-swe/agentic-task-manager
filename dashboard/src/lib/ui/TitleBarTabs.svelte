<script lang="ts" generics="T extends { id: string }">
  // A strip of tabs for a desktop window's title bar, as Notion's or a browser's (T-513, T-514): every tab one fixed
  // width, the bar's full height, between thin lines, the active one in the page's background so it joins the page
  // under it; a close button on each tab when the app gives `onclose`, and a + after them when it gives `onnew`. What a
  // tab shows is the app's (`label`); this component knows nothing about atm, so other apps can use it as it is.
  //
  // A WAI-ARIA tablist: the arrows, Home and End move between the tabs and select them, Enter and Space select the one
  // with focus. Each tab is a button (role tab) filling the tab, its label drawn over it: a click on the label falls
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
    width = 220,
    ariaLabel = 'Tabs',
    idPrefix = 'title-bar-tab-',
  }: Props = $props();

  const elements: Record<string, HTMLElement> = {};

  function onkeydown(e: KeyboardEvent, index: number) {
    const last = tabs.length - 1;
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
    <div class="title-bar-tab" class:selected={tab.id === active}>
      <!-- The tab itself (Enter and Space select it, as a button's click), named by its label -->
      <button
        bind:this={elements[tab.id]}
        class="title-bar-tab-select"
        role="tab"
        id="{idPrefix}{tab.id}"
        aria-selected={tab.id === active}
        aria-labelledby="{idPrefix}{tab.id}-label"
        tabindex={tab.id === active ? 0 : -1}
        onclick={() => onselect(tab.id)}
        onkeydown={(e) => onkeydown(e, index)}
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
