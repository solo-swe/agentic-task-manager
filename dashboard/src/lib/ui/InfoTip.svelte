<script lang="ts">
  import type { Snippet } from 'svelte';
  import { Popover } from 'bits-ui';
  import Icon from './Icon.svelte';

  // An ⓘ next to a section's title that explains it (docs/progress-tab-review.md: what it shows, how it's counted,
  // when to be careful). A click opens a small popover, Bits UI's (T-431): Escape, a click outside or the button again
  // closes it, and focus goes back to the button. It is placed by CSS, under the button, where it was before.
  let { label, id, children }: { label: string; id?: string; children: Snippet } = $props();
  let open = $state(false);
  let popover = $state<HTMLElement | null>(null);
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

<span class="infotip">
  <Popover.Root bind:open>
    <Popover.Trigger>
      {#snippet child({ props })}
        <button {...props} class="infotip-button" {id} aria-label="About {label}" title="About {label}"
          ><Icon name="info" size={14} /></button
        >
      {/snippet}
    </Popover.Trigger>
    <!-- Text only: Tab moves on from it, as from any text -->
    <Popover.ContentStatic bind:ref={popover} trapFocus={false} role="dialog" aria-label="About {label}">
      {#snippet child({ props })}
        <span {...props} class="infotip-popover" style:transform={shift ? `translateX(-${shift}px)` : undefined}
          >{@render children()}</span
        >
      {/snippet}
    </Popover.ContentStatic>
  </Popover.Root>
</span>
