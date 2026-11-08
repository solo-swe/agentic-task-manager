// @vitest-environment node
// The dev page runs under atm serve's Content-Security-Policy plus only what Vite needs (T-386, §11.9): a policy the
// server tightens, or the dev server loosens, fails here.
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { DEV_ADDITIONS, devCsp, inlineScriptHashes } from '../../build/security-headers';

const SERVER = readFileSync(new URL('../../../crates/atm-common/src/dashboard.rs', import.meta.url), 'utf8');

/** A policy as {directive: sources} */
function parse(policy: string): Record<string, string[]> {
  return Object.fromEntries(
    policy.split(';').map((part) => {
      const [name, ...sources] = part.trim().split(/\s+/);
      return [name, sources];
    }),
  );
}

/** page_csp's script-src, besides the page's inline scripts by hash: compiling WebAssembly, and the CompMD runtime's
 * asset folder on the page's own origin, which the dev page's `'self'` covers (docs/compmd-rendering-design.md §2) */
const SERVER_SCRIPTS = ["'wasm-unsafe-eval'"];

/** page_csp's body in atm-common's dashboard.rs */
function pageCsp(): string {
  const start = SERVER.indexOf('pub fn page_csp');
  return SERVER.slice(start, SERVER.indexOf('\n}\n', start));
}

/** page_csp's directives, read from dashboard.rs; its script-src is the page's inline scripts by hash (`hashes`), and
 * SERVER_SCRIPTS. A source built from the page's origin (`{assets}`) is the dev page's `'self'`, so it is left out. */
function serverPolicy(hashes: string[]): Record<string, string[]> {
  const policy: Record<string, string[]> = {};
  for (const [, name, sources] of pageCsp().matchAll(/^\s*(?:format!\()?"([a-z-]+)(?: ([^"]*))?"/gm)) {
    policy[name] =
      name === 'script-src'
        ? [...hashes, ...SERVER_SCRIPTS]
        : (sources ?? '').split(/\s+/).filter((s) => s && !/^\{.*\}$/.test(s));
  }
  return policy;
}

describe('the dev page’s Content-Security-Policy (T-386)', () => {
  const hashes = ["'sha256-abc='"];

  it('is the server’s, directive by directive, plus only the dev server’s own scripts, styles and WebSocket', () => {
    const server = serverPolicy(hashes);
    const dev = parse(devCsp(hashes));
    expect(Object.keys(server).sort()).toEqual(Object.keys(dev).sort());
    expect(Object.keys(server)).toContain('frame-ancestors');
    for (const [name, sources] of Object.entries(server)) {
      expect([...dev[name]].sort(), name).toEqual([...sources, ...(DEV_ADDITIONS[name] ?? [])].sort());
    }
  });

  it('the server’s script-src is the hashes, WebAssembly and the asset folder only: never self or eval', () => {
    const body = pageCsp();
    expect(body).toContain('let mut scripts = hashes.to_vec();');
    expect(body.match(/scripts\.push\(.*\);/g)).toEqual([
      `scripts.push("'wasm-unsafe-eval'".into());`,
      'scripts.push(assets.clone());',
    ]);
    expect(body).toContain('format!("script-src {}", scripts.join(" "))');
    // Its only frames: CompMD's sandbox frames from the same folder (T-285)
    expect(body).toContain('let assets = format!("{origin}{COMPMD_ASSETS}");');
    expect(body).toContain('format!("frame-src {assets}")');
    expect(SERVER).toContain('pub const COMPMD_ASSETS: &str = "/assets/compmd/";');
  });

  it('hashes inline scripts only, as the server does for the built page', () => {
    const html = '<script>one()</script><script type="module" src="/src/main.ts"></script>';
    const found = inlineScriptHashes(html);
    expect(found).toHaveLength(1);
    expect(found[0]).toMatch(/^'sha256-[A-Za-z0-9+/]{43}='$/);
    expect(inlineScriptHashes('<script>two()</script>')).not.toEqual(found);
  });
});
