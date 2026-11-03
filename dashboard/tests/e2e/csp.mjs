// The Content-Security-Policy `atm serve` sends with the dashboard (atm-cli/atm/api/server.py page_csp), rebuilt here
// for the e2e server (serve.mjs), so every e2e test runs under it; tests/scripts/serve.test.mjs checks the two agree.
import { createHash } from 'node:crypto';

/**
 * The policy for a page (its bytes): its own inline scripts by hash, and the CompMD runtime from `compmdAssets` (the
 * full URL of the asset folder it is served from, ending in `/`), which may compile WebAssembly; nothing else from
 * anywhere (docs/compmd-rendering-design.md §2)
 */
export function pageCsp(page, compmdAssets) {
  const hashes = [
    ...Buffer.from(page)
      .toString('latin1')
      .matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/gi),
  ]
    .filter(([, attrs]) => !attrs.includes('src='))
    .map(([, , body]) => `'sha256-${createHash('sha256').update(Buffer.from(body, 'latin1')).digest('base64')}'`);
  return [
    "default-src 'none'",
    `script-src ${[...hashes, "'wasm-unsafe-eval'", compmdAssets].join(' ')}`,
    "style-src 'unsafe-inline'",
    "img-src 'self' data:",
    "connect-src 'self'",
    "base-uri 'none'",
    "form-action 'none'",
    "frame-ancestors 'none'",
  ].join('; ');
}
