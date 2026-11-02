// Makes the files in branding/ from the logo's source, dashboard/src/assets/atm-logo.svg (see README.md here).
// Run `npm run branding` in dashboard/, where Playwright is installed.
import { readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const here = (path) => new URL(path, import.meta.url);

export const SOURCE = here('../dashboard/src/assets/atm-logo.svg');
/** The accent blue: the favicon's colour (dashboard/build/favicon.ts), on light and dark backgrounds alike. */
export const ACCENT = '#2f81f7';
/** GitHub's dark background, behind the PNGs. */
export const BACKGROUND = '#0d1117';

/** The logo for places outside the dashboard (README, GitHub): the source, with currentColor made the accent blue. */
export function brandLogo(svg, color = ACCENT) {
  return svg
    .replace(
      /<!--[\s\S]*?-->/,
      '<!-- The ATM logo in the accent blue, for the README and GitHub. Made by branding/generate.mjs from\n' +
        "       dashboard/src/assets/atm-logo.svg: don't edit it, change the source and run `npm run branding` -->",
    )
    .replace(/currentColor/g, color);
}

const page = (width, height, style, body) => `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <style>
      html, body { margin: 0; width: ${width}px; height: ${height}px; }
      body {
        display: flex; flex-direction: column; align-items: center; justify-content: center;
        background: ${BACKGROUND};
      }
      ${style}
    </style>
  </head>
  <body>${body}</body>
</html>`;

const svgImg = (svg, size) =>
  `<img src="data:image/svg+xml,${encodeURIComponent(svg)}" width="${size}" height="${size}" alt="" />`;

/** The logo alone, 512×512: for places that take only PNG, JPG or GIF (the org's avatar on GitHub). */
export const logoPage = (svg) => ({
  file: 'atm-logo.png',
  width: 512,
  height: 512,
  html: page(512, 512, '', svgImg(svg, 360)),
});

/** The social preview's dashboard (T-279): the built dashboard on a demo project, at a fixed time */
export const DASHBOARD = here('../plugin/templates/tasks-dashboard.html');
export const DEMO = here('demo-tasks.json');
export const DEMO_NOW = '2026-03-10T15:00:00Z';
/** The demo's agents at work, as atm's claims.json lists them */
const DEMO_CLAIMS = {
  claims: [
    { task: 'T-006', agent: 'claude:4f2a91c0', branch: 'dev/checkout', claimed_at: '2026-03-10T13:20:00Z', lease_expires_at: '2026-03-10T16:00:00Z' },
    { task: 'T-007', agent: 'claude:9b13de77', branch: 'dev/checkout', claimed_at: '2026-03-10T14:05:00Z', lease_expires_at: '2026-03-10T16:00:00Z' },
  ],
};

/** The two commands that install atm, as the README gives them */
export const INSTALL = ['/plugin marketplace add solo-swe/agentic-task-manager', '/plugin install atm@solo-swe-plugins'];

/**
 * A screenshot of the built dashboard on the demo project (the Kanban board, dark theme), as a PNG data URL: served
 * from a page route the way the e2e tests serve it, so the card shows the real product and follows its changes.
 */
async function dashboardShot(browser) {
  const html = readFileSync(DASHBOARD, 'utf8');
  const tasks = readFileSync(DEMO, 'utf8');
  const { meta } = JSON.parse(tasks);
  const tab = await browser.newPage({
    viewport: { width: 1280, height: 760 },
    deviceScaleFactor: 2,
    colorScheme: 'dark',
    reducedMotion: 'reduce',
    locale: 'en-US',
    timezoneId: 'UTC',
  });
  try {
    await tab.clock.setFixedTime(new Date(DEMO_NOW));
    // No auto-refresh, and What's new already seen (no "new" on the version chip)
    await tab.addInitScript(() =>
      localStorage.setItem('atm-dashboard', JSON.stringify({ autoRefresh: false, lastSeenVersion: '999.0.0' })),
    );
    const json = (body, status = 200) => ({ status, contentType: 'application/json', body: JSON.stringify(body) });
    await tab.route('http://atm.local/**', (route) => {
      const path = new URL(route.request().url()).pathname;
      if (path === '/') return route.fulfill({ contentType: 'text/html', body: html });
      if (path === '/api/projects')
        return route.fulfill(json({ projects: [{ id: meta.id, name: meta.project, path: '/demo', missing: false, summary: null }] }));
      if (path === `/p/${meta.id}/tasks.json`) return route.fulfill({ contentType: 'application/json', body: tasks });
      if (path === `/p/${meta.id}/claims.json`) return route.fulfill(json(DEMO_CLAIMS));
      return route.fulfill(json({ error: 'not found' }, 404));
    });
    await tab.goto('http://atm.local/#view=kanban');
    await tab.locator('.kanban-card').first().waitFor();
    const png = await tab.screenshot();
    return `data:image/png;base64,${png.toString('base64')}`;
  } finally {
    await tab.close();
  }
}

const FONT = "-apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif";
const MONO = "'SF Mono', Menlo, Consolas, monospace";
const TAGLINE = 'Task management for Claude Code agents, with a live dashboard of every project.';
const installBlock = (size) =>
  `<div class="install">${INSTALL.map((c) => `<code style="font-size:${size}px">${c}</code>`).join('')}</div>`;
const CARD_STYLE = `
  body { color: #e6edf3; font-family: ${FONT}; }
  h1 { margin: 0; font-weight: 700; letter-spacing: -1px; }
  p { margin: 0; color: #8b949e; line-height: 1.35; }
  .install { display: flex; flex-direction: column; gap: 8px; }
  .install code { display: block; padding: 8px 14px; border-radius: 8px; background: #161b22; border: 1px solid #30363d;
    font-family: ${MONO}; color: #e6edf3; white-space: nowrap; }
  .install code::before { content: '› '; color: ${ACCENT}; }
  .shot { border-radius: 12px; border: 1px solid #30363d; box-shadow: 0 24px 60px rgba(0, 0, 0, 0.6); }`;

/** GitHub's social preview card, 1280×640, shown when the repository is linked: three layouts to choose from */
export const PREVIEW_LAYOUTS = {
  // A: words on the left (logo, name, line, install), the dashboard on the right, cut off by the card's edge
  a: (svg, shot) =>
    page(
      1280,
      640,
      `${CARD_STYLE}
      body { flex-direction: row; justify-content: flex-start; overflow: hidden; }
      .words { display: flex; flex-direction: column; gap: 22px; width: 560px; padding-left: 56px; flex-shrink: 0; }
      .brand { display: flex; align-items: center; gap: 18px; }
      h1 { font-size: 46px; line-height: 1.05; }
      p { font-size: 24px; }
      .shot { width: 1000px; margin-left: 32px; flex-shrink: 0; }`,
      `<div class="words">
        <div class="brand">${svgImg(svg, 84)}<h1>Agentic Task<br />Manager</h1></div>
        <p>${TAGLINE}</p>
        ${installBlock(15)}
      </div>
      <img class="shot" src="${shot}" alt="" />`,
    ),
  // B: a title band on top, the dashboard below across the card, the install line at the bottom over it
  b: (svg, shot) =>
    page(
      1280,
      640,
      `${CARD_STYLE}
      body { justify-content: flex-start; overflow: hidden; }
      .band { display: flex; align-items: center; gap: 22px; width: 1152px; padding: 40px 0 26px; }
      h1 { font-size: 50px; }
      p { font-size: 22px; margin-top: 6px; }
      .shot { width: 1152px; }
      .foot { position: absolute; left: 0; right: 0; bottom: 0; display: flex; justify-content: center; gap: 14px;
        padding: 70px 0 28px; background: linear-gradient(to bottom, transparent, ${BACKGROUND} 55%); }
      .foot .install { flex-direction: row; gap: 14px; }`,
      `<div class="band">${svgImg(svg, 96)}<div><h1>Agentic Task Manager</h1><p>${TAGLINE}</p></div></div>
      <img class="shot" src="${shot}" alt="" />
      <div class="foot">${installBlock(18)}</div>`,
    ),
  // C: the dashboard tilted behind, faded; the logo, the name and the install centred over it
  c: (svg, shot) =>
    page(
      1280,
      640,
      `${CARD_STYLE}
      body { overflow: hidden; position: relative; }
      .shot { position: absolute; width: 1300px; left: -10px; top: 40px; opacity: 0.32;
        transform: perspective(1400px) rotateX(18deg) rotateZ(-4deg); }
      .veil { position: absolute; inset: 0; background: radial-gradient(ellipse at center, ${BACKGROUND}ee 30%, ${BACKGROUND}88 75%); }
      .words { position: relative; display: flex; flex-direction: column; align-items: center; gap: 20px; }
      h1 { font-size: 62px; }
      p { font-size: 26px; text-align: center; max-width: 1000px; }`,
      `<img class="shot" src="${shot}" alt="" /><div class="veil"></div>
      <div class="words">${svgImg(svg, 120)}<h1>Agentic Task Manager</h1><p>${TAGLINE}</p>${installBlock(19)}</div>`,
    ),
};
/** The layout the social preview uses */
export const PREVIEW_LAYOUT = 'a';

export const socialPreviewPage = (svg, shot, layout = PREVIEW_LAYOUT, file = 'social-preview.png') => ({
  file,
  width: 1280,
  height: 640,
  html: PREVIEW_LAYOUTS[layout](svg, shot),
});

async function main() {
  const logo = brandLogo(readFileSync(SOURCE, 'utf8'));
  writeFileSync(here('atm-logo.svg'), logo);
  console.log('branding/atm-logo.svg');

  // Playwright is installed in dashboard/, not next to this script
  const require = createRequire(here('../dashboard/package.json'));
  const { chromium } = require('@playwright/test');
  const browser = await chromium.launch();
  try {
    const shot = await dashboardShot(browser);
    // `--variants <dir>`: every layout, into <dir>, to choose from; otherwise the files in branding/
    const at = process.argv.indexOf('--variants');
    const outDir = at > 0 ? process.argv[at + 1] : null;
    const pages = outDir
      ? Object.keys(PREVIEW_LAYOUTS).map((l) => socialPreviewPage(logo, shot, l, `social-preview-${l}.png`))
      : [logoPage(logo), socialPreviewPage(logo, shot)];
    for (const { file, width, height, html } of pages) {
      const tab = await browser.newPage({ viewport: { width, height } });
      await tab.setContent(html, { waitUntil: 'load' });
      const path = outDir ? `${outDir}/${file}` : fileURLToPath(here(file));
      await tab.screenshot({ path });
      await tab.close();
      console.log(outDir ? path : `branding/${file}`);
    }
  } finally {
    await browser.close();
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main();
