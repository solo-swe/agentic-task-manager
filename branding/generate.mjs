// Makes the files in branding/ from the logo's source, dashboard/src/assets/atm-logo.svg (see README.md here).
// Run `pnpm run branding` in dashboard/, where Playwright is installed.
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
        "       dashboard/src/assets/atm-logo.svg: don't edit it, change the source and run `pnpm run branding` -->",
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

/** The light face of the app icon and the avatar (T-504), top to bottom: as other macOS apps have, not GitHub's dark. */
export const FACE = ['#ffffff', '#e6ebf2'];

/**
 * A superellipse, |x|^n + |y|^n = 1, filling a `size`×`size` box at (`x`, `y`), as an SVG path: the shape of macOS
 * app icons since Big Sur, whose corners blend into the sides (a continuous curve), unlike a rounded rectangle's.
 */
export function squircle(x, y, size, n = 5, steps = 256) {
  const r = size / 2;
  const point = (t) => {
    const [c, s] = [Math.cos(t), Math.sin(t)];
    const along = (v) => Math.sign(v) * Math.abs(v) ** (2 / n);
    return `${(x + r + r * along(c)).toFixed(2)} ${(y + r + r * along(s)).toFixed(2)}`;
  };
  return `M${Array.from({ length: steps }, (_, i) => point((i / steps) * 2 * Math.PI)).join('L')}Z`;
}

const dataUrl = (svg) => `data:image/svg+xml,${encodeURIComponent(svg)}`;
const faceGradient = `<linearGradient id="face" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="${FACE[0]}" /><stop offset="1" stop-color="${FACE[1]}" />
    </linearGradient>`;

/** The logo's marks, its columns and check, without its frame: where they reach in the logo's 32×32 drawing (their
 * round 2.5 strokes included) */
const MARKS = { left: 8.75, top: 8.75, right: 26.75, bottom: 23.75 };
/** How much of the icon's shape the marks span across: about what VS Code's and Notion's marks span */
const MARKS_SHARE = 0.62;
/** The logo's strokes, `M10 10v12M16 10v6M22 10v3` and `M19.5 20.5l2 2 4-4.5` at width 2.5 with round ends, as the
 * filled shapes they paint, so any renderer draws them the same (Apple's, for Icon Composer, drew the strokes flat and
 * cut off). Each column a bar with round ends; the check two such bars meeting at its corner. */
export const MARKS_PATHS = 'M10 10v12M16 10v6M22 10v3';
export const CHECK_PATH = 'M19.5 20.5l2 2 4-4.5';
/** A stroke from (x1, y1) to (x2, y2), 2.5 wide with round ends, as the filled path it paints: two sides and two
 * half-circle ends, in plain coordinates */
function bar(x1, y1, x2, y2, r = 1.25) {
  const length = Math.hypot(x2 - x1, y2 - y1);
  const [nx, ny] = [(-(y2 - y1) / length) * r, ((x2 - x1) / length) * r];
  const p = (x, y) => `${x.toFixed(3)} ${y.toFixed(3)}`;
  return (
    `<path d="M${p(x1 + nx, y1 + ny)}L${p(x2 + nx, y2 + ny)}A${r} ${r} 0 0 0 ${p(x2 - nx, y2 - ny)}` +
    `L${p(x1 - nx, y1 - ny)}A${r} ${r} 0 0 0 ${p(x1 + nx, y1 + ny)}Z" />`
  );
}
const MARK_SHAPES = [
  bar(10, 10, 10, 22),
  bar(16, 10, 16, 16),
  bar(22, 10, 22, 13),
  bar(19.5, 20.5, 21.5, 22.5),
  bar(21.5, 22.5, 25.5, 18),
];

/**
 * The app icon's one layer (T-504): the logo's marks in the accent blue, without its frame, centred on a 1024×1024
 * canvas that stands for the icon's whole shape. On the app icon the shape itself is the board: the logo's own frame
 * inside it made a frame within a frame, its white ring a wide border next to other apps' icons (Hakan).
 */
