import '@testing-library/jest-dom/vitest';
import { afterAll } from 'vitest';

// CompMD's sandbox settles a page in a loop of a task, the drawing, the page's images, stylesheets and fonts, and an
// animation frame (@compmd/sandbox's Settling), which runs on after its view is destroyed. A file whose last test
// rendered CompMD could end, and jsdom go, while a loop was between two steps: its next step read `document` and
// failed the run with an unhandled rejection (ATM-709, under load only). Each file waits those steps out before it ends,
// a few times over, so a loop a step started again has finished too.
const LOOP_STEPS = 5;
const step = () => new Promise<void>((resolve) => requestAnimationFrame(() => setTimeout(resolve, 0)));
afterAll(async () => {
  if (typeof requestAnimationFrame === 'undefined') return;
  for (let i = 0; i < LOOP_STEPS; i++) await step();
});

// jsdom has no FontFaceSet, which every browser has: CompMD 0.5.0's page listens to it for fonts that finish loading
// (its `layout`), and its sandbox waits for `document.fonts.ready` before it says the page settled. Fonts are always
// loaded here.
if (typeof document !== 'undefined' && !('fonts' in document)) {
  const fonts = Object.assign(new EventTarget(), { ready: Promise.resolve(), status: 'loaded' });
  Object.defineProperty(document, 'fonts', { value: fonts });
}

// jsdom has no ResizeObserver, which Svelte's `bind:clientWidth` and friends use: one that never reports, as jsdom lays
// nothing out
if (typeof globalThis.ResizeObserver === 'undefined') {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}
