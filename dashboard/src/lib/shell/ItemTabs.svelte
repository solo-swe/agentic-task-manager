<script lang="ts">
  // The page's items as its tabs (ATM-752, docs/sidebar-overhaul-design.md §5): Overview, Inbox, Tasks, Code Reviews,
  // Documents, Analysis for a project, four for All projects. A click opens the item in place, on its first view; the
  // arrow keys move between them. In the app the sidebar takes their place (ATM-753).
  import type { Item } from '../address';
  import { ITEM_NAMES } from '../places';
  import { ITEM_ICONS } from '../tabs';
  import Icon from '../ui/Icon.svelte';
  import { adjacent } from '../tabs';

  interface Badge {
    count: number;
    /** What the count is, for the accessible name and the tooltip ("waiting for you") */
    says: string;
    id?: string;
  }

  let {
    items,
    current,
    onselect,
    badges = {},
    label,
  }: {
    items: readonly Item[];
    current: Item;
    onselect: (item: Item) => void;
    badges?: Partial<Record<Item, Badge>>;
    label: string;
  } = $props();

  // The buttons, for moving the focus with the arrow keys (`$state`, as `bind:this` into an object's property needs)
  const buttons: Partial<Record<Item, HTMLButtonElement>> = $state({});

  function onkeydown(e: KeyboardEvent) {
    const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = adjacent(items, current, step);
    onselect(next);
    buttons[next]?.focus();
  }
</script>

<div class="view-tabs item-tabs" role="tablist" aria-label={label}>
  {#each items as item (item)}
    {@const badge = badges[item]}
    <button
      bind:this={buttons[item]}
      class="view-tab item-tab"
      id="item-{item}"
      data-item={item}
      role="tab"
      aria-selected={current === item}
      tabindex={current === item ? 0 : -1}
      aria-label={badge?.count ? `${ITEM_NAMES[item]}, ${badge.count} ${badge.says}` : undefined}
      onclick={() => onselect(item)}
      {onkeydown}
      ><Icon name={ITEM_ICONS[item]} size={15} />{ITEM_NAMES[item]}{#if badge?.count}<span
          class="view-tab-badge"
          id={badge.id}
          title="{badge.count} {badge.says}">{badge.count}</span
        >{/if}</button
    >
  {/each}
</div>
