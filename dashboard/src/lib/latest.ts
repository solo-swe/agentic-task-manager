/**
 * Only the latest ask's answer counts (ATM-675, audit D1). A view asks for data again and again (the timer, focus, a
 * link, the app's events), and the answers can come back out of order: one that arrives after a newer ask was made,
 * or for another thing than the one now shown, must change nothing.
 *
 * `ask()` starts an ask and returns its ticket. After every `await`, `ticket.current()` says whether it is still the
 * latest ask; `ticket.current(key)` also whether `key` is still what `shown()` names (the workspace and project a load
 * was for, say). `ticket.done()`, in a `finally`, ends it; `busy` says whether one is still out, so a poll can wait for
 * the next tick rather than pile up.
 */
export interface Ticket {
  /** Whether this is still the latest ask, and (given a key) still for what is shown */
  current(key?: string): boolean;
  /** The ask is over, answered or failed; once is enough, more change nothing */
  done(): void;
}

export interface Latest {
  ask(): Ticket;
  /** Whether an ask is still out */
  readonly busy: boolean;
}

export function latest(shown: () => string = () => ''): Latest {
  let asked = 0;
  let out = 0;
  return {
    ask() {
      const number = ++asked;
      out++;
      let open = true;
      return {
        current: (key?: string) => number === asked && (key === undefined || key === shown()),
        done: () => {
          if (!open) return;
          open = false;
          out--;
        },
      };
    },
    get busy() {
      return out > 0;
    },
  };
}
