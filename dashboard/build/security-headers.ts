import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Plugin } from 'vite';

/**
 * What the dev page needs beyond `atm serve`'s policy (atm-cli/atm/api/server.py `page_csp`, T-226), and nothing
 * else: its scripts and styles are the dev server's own files (`'self'`, which also covers the CompMD runtime the
 * server's policy names by its asset folder, forwarded by dev-api.ts), and Vite's client talks over a WebSocket.
 * tests/unit/security-headers.test.ts holds the two policies to this.
 */
export const DEV_ADDITIONS: Record<string, string[]> = {
  'script-src': ["'self'"],
  'style-src': ["'self'"],
  'connect-src': ['ws://localhost:*', 'ws://127.0.0.1:*'],
};

/** The dev page's Content-Security-Policy: the server's, for index.html's inline scripts (`hashes`), plus
 * DEV_ADDITIONS */
export function devCsp(hashes: string[]): string {
  return [
    "default-src 'none'",
    ["script-src 'self'", ...hashes, "'wasm-unsafe-eval'"].join(' '),
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' data:",
    "connect-src 'self' ws://localhost:* ws://127.0.0.1:*",
    "base-uri 'none'",
    "form-action 'none'",
    "frame-ancestors 'none'",
  ].join('; ');
}

/** The hashes of a page's inline scripts, as the server computes them for the built page */
export function inlineScriptHashes(html: string): string[] {
  return [...html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/gi)]
    .filter(([, attrs]) => !attrs.includes('src='))
    .map(([, , body]) => `'sha256-${createHash('sha256').update(body).digest('base64')}'`);
}

/**
 * The dev server sends the headers `atm serve` sends, so something the policy would block shows up while
 * developing. Scripts are the dev server's own files here, plus index.html's inline ones (by hash, as in the built
 * page); Vite's client also needs its WebSocket.
 */
export function securityHeaders(): Plugin {
  return {
    name: 'atm-security-headers',
    apply: 'serve',
    configureServer(server) {
      const csp = devCsp(inlineScriptHashes(readFileSync(join(server.config.root, 'index.html'), 'utf8')));
      server.middlewares.use((_req, res, next) => {
        res.setHeader('Content-Security-Policy', csp);
        res.setHeader('X-Content-Type-Options', 'nosniff');
        res.setHeader('Cross-Origin-Resource-Policy', 'same-origin');
        next();
      });
    },
  };
}
