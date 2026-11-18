<script lang="ts">
  // A "N need(s) you" chip (D-3.3) that takes you to what needs you: the tasks, and the reviews waiting on you
  // (ATM-575: counted in the same chip, listed after the tasks). One thing opens it; several open a list of them under
  // the chip, Bits UI's menu (T-431: the arrows, Home and End move, Enter opens, Escape or a click outside closes, and
  // focus goes back to the chip). Used in the title bar and on the overview's project cards. It can sit in a <p>, so
  // it is built from phrasing elements only.
  import { tick } from 'svelte';
  import { DropdownMenu } from 'bits-ui';
  import type { NeedsYouReview } from '$generated/types';
  import type { NeedsYouItem } from '../waiting';
  import { taskLabel } from '../keys';
  import TaskKey from '../ui/TaskKey.svelte';

  let {
    items,
    onopen,
    reviews = [],
    onreview,
    chipId,
    chipClass = '',
    menuId,
    align = 'left',
  }: {
    items: NeedsYouItem[];
    /** Open task `id` (its details, or its project with its details) */
    onopen: (id: string) => void;
    /** The reviews waiting on you (ATM-575): listed after the tasks; none without `onreview` */
    reviews?: NeedsYouReview[];
    /** Open review `reference` (its page) */
    onreview?: (reference: string) => void;
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
  const REVIEW_REASONS = { review: 'waits for your review', answered: 'answered since you wrote' } as const;
  const listed = $derived(onreview ? reviews : []);
  const count = $derived(items.length + listed.length);
  const label = $derived(`${count} need${count === 1 ? 's' : ''} you`);
  // Tasks only: worded as before reviews were listed
  const what = $derived(listed.length ? 'what needs you' : 'the tasks that need you');

  // The list closes when it has nothing left to list (the tasks were answered meanwhile)
  $effect(() => {
    if (open && count < 2) open = false;
  });
</script>

{#if count === 1 && items.length === 1}
  <span class="menu-anchor needs-you">
    <button
      class="attention-chip {chipClass}"
      id={chipId}
      title="Open {taskLabel(items[0])} {items[0].name}"
      onclick={() => onopen(items[0].id)}>{label}</button
    >
  </span>
{:else if count === 1}
  <span class="menu-anchor needs-you">
    <button
      class="attention-chip {chipClass}"
      id={chipId}
      title="Open {listed[0].reference} {listed[0].title}"
      onclick={() => onreview?.(listed[0].reference)}>{label}</button
    >
  </span>
{:else if count}
  <span class="menu-anchor needs-you">
    <DropdownMenu.Root bind:open onOpenChange={(next) => next && keepInView()}>
      <DropdownMenu.Trigger>
        {#snippet child({ props })}
          <button {...props} class="attention-chip {chipClass}" id={chipId} title="Show {what}">{label}</button>
        {/snippet}
      </DropdownMenu.Trigger>
      <!-- Not modal: the page stays as it is (scrolling, a click on it closes the list), as the dashboard's popups -->
      <DropdownMenu.ContentStatic
        bind:ref={list}
        preventScroll={false}
        aria-label={listed.length ? 'What needs you' : 'Tasks that need you'}
      >
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
                    <span class="needs-you-id"><TaskKey {task} /></span>
                    <span class="needs-you-name">{task.name}</span>
                    <span class="needs-you-reason">{REASONS[task.reason]}</span>
                  </button>
                {/snippet}
              </DropdownMenu.Item>
            {/each}
            {#each listed as review (review.reference)}
              <DropdownMenu.Item onSelect={() => onreview?.(review.reference)}>
                {#snippet child({ props })}
                  <button {...props} class="needs-you-row" data-review={review.reference}>
                    <span class="needs-you-id">{review.reference}</span>
                    <span class="needs-you-name">{review.title}</span>
                    <span class="needs-you-reason">{REVIEW_REASONS[review.why]}</span>
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
