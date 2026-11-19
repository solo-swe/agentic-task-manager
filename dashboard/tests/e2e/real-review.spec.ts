import { execFileSync, spawn, type ChildProcess } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium, expect, test, webkit, type Browser, type Page } from '@playwright/test';

// A real review, measured as a reader meets it (ATM-587, Hakan on CR-1: the mocked test's numbers aren't what the app
// feels like): a project's own review served by this checkout's atm (`atm _serve` on a private state folder, so the
// developer's own server is untouched), in Chromium and in WebKit (the app's engine on macOS), CompMD's highlighting
// and rendering included. Not part of the suite: it runs only when given a project and a review,
//
//   ATM_MEASURE_PROJECT=<a project's folder> ATM_MEASURE_REVIEW=CR-1 pnpm exec playwright test --project measure --no-deps
//
// In a full run it is the `measure` project, after the others (ATM-641: beside the suite's timed tests it made them
// miss their budgets), so it never shares the machine with them.
//
// and prints what it measured (docs/review-performance.md). It imports Playwright's own `test`, not helpers.ts': the
// helpers' fixture plays atm serve, and this measures the real one.

// Or in `tests/e2e/real-review.local.json` (git ignores it): `{"project": "<folder>", "review": "CR-1"}`
const LOCAL = fileURLToPath(new URL('./real-review.local.json', import.meta.url));
const local = (existsSync(LOCAL) ? JSON.parse(readFileSync(LOCAL, 'utf8')) : {}) as {
  project?: string;
  review?: string;
};
const PROJECT = process.env.ATM_MEASURE_PROJECT ?? local.project ?? '';
const REVIEW = process.env.ATM_MEASURE_REVIEW ?? local.review ?? 'CR-1';
const ATM = fileURLToPath(new URL('../../../target/debug/atm', import.meta.url));

test.skip(!PROJECT, 'measures a real review: give ATM_MEASURE_PROJECT (and ATM_MEASURE_REVIEW)');
test.describe.configure({ mode: 'serial', timeout: 300_000 });

let home = '';
let server: ChildProcess | null = null;
let base = '';

test.beforeAll(async () => {
  home = mkdtempSync(path.join(tmpdir(), 'atm-measure-home-'));
  const env = { ...process.env, ATM_STATE_DIR: home };
  execFileSync(ATM, ['workspace', 'add-project', 'default', PROJECT], { env, stdio: 'pipe' });
  server = spawn(ATM, ['_serve', home, '8960'], { env, stdio: 'ignore' });
  const record = path.join(home, 'server.json');
  await expect.poll(() => existsSync(record), { timeout: 20_000 }).toBe(true);
  const port = JSON.parse(readFileSync(record, 'utf8')).port as number;
  base = `http://localhost:${port}`;
});

test.afterAll(() => {
  server?.kill('SIGTERM');
  if (home) rmSync(home, { recursive: true, force: true });
});

/** The review's page in the browser: its project's ID from atm's list */
async function openReview(page: Page) {
  const listed = (await (await page.request.get(`${base}/api/projects`)).json()) as { projects?: { id: string }[] };
  const id = (Array.isArray(listed) ? listed : (listed.projects ?? []))[0]?.id;
  expect(id, 'the project is listed').toBeTruthy();
  await page.goto(`${base}/projects/${id}/reviews/${REVIEW}`);
  await expect(page.locator('#review-tab-diffs')).toBeVisible({ timeout: 30_000 });
}

/** In the page: the frames from now for `ms`, each frame's length; and what is busy (frames over 50 ms) */
const FRAMES = `(ms) => new Promise((done) => {
  const frames = [];
  const start = performance.now();
  let last = start;
  const tick = (now) => {
    frames.push(now - last);
    last = now;
    if (now - start < ms) requestAnimationFrame(tick);
    else done(frames);
  };
  requestAnimationFrame(tick);
})`;

function summary(frames: number[]) {
  const long = frames.filter((f) => f > 50);
  let at = 0;
  let busyUntil = 0;
  for (const f of frames) {
    at += f;
    if (f > 50) busyUntil = at;
  }
  const sorted = [...frames].sort((a, b) => b - a);
  return {
    longFrames: long.length,
    blockedMs: Math.round(long.reduce((s, f) => s + f, 0)),
    slowest: sorted.slice(0, 3).map(Math.round),
    busyUntilMs: Math.round(busyUntil),
  };
}

