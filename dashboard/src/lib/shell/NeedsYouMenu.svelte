<script lang="ts">
  // A "N need(s) you" chip (D-3.3) that takes you to the task: one task opens it; several open a list of them under
  // the chip, Bits UI's menu (T-431: the arrows, Home and End move, Enter opens, Escape or a click outside closes, and
  // focus goes back to the chip). Used in the title bar and on the overview's project cards. It can sit in a <p>, so
  // it is built from phrasing elements only.
  import { tick } from 'svelte';
  import { DropdownMenu } from 'bits-ui';
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
  let list = $state<HTMLElement | null>(null);
  /** How far the list is moved so it stays inside the window (T-332): lined up with the chip, it can run off an edge */
  let shift = $state(0);
  const EDGE = 8;

  async function keepInView() {
    shift = 0;
    await tick();
    const box = list?.getBoundingClientRect();
    if (!box) return;
    const width = document.documentElement.clientWidth;
    if (box.left < EDGE) shift = EDGE - box.left;
    else if (box.right > width - EDGE) shift = Math.max(EDGE - box.left, width - EDGE - box.right);
  }

  const REASONS = { review: 'in review', blocked: 'blocked on a question', together: 'to do with you' } as const;
  const label = $derived(`${items.length} need${items.length === 1 ? 's' : ''} you`);

  // The list closes when it has nothing left to list (the tasks were answered meanwhile)
  $effect(() => {
    if (open && items.length < 2) open = false;
  });
</script>

{#if items.length === 1}
  <span class="menu-anchor needs-you">
    <button
      class="attention-chip {chipClass}"
      id={chipId}
      title="Open {items[0].id} {items[0].name}"
      onclick={() => onopen(items[0].id)}>{label}</button
    >
  </span>
{:else if items.length}
  <span class="menu-anchor needs-you">
    <DropdownMenu.Root bind:open onOpenChange={(next) => next && keepInView()}>
      <DropdownMenu.Trigger>
        {#snippet child({ props })}
          <button {...props} class="attention-chip {chipClass}" id={chipId} title="Show the tasks that need you"
            >{label}</button
          >
        {/snippet}
      </DropdownMenu.Trigger>
      <!-- Not modal: the page stays as it is (scrolling, a click on it closes the list), as the dashboard's popups -->
      <DropdownMenu.ContentStatic bind:ref={list} preventScroll={false} aria-label="Tasks that need you">
        {#snippet child({ props })}
          <span
            {...props}
            class="menu needs-you-menu"
            class:align-right={align === 'right'}
            id={menuId}
            style:translate={shift ? `${shift}px 0` : undefined}
          >
            {#each items as task (task.id)}
              <DropdownMenu.Item onSelect={() => onopen(task.id)}>
                {#snippet child({ props })}
                  <button {...props} class="needs-you-row" data-id={task.id}>
                    <span class="needs-you-id">{task.id}</span>
                    <span class="needs-you-name">{task.name}</span>
                    <span class="needs-you-reason">{REASONS[task.reason]}</span>
                  </button>
                {/snippet}
              </DropdownMenu.Item>
            {/each}
          </span>
        {/snippet}
      </DropdownMenu.ContentStatic>
    </DropdownMenu.Root>
  </span>
{/if}
