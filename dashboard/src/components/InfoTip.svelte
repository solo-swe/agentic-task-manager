<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  // An ⓘ next to a section's title that explains it (docs/progress-tab-review.md: what it shows, how it's counted,
  // when to be careful). A click opens a small popover; Escape, a click outside or the button again closes it.
  let { label, id, children }: { label: string; id?: string; children: Snippet } = $props();
  let open = $state(false);
  let root = $state<HTMLElement>();
  let popover = $state<HTMLElement>();
  /** How far the popover moves left so it fits, in px: 0 where it fits as placed */
  let shift = $state(0);

  /** The area the popover must stay inside: the nearest ancestor that clips (a popup, a scrolling panel), within the
   * window */
  function bounds(el: HTMLElement): { left: number; right: number } {
    let right = document.documentElement.clientWidth;
    let left = 0;
    for (let node = el.parentElement; node; node = node.parentElement) {
      const { overflowX } = getComputedStyle(node);
      if (overflowX !== 'visible') {
        const box = node.getBoundingClientRect();
        right = Math.min(right, box.right);
        left = Math.max(left, box.left);
        break;
      }
    }
    return { left, right };
  }

  // Opened near the right edge of what clips it (the Activity popup's last card), the popover moves left to fit,
  // never past the left edge
  $effect(() => {
    if (!open || !popover) {
      shift = 0; // so it opens next time where it is placed, and is measured there
      return;
    }
    const box = popover.getBoundingClientRect();
    const { left, right } = bounds(popover);
    const margin = 8;
    const over = box.right - (right - margin);
    shift = over > 0 ? Math.min(over, Math.max(0, box.left - (left + margin))) : 0;
  });
</script>

<svelte:window
  onclick={(e) => open && !root?.contains(e.target as Node) && (open = false)}
  onkeydown={(e) => e.key === 'Escape' && (open = false)}
/>

<span class="infotip" bind:this={root}>
  <button
    class="infotip-button"
    {id}
    aria-label="About {label}"
    aria-expanded={open}
    title="About {label}"
    onclick={() => (open = !open)}><Icon name="info" size={14} /></button
  >
  {#if open}
    <span
      class="infotip-popover"
      role="dialog"
      aria-label="About {label}"
      bind:this={popover}
      style:transform={shift ? `translateX(-${shift}px)` : undefined}>{@render children()}</span
    >
  {/if}
</span>
