// A box the user just opened to write in (a comment, a reply, a summary) takes the focus when it shows (`{@attach
// focusOnShow}`): `autofocus` only focuses when nothing has the focus, and the button that opened it has.
export function focusOnShow(el: HTMLElement): void {
  el.focus();
}
