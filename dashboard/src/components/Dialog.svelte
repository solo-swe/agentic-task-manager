<script lang="ts">
  import type { Snippet } from 'svelte';
  import { compactAt } from '../lib/scroll';
  import HintScroll from './HintScroll.svelte';

  interface Props {
    /** Prefix for the element IDs: `<id>-overlay`, `<id>-panel`, and `<id>-body` for a scrolling one */
    id: string;
    title: string;
    open: boolean;
    onclose: () => void;
    children: Snippet;
    /**
     * A long dialog (Help, What's new): its title stays, with a hairline under it, and the content scrolls below it
     * without a scrollbar, the hints counting these items (a selector) above and below. Once the content has
     * scrolled, the title compacts, as the dashboard's title bar does.
     */
    scrollItem?: string;
  }

  let { id, title, open, onclose, children, scrollItem }: Props = $props();

  let closeButton = $state<HTMLButtonElement>();
  /** The content has scrolled: the title compacts */
  let scrolled = $state(false);

  // Focus moves into the dialog when it opens and back to whatever opened it when it closes
  $effect(() => {
    if (!open) return;
    const opener = document.activeElement as HTMLElement | null;
    closeButton?.focus();
    return () => opener?.focus();
  });

  function onOverlayClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onclose();
  }

  function onKeydown(e: KeyboardEvent) {
    if (open && e.key === 'Escape') onclose();
  }
</script>

<svelte:document onkeydown={onKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions (Escape closes, see above) -->
<div class="detail-overlay" class:open id="{id}-overlay" onclick={onOverlayClick}>
  <div
    class="detail-panel {id}"
    class:scrolling={!!scrollItem}
    class:scrolled
    id="{id}-panel"
    role="dialog"
    aria-modal="true"
    aria-label={title}
  >
    <button bind:this={closeButton} class="detail-close" aria-label="Close" onclick={onclose}>&times;</button>
    <h3>{title}</h3>
    {#if scrollItem}
      <HintScroll
        item={scrollItem}
        outer="dialog-scroll"
        id="{id}-body"
        onscrolled={(top) => (scrolled = compactAt(top))}
      >
        {@render children()}
      </HintScroll>
    {:else}
      {@render children()}
    {/if}
  </div>
</div>
