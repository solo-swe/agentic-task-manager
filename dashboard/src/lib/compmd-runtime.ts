// CompMD for the whole document (ATM-646): what every rendered text shares, made once, not once per text. A review
// page renders tens to hundreds of texts (each diff run, each comment); each paid for a sandbox of its own and an
// observer of its own on <html>. The CompMD pages themselves are one per text; since CompMD 0.8.0 (ATM-648) they share
// one style sheet per document and read the colour scheme only for a frame or a component, so opening or closing one
// restyles nothing (the jump test in reviews.spec.ts holds it at none).
import { sandboxOptions, type CompmdModule } from './compmd';

type Sandbox = ReturnType<CompmdModule['createSandbox']>;
type View = ReturnType<Sandbox['open']>;

let shared: { compmd: CompmdModule; sandbox: Sandbox } | null = null;

/** The document's one sandbox, made the first time it is asked for */
export function sandboxOf(compmd: CompmdModule, make = sandboxOptions): Sandbox {
  if (shared?.compmd !== compmd) shared = { compmd, sandbox: compmd.createSandbox(make(compmd)) };
  return shared.sandbox;
}

/** The views open, told when the dashboard's theme or mode changes */
const following = new Set<Pick<View, 'scheme'>>();
let schemes: MutationObserver | null = null;

/** `view` follows the dashboard's theme and mode (attributes on <html>; its color-scheme follows them): one observer
 * for every view, made with the first and gone with the last. What stops following it back. */
export function followScheme(view: Pick<View, 'scheme'>): () => void {
  following.add(view);
  if (!schemes && typeof MutationObserver !== 'undefined') {
    schemes = new MutationObserver(() => {
      for (const v of following) v.scheme();
    });
    schemes.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'data-mode'] });
  }
  return () => {
    following.delete(view);
    if (following.size === 0) {
      schemes?.disconnect();
      schemes = null;
    }
  };
}

/** Forget the sandbox and the views (tests) */
export function resetRuntime() {
  shared = null;
  following.clear();
  schemes?.disconnect();
  schemes = null;
}
