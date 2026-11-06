<script lang="ts">
  // A "N need(s) you" chip (D-3.3) that takes you to the task: one task opens it; several open a list of them under
  // the chip (a menu: the arrows, Home and End move, Enter opens, Escape or a click outside closes, and focus goes
  // back to the chip). Used in the title bar and on the overview's project cards. It can sit in a <p>, so it is built
  // from phrasing elements only.
  import { tick } from 'svelte';
  import type { NeedsYouItem } from '../waiting';

  let {
    items,
    onopen,
    chipId,
    chipClass = '',
    menuId,
    align = 'left',
  }: {
    items: NeedsYouItem[];
    /** Open task `id` (its details, or its project with its details) */
    onopen: (id: string) => void;
    chipId?: string;
    chipClass?: string;
    menuId?: string;
    /** Which edge of the chip the list lines up with */
    align?: 'left' | 'right';
  } = $props();

  let open = $state(false);
  let root = $state<HTMLElement>();
  let chip = $state<HTMLButtonElement>();
  let list = $state<HTMLElement>();
  /** How far the list is moved so it stays inside the window (T-332): lined up with the chip, it can run off an edge */
  let shift = $state(0);
  const EDGE = 8;

  function keepInView() {
    shift = 0;
    const box = list?.getBoundingClientRect();
    if (!box) return;
    const width = document.documentElement.clientWidth;
    if (box.left < EDGE) shift = EDGE - box.left;
    else if (box.right > width - EDGE) shift = Math.max(EDGE - box.left, width - EDGE - box.right);
  }

  const REASONS = { review: 'in review', blocked: 'blocked on a question', together: 'to do with you' } as const;
  const rows = () => [...(list?.querySelectorAll<HTMLButtonElement>('.needs-you-row') ?? [])];

  async function onChip() {
    if (items.length === 1) {
      onopen(items[0].id);
      return;
    }
    if (open) {
      close();
      return;
    }
    open = true;
    await tick();
    keepInView();
    rows()[0]?.focus();
  }

  function close(refocus = true) {
    open = false;
    if (refocus) chip?.focus();
  }

  function choose(id: string) {
    close();
    onopen(id);
  }

  function onKey(e: KeyboardEvent) {
    const all = rows();
    const i = all.indexOf(document.activeElement as HTMLButtonElement);
    const move = (to: number) => {
      e.preventDefault();
      all[Math.max(0, Math.min(all.length - 1, to))]?.focus();
    };
    if (e.key === 'ArrowDown') move(i + 1);
    else if (e.key === 'ArrowUp') move(i - 1);
    else if (e.key === 'Home') move(0);
    else if (e.key === 'End') move(all.length - 1);
    else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close();
    }
  }

  function onWindowClick(e: MouseEvent) {
    if (open && root && !root.contains(e.target as Node)) close(false);
  }

  // The list closes when it has nothing left to list (the tasks were answered meanwhile)
  $effect(() => {
    if (open && items.length < 2) open = false;
  });
</script>

<svelte:window onclick={onWindowClick} />

{#if items.length}
  <span class="menu-anchor needs-you" bind:this={root}>
    <button
      class="attention-chip {chipClass}"
      id={chipId}
      bind:this={chip}
      aria-haspopup={items.length > 1 ? 'menu' : undefined}
      aria-expanded={items.length > 1 ? open : undefined}
      title={items.length === 1 ? `Open ${items[0].id} ${items[0].name}` : 'Show the tasks that need you'}
      onclick={onChip}>{items.length} need{items.length === 1 ? 's' : ''} you</button
    >
    {#if open}
      <span
        class="menu needs-you-menu"
        class:align-right={align === 'right'}
        id={menuId}
        role="menu"
        aria-label="Tasks that need you"
        tabindex="-1"
        style:translate={shift ? `${shift}px 0` : undefined}
        bind:this={list}
        onkeydown={onKey}
      >
        {#each items as task (task.id)}
          <button class="needs-you-row" role="menuitem" data-id={task.id} onclick={() => choose(task.id)}>
            <span class="needs-you-id">{task.id}</span>
            <span class="needs-you-name">{task.name}</span>
            <span class="needs-you-reason">{REASONS[task.reason]}</span>
          </button>
        {/each}
      </span>
    {/if}
  </span>
{/if}
