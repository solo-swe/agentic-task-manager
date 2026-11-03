// The real CompMD for component tests: the processor from its WebAssembly, the renderer and the sandbox as the served
// module has them (compmd/entry.ts). Mock lib/compmd's loadCompmd with it:
//
//   vi.mock('../../src/lib/compmd', async (actual) => ({
//     ...(await actual()),
//     loadCompmd: (await import('./compmd.helpers')).realCompmd,
//   }));
//
// and await compmdSettled() after opening a panel, before looking at its task text.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { vi } from 'vitest';
import { initSync, OpenDocument } from '@compmd/processor';
import { createSandbox } from '@compmd/sandbox';
import { request, sandbox } from 'virtual:compmd';
import type { CompmdModule } from '../../src/lib/compmd';

// A path, not a URL: in jsdom, URL is jsdom's, which node:fs doesn't take
const WASM = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../node_modules/@compmd/processor/pkg/compmd_wasm_bg.wasm',
);
let initialised = false;

export function realCompmd(): Promise<CompmdModule> {
  if (!initialised) {
    initSync({ module: readFileSync(WASM) });
    initialised = true;
  }
  // The sandbox follows the reader's colour scheme; jsdom has no matchMedia
  window.matchMedia ??= ((query: string) => ({
    matches: false,
    media: query,
    addEventListener() {},
    removeEventListener() {},
  })) as unknown as typeof window.matchMedia;
  return Promise.resolve({
    init: async () => undefined,
    OpenDocument,
    createSandbox,
    request,
    sandbox,
  } as unknown as CompmdModule);
}

/** Waits until every task text in the page is drawn, or shown as written (CompMd.svelte's data-compmd): CompMD
 * renders after the module loads, and says so after the next paint */
export async function compmdSettled() {
  await vi.waitFor(() => {
    const loading = document.querySelectorAll('.compmd[data-compmd="loading"]').length;
    if (loading) throw new Error(`${loading} task texts still loading`);
  });
}
