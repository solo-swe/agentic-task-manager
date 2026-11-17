import type { Page } from '@playwright/test';
import { addressOf, expect, openDashboard, projectDocument, test, type TasksServer } from './helpers';
import { E2E_SESSION } from './session.mjs';

// The reviews tab and a review's page (docs/code-review-design.md §3.11, ATM-574), and the diff viewer in it (§3.7,
// ATM-573): the fixture plays atm serve's review routes (the list, a review's page for a view, a round's sides by
// revision) and records the changes the page sends.

const lines = (n: number, change: (i: number) => string | null = () => null) =>
  Array.from({ length: n }, (_, i) => change(i + 1) ?? `line ${i + 1}`).join('\n') + '\n';

/** A Rust file of 40 lines, round 2 changing lines 3 and 30 (and adding one after 30) */
const OLD_A = lines(40);
const NEW_A = lines(40, (i) => (i === 3 ? 'LINE 3' : i === 30 ? 'line 30\nline 30b' : null));
const PATCH_A = [
  'diff --git a/src/a.rs b/src/a.rs',
  '--- a/src/a.rs',
  '+++ b/src/a.rs',
  '@@ -2,3 +2,3 @@',
  ' line 2',
  '-line 3',
  '+LINE 3',
  ' line 4',
  '@@ -29,3 +29,4 @@',
  ' line 29',
  ' line 30',
  '+line 30b',
  ' line 31',
  '',
].join('\n');
const NEW_B = '# Design\n\nThe plan.\n';
const PATCH_B =
  'diff --git a/docs/b.md b/docs/b.md\nnew file mode 100644\n--- /dev/null\n+++ b/docs/b.md\n@@ -0,0 +1,3 @@\n+# Design\n+\n+The plan.\n';
const PATCH_C = 'diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-Old\n+New\n';

const header = (n: number, status = 'open', more: Record<string, unknown> = {}) => ({
  id: `r${n}`,
  reference: `CR-${n}`,
  number: n,
  title: `Review ${n}`,
  status,
  tasks: ['T-001'],
  author: 'claude:a',
  reviewers: [],
  created: '2026-11-17T10:00:00Z',
  updated: '2026-11-17T10:00:00Z',
  ...more,
});

const comment = (id: string, body: string, resolved: boolean | null = false) => ({
  id,
  author: 'Ana',
  at: '2026-11-17T10:00:00Z',
  body,
  resolved,
  replies: [],
});

/** CR-2's page: three rounds, three files, a chain in place, a drifted one, one on the review */
function reviewPage(more: Record<string, unknown> = {}) {
  const file = (path: string, patch: string, status = 'modified', before: string | null = 'base1') => ({
    path,
    old_path: path,
    status,
    binary: false,
    patch,
    before,
    after: 'head3',
  });
  return {
    review: header(2),
    summary: 'Trims the names.',
    branch: 'dev/1',
    rounds: [1, 2, 3].map((number) => ({ number, kind: { kind: 'files' }, files: [], by: 'claude:a', at: '' })),
    chains: [
      {
        id: 't1',
        anchor: { round: 3, path: 'src/a.rs', side: 'new', lines: [3, 3], quote: 'LINE 3' },
        comments: [comment('c1', 'Why upper case?')],
        placed: [3, 3],
        drifted: null,
      },
      {
        id: 't2',
        anchor: { round: 1, path: 'src/a.rs', side: 'new', lines: [7, 8], quote: 'gone' },
        comments: [comment('c2', 'This went away.')],
        placed: [7, 8],
        drifted: { round: 1, lines: [7, 8] },
      },
      {
        id: 't3',
        anchor: { round: 1, path: null, side: 'new', lines: null, quote: null },
        comments: [comment('c3', 'Nice and small.')],
        placed: null,
        drifted: null,
      },
    ],
    view: {
      round: 3,
      against: null,
      files: [file('src/a.rs', PATCH_A), file('docs/b.md', PATCH_B, 'added', null), file('README.md', PATCH_C)],
    },
    needs_you: true,
    you: 'Ana',
    ...more,
  };
}

