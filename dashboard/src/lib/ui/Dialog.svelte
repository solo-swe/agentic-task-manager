<script lang="ts">
  import type { Snippet } from 'svelte';
  import { Dialog } from 'bits-ui';
  import { compactAt } from '../scroll';
  import HintScroll from './HintScroll.svelte';

  // A modal dialog (About, Help, What's new), on Bits UI's (T-431): focus moves into it and stays there while it is
  // open, and back to whatever opened it; Escape and a click outside close it; the page under it doesn't scroll. It
  // stays in the page while closed (the overlay's `open` class shows it), as the dashboard's other panels do.

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
</script>

<Dialog.Root {open} onOpenChange={(next) => !next && onclose()}>
  <Dialog.Overlay forceMount>
    {#snippet child({ props })}
      <div {...props} class="detail-overlay" class:open id="{id}-overlay">
        <Dialog.Content
          forceMount
          id="{id}-panel"
          onOpenAutoFocus={(e) => {
            // The close button, as before: the content may be long, and a link in it isn't where reading starts
            e.preventDefault();
            closeButton?.focus();
          }}
        >
          {#snippet child({ props })}
            <div {...props} class="detail-panel {id}" class:scrolling={!!scrollItem} class:scrolled aria-label={title}>
              <button bind:this={closeButton} class="detail-close" aria-label="Close" onclick={onclose}>&times;</button>
              <Dialog.Title>
                {#snippet child({ props })}<h3 {...props}>{title}</h3>{/snippet}
              </Dialog.Title>
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
          {/snippet}
        </Dialog.Content>
      </div>
    {/snippet}
  </Dialog.Overlay>
</Dialog.Root>
