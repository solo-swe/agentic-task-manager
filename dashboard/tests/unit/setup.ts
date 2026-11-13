import '@testing-library/jest-dom/vitest';

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
