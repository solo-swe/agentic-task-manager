// A box the user just opened to write in (a comment, a reply, a summary) takes the focus when it shows (`{@attach
// focusOnShow}`): `autofocus` only focuses when nothing has the focus, and the button that opened it has.
export function focusOnShow(el: HTMLElement): void {
  el.focus();
}

/** The same, for a box whose place is still being drawn when it shows (ATM-651: a rendered document's comment box,
 * under its block, the page cut there and the part above it drawn again): focused without scrolling, then brought into
 * view, as little as that takes, once the page has drawn. Focused at once, it scrolled to where the part above it still
 * held the whole document, a document away, and the box then sat under the file's header */
export function focusWhenPlaced(el: HTMLElement): () => void {
  el.focus({ preventScroll: true });
  let frame = requestAnimationFrame(() => {
    frame = requestAnimationFrame(() => el.scrollIntoView({ block: 'nearest' }));
  });
  return () => cancelAnimationFrame(frame);
}
