export const HIGHLIGHT_MS = 2000;

/** How to scroll: smoothly, but at once for someone who asked their system to reduce motion (T-345) */
export function scrollBehavior(): ScrollBehavior {
  return typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth';
}

/** What a scroller without a scrollbar needs for its "▲ N more" / "▼ N more" hints */
export interface Box {
  offsetTop: number;
  offsetHeight: number;
}

/**
 * How many items are at least partly out of view above and below a scroller showing `top` to `top + height` (the
 * items' offsetTop is relative to the scroller, which is positioned). A pixel of slack keeps rounding from counting
 * an item that is just in view.
 */
export function cutOff(top: number, height: number, items: Iterable<Box>): { above: number; below: number } {
  const bottom = top + height;
  let above = 0;
  let below = 0;
  for (const item of items) {
    if (item.offsetTop < top - 1) above++;
    else if (item.offsetTop + item.offsetHeight > bottom + 1) below++;
  }
  return { above, below };
}

/** Scroll a task card into view and flash its border, as the legacy "needs:" links did. */
export function scrollToTask(id: string) {
  const el = document.getElementById('task-' + id);
  if (!el) return;
  el.scrollIntoView({ behavior: scrollBehavior(), block: 'center' });
  el.style.borderColor = 'var(--yellow)';
  el.style.boxShadow = '0 0 12px color-mix(in srgb, var(--yellow) 30%, transparent)';
  setTimeout(() => {
    el.style.borderColor = '';
    el.style.boxShadow = '';
  }, HIGHLIGHT_MS);
}
