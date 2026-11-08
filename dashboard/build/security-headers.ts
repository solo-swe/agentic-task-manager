import { createHash } from 'node:crypto';
import type { ServerResponse } from 'node:http';
import type { Plugin } from 'vite';

/**
 * What the dev page needs beyond `atm serve`'s policy (atm-common's `page_csp`, T-226), and nothing
 * else: its scripts and styles are the dev server's own files (`'self'`, which also covers the CompMD runtime the
 * server's policy names by its asset folder, forwarded by dev-api.ts), and Vite's client talks over a WebSocket.
 * tests/unit/security-headers.test.ts holds the two policies to this.
 */
export const DEV_ADDITIONS: Record<string, string[]> = {
  'script-src': ["'self'"],
  // CompMD's sandbox frames, which the server's policy names by the asset folder (forwarded here)
  'frame-src': ["'self'"],
  'style-src': ["'self'"],
  'connect-src': ['ws://localhost:*', 'ws://127.0.0.1:*'],
};

/** The dev page's Content-Security-Policy: the server's, for index.html's inline scripts (`hashes`), plus
 * DEV_ADDITIONS */
export function devCsp(hashes: string[]): string {
  return [
    "default-src 'none'",
    ["script-src 'self'", ...hashes, "'wasm-unsafe-eval'"].join(' '),
    "frame-src 'self'",
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

type Headers = Record<string, string | string[] | number | undefined>;

/**
 * Hold an HTML response until it ends, then send it with `csp(body)` as its policy: SvelteKit renders the dev page
 * per request, with its own inline start-up script (T-428), so the hashes come from the page as sent. Anything that
 * isn't HTML passes through as it is.
 */
export function withPagePolicy(res: ServerResponse, csp: (html: string) => string): void {
  const writeHead = res.writeHead.bind(res) as (status: number, headers?: Headers) => ServerResponse;
  const write = res.write.bind(res) as (chunk: unknown, ...rest: unknown[]) => boolean;
  const end = res.end.bind(res) as (chunk?: unknown, ...rest: unknown[]) => ServerResponse;
  let head: [number, Headers | undefined] | null = null;
  let html: boolean | null = null;
  const chunks: Buffer[] = [];
  const contentType = () =>
    String(head?.[1]?.['content-type'] ?? head?.[1]?.['Content-Type'] ?? res.getHeader('Content-Type') ?? '');
  const isHtml = () => (html ??= /text\/html/i.test(contentType()));
  const flushHead = () => {
    if (head) writeHead(...head);
    head = null;
  };
  const piece = (chunk: unknown) =>
    typeof chunk === 'string' ? Buffer.from(chunk) : chunk instanceof Uint8Array ? Buffer.from(chunk) : null;
  res.writeHead = ((status: number, ...rest: unknown[]) => {
    const headers = rest.find((r) => r && typeof r === 'object') as Headers | undefined;
    head = [status, headers];
    return res;
  }) as typeof res.writeHead;
  res.write = ((chunk: unknown, ...rest: unknown[]) => {
    if (!isHtml()) {
      flushHead();
      return write(chunk, ...rest);
    }
    const bytes = piece(chunk);
    if (bytes) chunks.push(bytes);
    return true;
  }) as typeof res.write;
  res.end = ((chunk?: unknown, ...rest: unknown[]) => {
    if (!isHtml()) {
      flushHead();
      return end(chunk, ...rest);
    }
    const bytes = typeof chunk === 'function' ? null : piece(chunk);
    if (bytes) chunks.push(bytes);
    const body = Buffer.concat(chunks);
    const policy = csp(body.toString('utf8'));
    if (head?.[1]) {
      for (const key of Object.keys(head[1])) if (/^content-(security-policy|length)$/i.test(key)) delete head[1][key];
    }
    res.setHeader('Content-Security-Policy', policy);
    res.setHeader('Content-Length', body.length);
    flushHead();
    return end(body);
  }) as typeof res.end;
}

/**
 * The dev server sends the headers `atm serve` sends, so something the policy would block shows up while
 * developing. Scripts are the dev server's own files here, plus the page's inline ones (by hash, as in the built
 * page, computed from the page as sent); Vite's client also needs its WebSocket.
 */
export function securityHeaders(): Plugin {
  return {
    name: 'atm-security-headers',
    apply: 'serve',
    configureServer(server) {
      server.middlewares.use((_req, res, next) => {
        res.setHeader('Content-Security-Policy', devCsp([]));
        res.setHeader('X-Content-Type-Options', 'nosniff');
        res.setHeader('Cross-Origin-Resource-Policy', 'same-origin');
        withPagePolicy(res, (html) => devCsp(inlineScriptHashes(html)));
        next();
      });
    },
  };
}