async function measure(browser: Browser, engine: string) {
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 }, colorScheme: 'dark' });
  await openReview(page);
  // Opening the Diffs tab: the first file's lines, then how long the page stays busy (5 s watched)
  const opened = await page.evaluate(async (framesSource) => {
    const frames = eval(framesSource) as (ms: number) => Promise<number[]>;
    const start = performance.now();
    (document.querySelector('#review-tab-diffs') as HTMLElement).click();
    const watching = frames(5000);
    while (!document.querySelector('.diff-view .diff-line')) await new Promise((r) => requestAnimationFrame(r));
    const firstDrawn = performance.now() - start;
    const all = await watching;
    // When each long frame ended, from the click, and when CompMD's runtime arrived: what the long frames line up with
    let at = 0;
    const longAt: [number, number][] = [];
    for (const f of all) {
      at += f;
      if (f > 50) longAt.push([Math.round(at), Math.round(f)]);
    }
    const compmd = performance
      .getEntriesByType('resource')
      .filter((e) => e.name.includes('/compmd'))
      .map((e) => {
        const timing = e as PerformanceResourceTiming;
        return [e.name.split('/').pop(), Math.round(timing.startTime - start), Math.round(timing.responseEnd - start)];
      });
    const first = (document.querySelector('.review-file-card') as HTMLElement | null)?.dataset.file;
    const measures = performance
      .getEntriesByType('measure')
      .filter((m) => m.duration > 20)
      .sort((a, b) => b.duration - a.duration)
      .slice(0, 8)
      .map((m) => [m.name, Math.round(m.startTime - start), Math.round(m.duration)]);
    return { firstDrawn: Math.round(firstDrawn), frames: all, longAt, compmd, first, measures };
  }, FRAMES);
  console.log(
    `opening (${engine}):`,
    JSON.stringify({ longAt: opened.longAt, compmd: opened.compmd, first: opened.first, measures: opened.measures }),
  );
  // Scrolled to the bottom a screen a step, as a reader does, each step after its frame; then the last file drawn
  const scrolled = await page.evaluate(async () => {
    const pane = document.querySelector('.review-diffs') as HTMLElement;
    const frame = () => new Promise<number>((r) => requestAnimationFrame(r));
    const frames: number[] = [];
    const start = performance.now();
    let last = start;
    let steps = 0;
    for (; steps < 3000 && pane.scrollTop + pane.clientHeight < pane.scrollHeight - 2; steps++) {
      pane.scrollTop += pane.clientHeight;
      const now = await frame();
      frames.push(now - last);
      last = now;
    }
    const reached = performance.now() - start;
    const cards = [...document.querySelectorAll('.review-file-card')];
    const lastCard = cards[cards.length - 1];
    // Then kept at the end, as a reader stays there, until the last file is drawn (20 s at most: one never drawn says
    // what it shows instead); how far the end moved away meanwhile, as the files above it were drawn
    const waitedFrom = performance.now();
    const endAt = pane.scrollHeight;
    while (
      lastCard &&
      !lastCard.querySelector('.diff-line, .review-file-binary, .review-file-moved') &&
      performance.now() - waitedFrom < 20_000
    ) {
      pane.scrollTop = pane.scrollHeight;
      await frame();
    }
    const endMoved = pane.scrollHeight - endAt;
    const lastShows = lastCard?.querySelector('.diff-line')
      ? 'lines'
      : `${lastCard?.getAttribute('data-file')}: ${(lastCard?.outerHTML ?? '')
          .replace(/\s+/g, ' ')
          .replace(/<svg.*?<\/svg>/g, '')
          .slice(0, 1500)}`;
    return {
      steps,
      files: cards.length,
      reachedMs: Math.round(reached),
      lastDrawnMs: Math.round(performance.now() - start),
      lastShows,
      endMoved,
      frames,
      rows: document.querySelectorAll('.diff-line').length,
      elements: document.querySelectorAll('*').length,
    };
  });
  // Back to the top in one jump, then to the bottom in one: how long until the page answers again
  const jumps = await page.evaluate(async (framesSource) => {
    const frames = eval(framesSource) as (ms: number) => Promise<number[]>;
    const pane = document.querySelector('.review-diffs') as HTMLElement;
    pane.scrollTop = 0;
    const top = await frames(3000);
    pane.scrollTop = pane.scrollHeight;
    const bottom = await frames(3000);
    return { top, bottom };
  }, FRAMES);
  const sides = await page.evaluate(() =>
    performance
      .getEntriesByType('resource')
      .filter((e) => e.name.includes('/documents?'))
      .map((e) => ({
        ms: (e as PerformanceResourceTiming).duration,
        bytes: (e as PerformanceResourceTiming).transferSize,
      })),
  );
  const result = {
    engine,
    open: { firstDrawnMs: opened.firstDrawn, ...summary(opened.frames) },
    scroll: {
      steps: scrolled.steps,
      files: scrolled.files,
      reachedMs: scrolled.reachedMs,
      lastDrawnMs: scrolled.lastDrawnMs,
      lastShows: scrolled.lastShows,
      endMovedPx: scrolled.endMoved,
      ...summary(scrolled.frames),
      rows: scrolled.rows,
      elements: scrolled.elements,
    },
    jumpTop: summary(jumps.top),
    jumpBottom: summary(jumps.bottom),
    sides: {
      count: sides.length,
      totalMs: Math.round(sides.reduce((s, e) => s + e.ms, 0)),
      slowestMs: Math.round(Math.max(0, ...sides.map((e) => e.ms))),
    },
  };
  console.log(`real review ${REVIEW} (${engine}):`, JSON.stringify(result));
  await page.close();
  return result;
}

for (const [engine, type] of [
  ['chromium', chromium],
  ['webkit', webkit],
] as const) {
  test(`${REVIEW} in ${engine}: the Diffs tab opened, scrolled to the bottom, jumped`, async () => {
    const browser = await type.launch();
    try {
      const result = await measure(browser, engine);
      expect(result.scroll.files).toBeGreaterThan(0);
      expect(result.scroll.lastShows, 'the last file is drawn').toBe('lines');
    } finally {
      await browser.close();
    }
  });
}