/** The fixture's project, with CR-1 and CR-2 listed and CR-2's page and sides served; on `path` */
async function openReviews(page: Page, tasksServer: TasksServer, path = '') {
  tasksServer
    .setReviews({
      reviews: [
        { review: header(2), branch: 'dev/1', chains: 3, open: 2, needs_you: true },
        { review: header(1, 'approved', { author: null }), branch: null, chains: 0, open: 0, needs_you: false },
      ],
      you: 'Ana',
    })
    .setReview('CR-2', reviewPage())
    .setRevisionDocument(
      'base1',
      projectDocument('src/a.rs', OLD_A, { kind: 'code', language: 'rust', line_offset: 1 }),
    )
    .setRevisionDocument(
      'head3',
      projectDocument('src/a.rs', NEW_A, { kind: 'code', language: 'rust', line_offset: 1 }),
    )
    .setRevisionDocument('head3', projectDocument('docs/b.md', NEW_B))
    .setRevisionDocument('base1', projectDocument('README.md', 'Old\n', { kind: 'text', line_offset: 1 }))
    .setRevisionDocument('head3', projectDocument('README.md', 'New\n', { kind: 'text', line_offset: 1 }));
  await openDashboard(page, tasksServer, 'standard', { tab: 'reviews' });
  if (path) await page.goto(`/projects/${tasksServer.projectId}/reviews/${path}`);
}

const reviewPath = (tasksServer: TasksServer, rest = '') => `/projects/${tasksServer.projectId}/reviews${rest}`;

/** The page's address without its Sprint filter (the fixture's project has sprints, which the page keeps) */
function shown(page: Page): string {
  const url = new URL(addressOf(page), 'http://x');
  url.searchParams.delete('sprint');
  return url.pathname + decodeURIComponent(url.search);
}

test.describe('the reviews tab', () => {
  test('lists the project’s reviews, which need you, and searches and filters them through the server', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer);
    await expect(page.locator('#tab-reviews')).toHaveAttribute('aria-selected', 'true');
    const rows = page.locator('.review-row');
    await expect(rows).toHaveCount(2);
    await expect(rows.first()).toHaveAttribute('data-review', 'CR-2');
    await expect(rows.first()).toHaveClass(/needs-you/);
    await expect(rows.nth(1)).toContainText('no author yet');
    await page.getByRole('searchbox', { name: 'Search reviews' }).fill('names');
    await expect.poll(() => tasksServer.reviewRequests.at(-1)).toBe('?q=names');
    await page.getByRole('button', { name: 'Open', exact: true }).click();
    await expect.poll(() => tasksServer.reviewRequests.at(-1)).toBe('?q=names&status=open');
  });

  test('opens a review from the list, and the one needing you next', async ({ page, tasksServer }) => {
    await openReviews(page, tasksServer);
    await page.locator('.review-row[data-review="CR-2"] .review-link').click();
    await expect(page.locator('#review-page')).toHaveAttribute('data-review', 'CR-2');
    expect(shown(page)).toBe(reviewPath(tasksServer, '/CR-2'));
    await page.goBack();
    await expect(page.locator('#review-list')).toBeVisible();
    await page.getByRole('button', { name: /Next review needing you/ }).click();
    await expect(page.locator('#review-page')).toHaveAttribute('data-review', 'CR-2');
  });

  test('a reference two reviews share offers both to choose from', async ({ page, tasksServer }) => {
    tasksServer.refuseReview(
      'CR-8',
      400,
      'CR-8 is 2 reviews: CR-8.1 (Theirs), CR-8.2 (Mine) (two clones took the number; name one)',
    );
    await openReviews(page, tasksServer);
    tasksServer.setReviews({
      reviews: [
        {
          review: header(8, 'open', { reference: 'CR-8.1', title: 'Theirs' }),
          branch: null,
          chains: 0,
          open: 0,
          needs_you: false,
        },
        {
          review: header(9, 'open', { reference: 'CR-8.2', title: 'Mine' }),
          branch: null,
          chains: 0,
          open: 0,
          needs_you: false,
        },
        { review: header(1, 'open'), branch: null, chains: 0, open: 0, needs_you: false },
      ],
      you: 'Ana',
    });
    await page.goto(reviewPath(tasksServer, '/CR-8'));
    await expect(page.getByRole('alert')).toContainText('CR-8 is 2 reviews');
    const choices = page.getByRole('list', { name: 'The reviews CR-8 names' });
    await expect(choices.getByRole('listitem')).toHaveText([/CR-8\.1\s+Theirs/, /CR-8\.2\s+Mine/]);
    tasksServer.setReview('CR-8.2', reviewPage({ review: header(9, 'open', { reference: 'CR-8.2', title: 'Mine' }) }));
    await choices.getByRole('link', { name: 'CR-8.2' }).click();
    await expect(page.locator('#review-page')).toHaveAttribute('data-review', 'CR-8.2');
  });

  test('a review the project doesn’t have says so, and goes back to all reviews', async ({ page, tasksServer }) => {
    await openReviews(page, tasksServer, 'CR-9');
    await expect(page.getByRole('alert')).toHaveText('no review CR-9');
    await page.getByRole('button', { name: 'All reviews' }).click();
    await expect(page.locator('#review-list')).toBeVisible();
    expect(shown(page)).toBe(reviewPath(tasksServer));
  });
});

