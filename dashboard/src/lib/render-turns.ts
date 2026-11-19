// Renders that take turns (ATM-587, measured on CompMD's CR-1 in WebKit: a review's runs of code, each highlighted by
// CompMD, all rendered in one task once its grammars had loaded, made frames of 0.5 to 1.5 s). A render waiting its
// turn shows its text plain; the turns go one at a time, the one nearest the view first, each when the page is idle
// after the one before it has drawn, so scrolling and reading never wait for the highlighting.

/** One render waiting: where it is on the page, and the render itself, done when it has drawn */
interface Turn {
  where: () => Element | undefined;
  render: () => Promise<void>;
  /** Cancelled: not rendered, even once picked to go next (ATM-620) */
  cancelled?: boolean;
}

const waiting = new Set<Turn>();
let running = false;
/** A mouse button held down (a drag picking lines, a text selection): no turn starts, so nothing is redrawn under the
 * pointer (a drag over runs whose highlighting came in meanwhile lost its lines); the turns go on once it is let go */
let pressed = false;
let listening = false;

function listen() {
  if (listening || typeof window === 'undefined') return;
  listening = true;
  window.addEventListener('mousedown', () => (pressed = true), true);
  window.addEventListener(
    'mouseup',
    () => {
      pressed = false;
      next();
    },
    true,
  );
}

/** How far a box is from the view: 0 inside it, else the distance to its nearer edge; nowhere (no element), last */
function distance(el: Element | undefined): number {
  if (!el?.isConnected) return Number.POSITIVE_INFINITY;
  const box = el.getBoundingClientRect();
  const height = window.innerHeight || document.documentElement.clientHeight;
  if (box.bottom >= 0 && box.top <= height) return 0;
  return box.top > height ? box.top - height : -box.bottom;
}

/** When the page is idle (or soon, where there is no idle callback): the next turn never waits long */
function whenIdle(act: () => void) {
  if (typeof requestIdleCallback === 'function') requestIdleCallback(act, { timeout: 150 });
  else setTimeout(act, 0);
}

function next() {
  if (running || pressed || !waiting.size) return;
  let nearest: Turn | null = null;
  let best = Number.POSITIVE_INFINITY;
  for (const turn of waiting) {
    const d = distance(turn.where());
    if (nearest === null || d < best) [nearest, best] = [turn, d];
  }
  const turn = nearest as Turn;
  waiting.delete(turn);
  running = true;
  whenIdle(() => {
    // Cancelled while it waited for the page to be idle (ATM-620): its render gone with it, the next one's turn. A
    // render of a text whose CompMd was destroyed made a view nothing would destroy, holding the card it was in
    if (turn.cancelled) {
      running = false;
      next();
      return;
    }
    // Pressed meanwhile: back in the line, to go once it is let go
    if (pressed) {
      running = false;
      waiting.add(turn);
      return;
    }
    turn
      .render()
      .catch(() => {})
      .finally(() => {
        running = false;
        next();
      });
  });
}

/** `render` when its turn comes (`where` says where it is, for which goes first). Cancel it (it isn't needed any more:
 * its text changed, or it is gone) with what this returns; one that started finishes. */
export function takeTurn(where: () => Element | undefined, render: () => Promise<void>): () => void {
  listen();
  const turn: Turn = { where, render };
  waiting.add(turn);
  next();
  return () => {
    turn.cancelled = true;
    waiting.delete(turn);
  };
}

/** For tests: the turns waiting, and whether one is running */
export function turnsWaiting(): { waiting: number; running: boolean } {
  return { waiting: waiting.size, running };
}

/** For tests: forget every turn */
export function resetTurns() {
  waiting.clear();
  running = false;
  pressed = false;
}
