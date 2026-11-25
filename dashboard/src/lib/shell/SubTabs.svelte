<script lang="ts" generics="T extends string">
  // An item's views as sub-tabs under the page's header (ATM-752, docs/sidebar-overhaul-design.md §2.3): Tasks' Board ·
  // List · Dependencies, Analysis' Progress · Agents. Each is a step in the tab's history, as the items are; the arrow
  // keys move between them.
  import { adjacent } from '../tabs';
  import Icon, { type IconName } from '../ui/Icon.svelte';

  let {
    views,
    current,
    onselect,
    idPrefix,
    panel,
    label,
  }: {
    views: readonly { id: T; label: string; icon: IconName }[];
    current: T;
    onselect: (view: T) => void;
    /** The buttons' ids: `<idPrefix><view>` (`tab-kanban`, `overview-tab-board`) */
    idPrefix: string;
    /** The id of the panel a sub-tab controls, by its view */
    panel: (view: T) => string;
    label: string;
  } = $props();

  const buttons: Partial<Record<string, HTMLButtonElement>> = $state({});

  function onkeydown(e: KeyboardEvent) {
    const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = adjacent(
      views.map((v) => v.id),
      current,
      step,
    );
    onselect(next);
    buttons[next]?.focus();
  }
</script>

<div class="sub-tabs" role="tablist" aria-label={label}>
  {#each views as view (view.id)}
    <button
      bind:this={buttons[view.id]}
      class="sub-tab"
      id="{idPrefix}{view.id}"
      role="tab"
      aria-selected={current === view.id}
      aria-controls={panel(view.id)}
      tabindex={current === view.id ? 0 : -1}
      onclick={() => onselect(view.id)}
      {onkeydown}><Icon name={view.icon} size={14} />{view.label}</button
    >
  {/each}
</div>
