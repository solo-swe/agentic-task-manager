<script lang="ts">
  import type { Snippet } from 'svelte';
  import { cutOff, scrollBehavior } from '../scroll';

  // Content scrolling on its own without a scrollbar (T-262, T-282): "▲ N more" and "▼ N more" at the edges say how
  // many items (`item`, a selector) are cut off above and below; the wheel and the trackpad scroll as usual, a hint
  // scrolls a page. `outer` and `body` add classes to the wrapper and the scroller.
  let {
    children,
    item,
    outer = '',
    body = '',
    id,
    onscrolled,
  }: {
    children: Snippet;
    item: string;
    outer?: string;
    body?: string;
    id?: string;
    /** Told how far the content has scrolled, on every scroll (Help shrinks its title once it has) */
    onscrolled?: (scrollTop: number) => void;
  } = $props();
  let el = $state<HTMLElement>();
  let above = $state(0);
  let below = $state(0);

  function update() {
    if (!el) return;
    ({ above, below } = cutOff(el.scrollTop, el.clientHeight, el.querySelectorAll<HTMLElement>(item)));
  }
  $effect(() => {
    if (!el) return;
    update();
    // A resized window or scroller, and items coming and going, change what is cut off
    const resize = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(update);
    resize?.observe(el);
    const mutation = new MutationObserver(update);
    mutation.observe(el, { childList: true, subtree: true });
    return () => {
      resize?.disconnect();
      mutation.disconnect();
    };
  });
  const page = (direction: 1 | -1) =>
    el?.scrollBy({ top: direction * el.clientHeight * 0.85, behavior: scrollBehavior() });
</script>

<div class="hint-scroll {outer}">
  {#if above}
    <button class="kanban-more top" aria-label="Scroll up: {above} more" onclick={() => page(-1)}>▲ {above} more</button
    >
  {/if}
  <div
    class="hint-scroll-body {body}"
    {id}
    bind:this={el}
    onscroll={() => {
      update();
      if (el) onscrolled?.(el.scrollTop);
    }}
  >
    {@render children()}
  </div>
  {#if below}
    <button class="kanban-more bottom" aria-label="Scroll down: {below} more" onclick={() => page(1)}
      >▼ {below} more</button
    >
  {/if}
</div>