test.describe('a review’s page', () => {
  test('shows the header, the files as a tree, and the first file’s diff, its sides read at their revisions', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer, 'CR-2');
    await expect(page.locator('.review-title')).toContainText('Review 2');
    await expect(page.locator('.review-facts')).toContainText('dev/1');
    await expect(page.locator('.review-task', { hasText: 'T-001' })).toHaveAttribute('href', /task=T-001/);
    await expect(page.locator('.review-files li')).toHaveText([
      'docs/',
      /A\s*b\.md/,
      'src/',
      /M\s*a\.rs/,
      /M\s*README\.md/,
    ]);
    await expect(page.locator('.diff-view')).toHaveAttribute('data-path', 'src/a.rs');
    await expect(page.getByRole('button', { name: 'Line 3', exact: true }).first()).toBeVisible();
    expect(tasksServer.documentRequests).toEqual(expect.arrayContaining(['src/a.rs@base1', 'src/a.rs@head3']));
  });

  test('the file shown, the round and the round against are the address’s, and are asked of the server', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer, 'CR-2');
    await page.locator('[data-file="README.md"]').click();
    await expect(page.locator('.diff-view')).toHaveAttribute('data-path', 'README.md');
    expect(shown(page)).toBe(reviewPath(tasksServer, '/CR-2?file=README.md'));
    // A new file has no side before: only its side after is read
    await page.locator('[data-file="docs/b.md"]').click();
    await expect(page.locator('.diff-view')).toHaveAttribute('data-path', 'docs/b.md');
    expect(tasksServer.documentRequests.filter((r) => r.startsWith('docs/b.md'))).toEqual(['docs/b.md@head3']);
    await page.reload();
    await expect(page.locator('.diff-view')).toHaveAttribute('data-path', 'docs/b.md');
    await page.selectOption('#review-against', '1');
    await expect.poll(() => tasksServer.reviewRequests.at(-1)).toBe('CR-2?round=3&against=1');
    expect(shown(page)).toBe(reviewPath(tasksServer, '/CR-2?round=3&against=1&file=docs/b.md'));
    await page.selectOption('#review-round', '2');
    await expect.poll(() => tasksServer.reviewRequests.at(-1)).toBe('CR-2?round=2');
  });

  test('a drifted chain links to the round it was made on', async ({ page, tasksServer }) => {
    await openReviews(page, tasksServer, 'CR-2');
    await page.getByRole('button', { name: /see it where it was made, round 1, lines 7–8/ }).click();
    await expect.poll(() => tasksServer.reviewRequests.at(-1)).toBe('CR-2?round=1');
  });
});

