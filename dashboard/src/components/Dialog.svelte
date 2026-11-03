<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    /** Prefix for the element IDs: `<id>-overlay`, `<id>-panel` */
    id: string;
    title: string;
    open: boolean;
    onclose: () => void;
    children: Snippet;
    /** Its content has scrolled: the title compacts, as the dashboard's title bar does */
    scrolled?: boolean;
  }

  let { id, title, open, onclose, children, scrolled = false }: Props = $props();

  let closeButton = $state<HTMLButtonElement>();

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
  <div class="detail-panel {id}" class:scrolled id="{id}-panel" role="dialog" aria-modal="true" aria-label={title}>
    <button bind:this={closeButton} class="detail-close" aria-label="Close" onclick={onclose}>&times;</button>
    <h3>{title}</h3>
    {@render children()}
  </div>
</div>
