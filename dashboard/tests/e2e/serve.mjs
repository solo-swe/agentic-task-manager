// Minimal static server for e2e tests. The built dashboard is served at its pages' paths (/, /projects/<id>/<tab>, …;
// T-429), with the security headers atm serve sends (T-226), and the build's asset folder (dist/assets/) at /assets/
// (the CompMD runtime, its sandbox's frames and packs) as atm serve serves it; tasks.json is always supplied per test
// via page.route.
import { createServer } from 'node:http';
import { lstat, readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { pageCsp } from './csp.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const ASSETS = root + 'dist/assets/';
const PAGE = root + 'dist/tasks-dashboard.html';
// As atm serve's (atm-api's pages.rs)
const PAGE_PATH = /^\/(?:projects(?:\/[^/]+(?:\/[a-z]+)?)?|board|progress|agents)?\/?$/;
// As atm serve's asset route (atm-api's assets.rs): plain names, known extensions, no symlinks; every
// asset readable cross-origin (the sandbox's frames have an opaque origin), a frame document sandboxed
const ASSET = /^\/assets\/((?:[A-Za-z0-9_@][A-Za-z0-9._@-]*\/)*[A-Za-z0-9_@][A-Za-z0-9._@-]*\.(js|wasm|html))$/;
const ASSET_TYPES = {
  js: 'text/javascript; charset=utf-8',
  wasm: 'application/wasm',
  html: 'text/html; charset=utf-8',
};
const SECURITY = { 'x-content-type-options': 'nosniff', 'cross-origin-resource-policy': 'same-origin' };
const port = Number(process.env.PORT ?? 4173);

createServer(async (req, res) => {
  const path = new URL(req.url, 'http://x').pathname;
  const asset = ASSET.exec(path);
  if (asset) {
    try {
      if ((await lstat(ASSETS + asset[1])).isFile()) {
        const body = await readFile(ASSETS + asset[1]);
        res.writeHead(200, {
          'content-type': ASSET_TYPES[asset[2]],
          'cache-control': 'no-cache',
          'access-control-allow-origin': '*',
          ...(asset[2] === 'html' ? { 'content-security-policy': 'sandbox allow-scripts' } : {}),
          ...SECURITY,
        });
        res.end(body);
        return;
      }
    } catch {
      // not there: 404 below
    }
    res.writeHead(404).end('not found');
    return;
  }
  if (!PAGE_PATH.test(path)) {
    res.writeHead(404).end('not found');
    return;
  }
  try {
    const body = await readFile(PAGE);
    res
      .writeHead(200, {
        'content-type': 'text/html; charset=utf-8',
        'cache-control': 'no-store',
        'content-security-policy': pageCsp(body, `http://${req.headers.host}/assets/compmd/`),
        ...SECURITY,
      })
      .end(body);
  } catch {
    res.writeHead(404).end('missing build: ' + PAGE);
  }
}).listen(port, () => console.log(`e2e server on http://localhost:${port}`));