test.describe('the diff viewer on a review’s page', () => {
  test('split and unified; unchanged runs folded until shown', async ({ page, tasksServer }) => {
    await openReviews(page, tasksServer, 'CR-2');
    const view = page.locator('.diff-view');
    await expect(view).toHaveAttribute('data-mode', 'split');
    await expect(view.locator('.diff-run').first().locator('.diff-column')).toHaveCount(2);
    await page.getByRole('button', { name: 'Unified' }).click();
    await expect(view).toHaveAttribute('data-mode', 'unified');
    await expect(view.locator('.diff-run').first().locator('.diff-column')).toHaveCount(1);
    // Between the two hunks: lines 5 to 28 folded
    const fold = view.getByRole('button', { name: /Show \d+ unchanged lines/ }).nth(1);
    const shownBefore = await view.locator('.diff-line').count();
    await fold.click();
    await expect.poll(() => view.locator('.diff-line').count()).toBeGreaterThan(shownBefore);
  });

  test('keys: n and p go through the changes, ] and [ through the files, v marks one viewed', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer, 'CR-2');
    await expect(page.getByRole('button', { name: 'Line 3', exact: true }).first()).toBeVisible();
    await page.setViewportSize({ width: 1200, height: 300 });
    await page.keyboard.press('n');
    await page.keyboard.press('n');
    await expect(
      page.locator('.diff-line', { has: page.getByRole('button', { name: 'Line 31', exact: true }) }).first(),
    ).toBeInViewport();
    await page.keyboard.press('p');
    await expect(page.getByRole('button', { name: 'Line 3', exact: true }).first()).toBeInViewport();
    await page.keyboard.press('v');
    await expect(page.getByLabel('Viewed src/a.rs')).toBeChecked();
    await expect(page.locator('.review-viewed-count')).toHaveText('1 of 3 viewed');
    await page.keyboard.press(']');
    await expect(page.locator('.diff-view')).toHaveAttribute('data-path', 'docs/b.md');
    await page.keyboard.press('[');
    await expect(page.locator('.diff-view')).toHaveAttribute('data-path', 'src/a.rs');
    // The tick is kept in the browser
    await page.reload();
    await expect(page.getByLabel('Viewed src/a.rs')).toBeChecked();
    // Not while typing
    await page.getByLabel('Summary').focus();
    await page.keyboard.press(']');
    await expect(page.locator('.diff-view')).toHaveAttribute('data-path', 'src/a.rs');
  });

  test('picking lines starts a chain on them, sent as the user with the page’s token', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer, 'CR-2');
    const newSide = page.locator('.diff-view .diff-new');
    await newSide.getByRole('button', { name: 'Line 2', exact: true }).click();
    await newSide.getByRole('button', { name: 'Line 4', exact: true }).click({ modifiers: ['Shift'] });
    await expect(page.locator('.diff-picked')).toContainText('Lines 2–4');
    await page.locator('.diff-picked').getByRole('button', { name: 'Comment' }).click();
    await page.locator('#review-comment-body').fill('Three lines, one thought.');
    await page.locator('.review-composer').getByRole('button', { name: 'Comment' }).click();
    await expect.poll(() => tasksServer.reviewChanges.length).toBe(1);
    expect(tasksServer.reviewChanges[0]).toEqual({
      method: 'POST',
      path: '/CR-2/comments',
      body: {
        path: 'src/a.rs',
        lines: [2, 4],
        side: 'new',
        quote: 'line 2\nLINE 3\nline 4',
        round: 3,
        body: 'Three lines, one thought.',
      },
      session: E2E_SESSION,
    });
    // And the page asks for the review again, to show it
    await expect.poll(() => tasksServer.reviewRequests.filter((r) => r === 'CR-2').length).toBeGreaterThan(1);
  });

  test('chains show in place: a reply, a resolve, and a comment on the review are sent', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer, 'CR-2');
    const inPlace = page.locator('.diff-view [data-comment="c1"]');
    await expect(inPlace).toBeVisible();
    await inPlace.getByRole('button', { name: 'Reply' }).click();
    await inPlace.getByLabel('Reply to Ana').fill('Shouting, on purpose.');
    await inPlace.locator('.chain-reply').getByRole('button', { name: 'Reply' }).click();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({
        method: 'POST',
        path: '/CR-2/comments/c1/replies',
        body: { body: 'Shouting, on purpose.' },
      });
    await inPlace.getByLabel('Resolved').check();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({
        method: 'PATCH',
        path: '/CR-2/comments/c1',
        body: { resolved: true },
      });
    await expect(page.locator('.review-conversation [data-comment="c3"]')).toBeVisible();
    await page.getByRole('button', { name: 'Comment on the review' }).click();
    await page.locator('#review-comment-body').fill('Overall, fine.');
    await page.locator('.review-composer').getByRole('button', { name: 'Comment' }).click();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({
        method: 'POST',
        path: '/CR-2/comments',
        body: { path: null, lines: null, round: null, body: 'Overall, fine.' },
      });
  });

  test('submit: a decision is sent; a refusal is shown, and nothing else changes', async ({ page, tasksServer }) => {
    await openReviews(page, tasksServer, 'CR-2');
    await page.getByLabel('Summary').fill('Two things.');
    await page.getByRole('button', { name: 'Request changes' }).click();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({
        method: 'POST',
        path: '/CR-2/decisions',
        body: { decision: 'request_changes', body: 'Two things.' },
      });
    tasksServer.answerReviewChanges(403, { error: "the review's author can't decide it" });
    await page.getByRole('button', { name: 'Approve' }).click();
    await expect(page.getByRole('alert')).toHaveText("the review's author can't decide it");
    expect(tasksServer.reviewChanges.at(-1)).toMatchObject({ body: { decision: 'approve' } });
  });
});

