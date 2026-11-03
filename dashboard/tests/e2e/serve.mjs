// Minimal static server for e2e tests. The built dashboard is served as /svelte/tasks-dashboard.html, with the
// security headers atm serve sends (T-226), and the plugin's asset folder beside it (/svelte/assets/…, the CompMD
// runtime) as atm serve serves it; tasks.json is always supplied per test via page.route.
import { createServer } from 'node:http';
import { lstat, readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { pageCsp } from './csp.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const ASSETS = root + '../plugin/assets/';
const VARIANTS = {
  svelte: root + '../plugin/templates/tasks-dashboard.html',
};
// As atm serve's asset route: plain names, known extensions, no symlinks (atm-cli/atm/api/server.py asset_file)
const ASSET = /^\/(\w+)\/assets\/((?:[A-Za-z0-9_@][A-Za-z0-9._@-]*\/)*[A-Za-z0-9_@][A-Za-z0-9._@-]*\.(js|wasm))$/;
const ASSET_TYPES = { js: 'text/javascript; charset=utf-8', wasm: 'application/wasm' };
const SECURITY = { 'x-content-type-options': 'nosniff', 'cross-origin-resource-policy': 'same-origin' };
const port = Number(process.env.PORT ?? 4173);

createServer(async (req, res) => {
  const path = new URL(req.url, 'http://x').pathname;
  const asset = ASSET.exec(path);
  if (asset && VARIANTS[asset[1]]) {
    try {
      if ((await lstat(ASSETS + asset[2])).isFile()) {
        const body = await readFile(ASSETS + asset[2]);
        res.writeHead(200, { 'content-type': ASSET_TYPES[asset[3]], 'cache-control': 'no-cache', ...SECURITY });
        res.end(body);
        return;
      }
    } catch {
      // not there: 404 below
    }
    res.writeHead(404).end('not found');
    return;
  }
  const match = /^\/(\w+)\/tasks-dashboard\.html$/.exec(path);
  const file = match && VARIANTS[match[1]];
  if (!file) {
    res.writeHead(404).end('not found');
    return;
  }
  try {
    const body = await readFile(file);
    res
      .writeHead(200, {
        'content-type': 'text/html; charset=utf-8',
        'cache-control': 'no-store',
        'content-security-policy': pageCsp(body, `http://${req.headers.host}/${match[1]}/assets/compmd/`),
        ...SECURITY,
      })
      .end(body);
  } catch {
    res.writeHead(404).end('missing build: ' + file);
  }
}).listen(port, () => console.log(`e2e server on http://localhost:${port}`));