export function marksLayer(logo) {
  const box = (MARKS.right - MARKS.left) / MARKS_SHARE;
  const [cx, cy] = [(MARKS.left + MARKS.right) / 2, (MARKS.top + MARKS.bottom) / 2];
  const [x, y] = [(cx - box / 2).toFixed(3), (cy - box / 2).toFixed(3)];
  const side = box.toFixed(3);
  const color = /stroke="(#[0-9a-f]{6})"/i.exec(logo)?.[1] ?? ACCENT;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="${x} ${y} ${side} ${side}">
  <!-- The ATM logo's marks, the app icon's layer. Made by branding/generate.mjs -->
  <g fill="${color}">
    ${MARK_SHAPES.join('\n    ')}
  </g>
</svg>
`;
}

/**
 * The app icon (T-504), on Apple's icon grid: a 1024×1024 canvas, the body an
 * 824×824 squircle in its middle with a transparent margin, a soft shadow under it and a hairline edge (so the white
 * face shows on a light Dock), the marks layer on it. `pnpm tauri icon` makes the app's icon set from it (README.md).
 */
export function appIcon(logo) {
  const body = squircle(100, 100, 824);
  return `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024">
  <defs>
    ${faceGradient}
    <filter id="shadow" x="-10%" y="-10%" width="120%" height="130%">
      <feDropShadow dx="0" dy="12" stdDeviation="14" flood-color="#000" flood-opacity="0.25" />
    </filter>
  </defs>
  <path d="${body}" fill="url(#face)" filter="url(#shadow)" />
  <path d="${body}" fill="none" stroke="#000" stroke-opacity="0.08" stroke-width="2" />
  <image href="${dataUrl(marksLayer(logo))}" x="100" y="100" width="824" height="824" />
</svg>`;
}

/**
 * The org's avatar on GitHub (T-504), 512×512: the face edge to edge (GitHub rounds the corners itself), the marks
 * layer on it, as on the app icon.
 */
export function avatar(logo) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512">
  <defs>${faceGradient}</defs>
  <rect width="512" height="512" fill="url(#face)" />
  <image href="${dataUrl(marksLayer(logo))}" x="0" y="0" width="512" height="512" />
</svg>`;
}

/** An SVG as a page to screenshot, its background transparent (omitBackground) */
const svgPage = (file, size, svg) => ({
  file,
  width: size,
  height: size,
  transparent: true,
  html: `<!doctype html><html><head><style>html, body { margin: 0; background: transparent; }
    svg { display: block; }</style></head><body>${svg}</body></html>`,
});

/** The icon pages: the app icon's source and the avatar */
export const iconPages = (logo) => [svgPage('app-icon.png', 1024, appIcon(logo)), svgPage('atm-logo.png', 512, avatar(logo))];

/** The social preview's dashboard (T-279): the built dashboard on a demo project, at a fixed time */
export const DASHBOARD = here('../dashboard/dist/tasks-dashboard.html');
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
async function dashboardShot(browser, colorScheme = 'dark') {
  const html = readFileSync(DASHBOARD, 'utf8');
  const tasks = readFileSync(DEMO, 'utf8');
  const { meta } = JSON.parse(tasks);
  const tab = await browser.newPage({
    viewport: { width: 1280, height: 760 },
    deviceScaleFactor: 2,
    colorScheme,
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
const TAGLINE = 'Task management for agentic software development, with a live dashboard of every project.';
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

/** A card on a light background (GitHub's light colours), around the dashboard in light mode: to compare */
export const lightCard = (html) =>
  html.replace(
    '</style>',
    `body { background: #ffffff !important; color: #1f2328 !important; }
      p { color: #59636e !important; }
      .install code { background: #f6f8fa !important; border-color: #d1d9e0 !important; color: #1f2328 !important; }
      .shot { border-color: #d1d9e0 !important; box-shadow: 0 24px 60px rgba(31, 35, 40, 0.18) !important; }
    </style>`,
  );

export const socialPreviewPage = (svg, shot, layout = PREVIEW_LAYOUT, file = 'social-preview-dark.png') => ({
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
  const render = async ({ file, width, height, html, transparent }, outDir) => {
    const tab = await browser.newPage({ viewport: { width, height } });
    await tab.setContent(html, { waitUntil: 'load' });
    const path = outDir ? `${outDir}/${file}` : fileURLToPath(here(file));
    await tab.screenshot({ path, omitBackground: !!transparent });
    await tab.close();
    console.log(outDir ? path : `branding/${file}`);
  };
  try {
    // `--icons`: only the app icon's source and the avatar, without the dashboard the social preview needs
    if (process.argv.includes('--icons')) {
      for (const p of iconPages(logo)) await render(p);
      return;
    }
    const shot = await dashboardShot(browser);
    // `--variants <dir>`: every layout, dark and light, into <dir>, to choose from; otherwise the files in branding/
    const at = process.argv.indexOf('--variants');
    const outDir = at > 0 ? process.argv[at + 1] : null;
    const light = await dashboardShot(browser, 'light');
    // The social preview, dark and light: GitHub takes one, uploaded by hand (README.md)
    const lightPreview = (layout, file) => {
      const p = socialPreviewPage(logo, light, layout, file);
      return { ...p, html: lightCard(p.html) };
    };
    const pages = outDir
      ? Object.keys(PREVIEW_LAYOUTS).flatMap((l) => [
          socialPreviewPage(logo, shot, l, `social-preview-${l}-dark.png`),
          lightPreview(l, `social-preview-${l}-light.png`),
        ])
      : [
          ...iconPages(logo),
          socialPreviewPage(logo, shot, PREVIEW_LAYOUT, 'social-preview-dark.png'),
          lightPreview(PREVIEW_LAYOUT, 'social-preview-light.png'),
        ];
    for (const p of pages) await render(p, outDir);
  } finally {
    await browser.close();
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) await main();