test.describe('performance', () => {
  // As performance.spec.ts: inside the page, from the action until two frames after it is drawn
  const BUDGET_MS = 200 * (process.env.CI ? 2 : 1);

  test('a large review (300 files) opens a file within budget', async ({ page, tasksServer }) => {
    const files = Array.from({ length: 300 }, (_, i) => {
      const path = `src/m${String(Math.floor(i / 20)).padStart(2, '0')}/f${String(i).padStart(3, '0')}.rs`;
      return {
        path,
        old_path: path,
        status: 'modified',
        binary: false,
        patch: `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -1,3 +1,3 @@\n line 1\n-line 2\n+LINE 2\n line 3\n`,
        before: 'base1',
        after: 'head3',
      };
    });
    await openReviews(page, tasksServer);
    for (const f of files.slice(0, 2)) {
      tasksServer
        .setRevisionDocument(
          'base1',
          projectDocument(f.path, lines(200), { kind: 'code', language: 'rust', line_offset: 1 }),
        )
        .setRevisionDocument(
          'head3',
          projectDocument(
            f.path,
            lines(200, (i) => (i === 2 ? 'LINE 2' : null)),
            {
              kind: 'code',
              language: 'rust',
              line_offset: 1,
            },
          ),
        );
    }
    tasksServer.setReview('CR-2', reviewPage({ chains: [], view: { round: 3, against: null, files } }));
    await page.goto(reviewPath(tasksServer, '/CR-2'));
    await expect(page.locator('.review-file')).toHaveCount(300);
    await expect(page.locator('.diff-view[data-path="src/m00/f000.rs"] .diff-line').first()).toBeVisible();
    const ms = await page.evaluate(async () => {
      const frame = () => new Promise((r) => requestAnimationFrame(r));
      const start = performance.now();
      (document.querySelector('[data-file="src/m00/f001.rs"]') as HTMLElement).click();
      while (!document.querySelector('.diff-view[data-path="src/m00/f001.rs"] .diff-line')) await frame();
      await frame();
      await frame();
      return performance.now() - start;
    });
    expect(ms).toBeLessThan(BUDGET_MS);
  });
});
