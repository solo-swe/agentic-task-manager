// What is being written and not sent yet, kept outside the box it is written in (ATM-732): a comment or reply box in
// the diff view sits in a run of rows that an update of the review may cut again (a chain arriving above it), which
// makes the box again. Its text, its cursor and whether it had the focus are kept here as they change, by what the box
// is for (`comment:<file>`, `reply:<comment id>`), so the box made again comes back as it was. One review's page at a
// time: the page forgets them all when it goes (`forgetAllUnsent`).

export interface Unsent {
  text: string;
  start: number;
  end: number;
  focused: boolean;
}

const kept = new Map<string, Unsent>();

/** What is kept for `key`, if anything */
export function unsent(key: string): Unsent | undefined {
  return kept.get(key);
}

/** The box for `key` was sent or closed: nothing kept for it */
export function forgetUnsent(key: string) {
  kept.delete(key);
}

/** Every key kept, for a component to find its own among (a chain's replies) */
export function unsentKeys(): string[] {
  return [...kept.keys()];
}

/** The review's page went: nothing written on it is kept for the next */
export function forgetAllUnsent() {
  kept.clear();
}

/**
 * An attachment for a box written in (`{@attach keepsUnsent('reply:c9', focusOnShow)}`): what it holds, its cursor
 * and its focus kept as they change. A box made again, with something kept for it, gets its cursor back, and the
 * focus only if it had it (so a box remade under a reader writing elsewhere doesn't take it from them); a box just
 * opened is focused by `focus` (its text is the component's, bound to the box, and comes back through `unsent`). A
 * box removed while it had the focus keeps that: a blur only counts once the box is still on the page after it.
 */
export function keepsUnsent(key: string, focus?: (el: HTMLElement) => void | (() => void)) {
  return (el: HTMLTextAreaElement) => {
    const was = kept.get(key);
    let unfocus: void | (() => void);
    if (was && el.value === was.text) {
      if (was.focused) el.focus({ preventScroll: true });
      el.setSelectionRange(was.start, was.end);
    } else unfocus = focus?.(el);
    const save = () => {
      if (!el.isConnected) return;
      if (!el.value) {
        kept.delete(key);
        return;
      }
      kept.set(key, {
        text: el.value,
        start: el.selectionStart,
        end: el.selectionEnd,
        focused: document.activeElement === el,
      });
    };
    const blur = () =>
      queueMicrotask(() => {
        const now = kept.get(key);
        if (el.isConnected && now) now.focused = false;
      });
    const events = ['input', 'select', 'keyup', 'pointerup', 'focus'] as const;
    for (const name of events) el.addEventListener(name, save);
    el.addEventListener('blur', blur);
    return () => {
      unfocus?.();
      for (const name of events) el.removeEventListener(name, save);
      el.removeEventListener('blur', blur);
    };
  };
}
