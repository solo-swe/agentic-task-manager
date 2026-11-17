import type { Page } from '@playwright/test';
import { addressOf, expect, openDashboard, projectDocument, test, type TasksServer } from './helpers';
import { E2E_SESSION } from './session.mjs';

// The reviews tab and a review's page (docs/code-review-design.md §3.11, ATM-574), and the diff viewer in it (§3.7,
// ATM-573): the fixture plays atm serve's review routes (the list, a review's page for a view, a round's sides by
// revision) and records the changes the page sends. A review's page is a window of its own: the list opens it in a
// browser tab, with its Overview, Diffs and Comments tabs.

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
/** A Markdown file changed in place: both sides */
const OLD_D = '# Plan\n\nFirst step.\n\nOld ending.\n';
const NEW_D = '# Plan\n\nFirst step.\n\nNew ending.\n';
const PATCH_D =
  'diff --git a/docs/plan.md b/docs/plan.md\n--- a/docs/plan.md\n+++ b/docs/plan.md\n@@ -5 +5 @@\n-Old ending.\n+New ending.\n';

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

const file = (path: string, patch: string, status = 'modified', before: string | null = 'base1') => ({
  path,
  old_path: path,
  status,
  binary: false,
  patch,
  before,
  after: 'head3',
});

/** CR-2's page: three rounds, three files, a chain in place, a drifted one, one on the review */
function reviewPage(more: Record<string, unknown> = {}) {
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
        comments: [comment('c3', 'Nice and small.', true)],
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
    .setRevisionDocument('base1', projectDocument('docs/plan.md', OLD_D))
    .setRevisionDocument('head3', projectDocument('docs/plan.md', NEW_D))
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

const card = (page: Page, path: string) => page.locator(`.review-file-card[data-file="${path}"]`);
const diffOf = (page: Page, path: string) => page.locator(`.diff-view[data-path="${path}"]`);
/** The Diffs tab of CR-2, its first file's lines drawn */
async function openDiffs(page: Page, tasksServer: TasksServer, rest = '') {
  await openReviews(page, tasksServer, `CR-2${rest}`);
  await page.getByRole('tab', { name: /^Diffs/ }).click();
  await expect(diffOf(page, 'src/a.rs').getByRole('button', { name: 'Line 3', exact: true }).first()).toBeVisible();
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

  test('opens a review in a browser tab of its own, and the one needing you next; the list stays', async ({
    page,
    tasksServer,
  }) => {
    // The new tab's own page has no fixture behind it (it is the page's): its address is what is checked here, the
    // review's page itself by the tests that open its address
    await openReviews(page, tasksServer);
    const [opened] = await Promise.all([
      page.waitForEvent('popup'),
      page.locator('.review-row[data-review="CR-2"] .review-link').click(),
    ]);
    await opened.waitForURL(/\/reviews\/CR-2/);
    expect(shown(opened)).toBe(reviewPath(tasksServer, '/CR-2'));
    // Opened without a handle back on this page
    expect(await opened.evaluate(() => window.opener)).toBeNull();
    await opened.close();
    await expect(page.locator('#review-list')).toBeVisible();
    expect(shown(page)).toBe(reviewPath(tasksServer));
    const [next] = await Promise.all([
      page.waitForEvent('popup'),
      page.getByRole('button', { name: /Next review needing you/ }).click(),
    ]);
    await next.waitForURL(/\/reviews\/CR-2/);
    await next.close();
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
  test('a window of its own: titled by the project, the reference and the title, without the dashboard around it', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer, 'CR-2');
    await expect(page.locator('#review-window #review-page')).toBeVisible();
    await expect(page).toHaveTitle(/^\S.* CR-2: Review 2$/);
    await expect(page.locator('#tab-reviews')).toHaveCount(0);
    await expect(page.locator('.review-project')).not.toBeEmpty();
    // Back to the list, in the dashboard again
    await page.getByRole('link', { name: '← Reviews' }).click();
    await expect(page.locator('#review-list')).toBeVisible();
    await expect(page.locator('#tab-reviews')).toHaveAttribute('aria-selected', 'true');
  });

  test('Overview: the summary, the facts, the tasks as links, the review’s own chains', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer, 'CR-2');
    await expect(page.getByRole('tab', { name: 'Overview' })).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('.review-title')).toContainText('Review 2');
    await expect(page.locator('.review-summary')).toContainText('Trims the names.');
    await expect(page.locator('.review-facts')).toContainText('dev/1');
    await expect(page.locator('.review-task', { hasText: 'T-001' })).toHaveAttribute('href', /task=T-001/);
    await expect(page.locator('.review-conversation [data-chain="t3"]')).toBeVisible();
    // No diff is made until the Diffs tab is opened
    await expect(page.locator('.diff-view')).toHaveCount(0);
  });

  test('Diffs: the files as a tree beside their cards, each diff’s sides read at their revisions', async ({
    page,
    tasksServer,
  }) => {
    await openDiffs(page, tasksServer);
    await expect(page.locator('.review-files li')).toHaveText([
      'docs/',
      /A\s*b\.md/,
      'src/',
      /M\s*a\.rs/,
      /M\s*README\.md/,
    ]);
    await expect(page.locator('.review-file-card')).toHaveCount(3);
    expect(tasksServer.documentRequests).toEqual(expect.arrayContaining(['src/a.rs@base1', 'src/a.rs@head3']));
    // A new file has no side before: only its side after is read
    await expect
      .poll(() => tasksServer.documentRequests.filter((r) => r.startsWith('docs/b.md')))
      .toEqual(['docs/b.md@head3']);
  });

  test('the file gone to, the round and the round against are the address’s, and are asked of the server', async ({
    page,
    tasksServer,
  }) => {
    await openDiffs(page, tasksServer);
    await page.locator('.review-file-name[data-file="README.md"]').click();
    await expect(page.locator('.review-file.current .review-file-name')).toHaveAttribute('data-file', 'README.md');
    await expect(card(page, 'README.md')).toBeInViewport();
    // The address follows once the page is idle
    await expect.poll(() => shown(page)).toBe(reviewPath(tasksServer, '/CR-2?file=README.md'));
    // The address's file opens the Diffs tab at it
    await page.reload();
    await expect(page.getByRole('tab', { name: /^Diffs/ })).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('.review-file.current .review-file-name')).toHaveAttribute('data-file', 'README.md');
    await page.selectOption('#review-against', '1');
    await expect.poll(() => tasksServer.reviewRequests.at(-1)).toBe('CR-2?round=3&against=1');
    expect(shown(page)).toBe(reviewPath(tasksServer, '/CR-2?round=3&against=1&file=README.md'));
    await page.selectOption('#review-round', '2');
    await expect.poll(() => tasksServer.reviewRequests.at(-1)).toBe('CR-2?round=2');
  });

  test('a drifted chain links to the round it was made on', async ({ page, tasksServer }) => {
    await openDiffs(page, tasksServer);
    await page.getByRole('button', { name: /see it where it was made, round 1, lines 7–8/ }).click();
    await expect.poll(() => tasksServer.reviewRequests.at(-1)).toBe('CR-2?round=1');
  });

  test('a refresh that changed nothing changes nothing on the page: no diff made again, nothing read again', async ({
    page,
    tasksServer,
  }) => {
    await openDiffs(page, tasksServer);
    await page.locator('.diff-view[data-path="src/a.rs"]').evaluate((el) => ((el as HTMLElement).dataset.kept = 'yes'));
    const reads = tasksServer.documentRequests.length;
    const asked = tasksServer.reviewRequests.length;
    await page.evaluate(() => window.dispatchEvent(new Event('focus')));
    await expect.poll(() => tasksServer.reviewRequests.length).toBeGreaterThan(asked);
    await page.waitForTimeout(200);
    await expect(page.locator('.diff-view[data-path="src/a.rs"]')).toHaveAttribute('data-kept', 'yes');
    await expect(page.locator('.diff-loading')).toHaveCount(0);
    expect(tasksServer.documentRequests.length).toBe(reads);
  });

  test('Comments: every chain, all, open or resolved; a chain’s place goes to its file', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer, 'CR-2');
    await page.getByRole('tab', { name: /^Comments/ }).click();
    const listed = page.locator('.review-comments .chain');
    await expect(listed).toHaveCount(3);
    await page.getByRole('button', { name: /^Open/ }).click();
    await expect(listed).toHaveCount(2);
    await page.getByRole('button', { name: /^Resolved/ }).click();
    await expect(listed).toHaveCount(1);
    await expect(listed.first()).toHaveAttribute('data-chain', 't3');
    await page.getByRole('button', { name: /^All/ }).click();
    await page.getByRole('button', { name: 'src/a.rs:3', exact: true }).click();
    await expect(page.getByRole('tab', { name: /^Diffs/ })).toHaveAttribute('aria-selected', 'true');
    await expect(card(page, 'src/a.rs')).toBeInViewport();
  });
});

test.describe('the diff viewer on a review’s page', () => {
  test('split and unified for every file, kept for the next page; unchanged runs folded until shown', async ({
    page,
    tasksServer,
  }) => {
    await openDiffs(page, tasksServer);
    const view = diffOf(page, 'src/a.rs');
    await expect(view).toHaveAttribute('data-mode', 'split');
    await expect(view.locator('.diff-run').first().locator('.diff-column')).toHaveCount(2);
    await page.getByRole('button', { name: 'Unified' }).click();
    await expect(page.locator('.diff-view[data-mode="unified"]')).toHaveCount(3);
    await expect(view.locator('.diff-run').first().locator('.diff-column')).toHaveCount(1);
    // Between the two hunks: lines 5 to 28 folded
    const fold = view.getByRole('button', { name: /Show \d+ unchanged lines/ }).nth(1);
    const shownBefore = await view.locator('.diff-line').count();
    await fold.click();
    await expect.poll(() => view.locator('.diff-line').count()).toBeGreaterThan(shownBefore);
    await page.reload();
    await page.getByRole('tab', { name: /^Diffs/ }).click();
    await expect(diffOf(page, 'src/a.rs')).toHaveAttribute('data-mode', 'unified');
  });

  test('Rendered: a Markdown file on the document viewer’s page, with its line numbers; Unified shows one side', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer);
    tasksServer.setReview(
      'CR-2',
      reviewPage({ view: { round: 3, against: null, files: [file('docs/plan.md', PATCH_D)] } }),
    );
    await page.goto(reviewPath(tasksServer, '/CR-2?file=docs/plan.md'));
    const view = diffOf(page, 'docs/plan.md');
    await expect(view.locator('.diff-line').first()).toBeVisible();
    await page.getByRole('button', { name: 'Rendered' }).click();
    await expect(view).toHaveAttribute('data-view', 'rendered');
    await expect(view.locator('.diff-pane')).toHaveCount(2);
    // Each side's lines numbered beside their text, as the document viewer numbers them; the changed line marked
    const newSide = view.locator('.diff-pane.diff-new');
    await expect(newSide.locator('.doc-gutter-lines [data-line="5"]')).toHaveClass(/marked/);
    await expect(newSide.locator('.doc-gutter-lines [data-line="1"]')).toBeVisible();
    await expect(newSide.locator('.diff-block-mark.diff-added')).not.toHaveCount(0);
    await page.getByRole('button', { name: 'Unified' }).click();
    await expect(view.locator('.diff-pane')).toHaveCount(1);
    await expect(view.locator('.diff-pane')).toHaveClass(/diff-new/);
  });

  test('the + on a line, dragged over three lines, opens the comment under them; sent as the user with the page’s token', async ({
    page,
    tasksServer,
  }) => {
    await openDiffs(page, tasksServer);
    // The new side's runs: the chain on line 3 cuts the first hunk after it
    const newSide = diffOf(page, 'src/a.rs').locator('.diff-new');
    const lineBox = async (n: number) =>
      (await newSide
        .getByRole('button', { name: `Line ${n}`, exact: true })
        .first()
        .boundingBox())!;
    const two = await lineBox(2);
    // The pointer over line 2 shows its +
    await page.mouse.move(two.x + two.width + 40, two.y + two.height / 2);
    const plus = newSide.getByRole('button', { name: 'Comment on line 2' });
    await expect(plus).toBeVisible();
    const box = (await plus.boundingBox())!;
    const four = await lineBox(4);
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width / 2, four.y + four.height / 2, { steps: 4 });
    await page.mouse.up();
    const composer = page.locator('.diff-composer');
    await expect(composer).toContainText('Lines 2–4');
    // Each line picked keeps its + while the comment is written
    await expect(newSide.locator('.diff-plus.held')).toHaveCount(3);
    await composer.getByRole('textbox').fill('Three lines, one thought.');
    await composer.getByRole('button', { name: 'Comment' }).click();
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

  test('a press on a line number picks it, Shift-press extends the pick; Escape drops it', async ({
    page,
    tasksServer,
  }) => {
    await openDiffs(page, tasksServer);
    const newSide = diffOf(page, 'src/a.rs').locator('.diff-new');
    await newSide.getByRole('button', { name: 'Line 29', exact: true }).click();
    await newSide.getByRole('button', { name: 'Line 31', exact: true }).click({ modifiers: ['Shift'] });
    await expect(page.locator('.diff-composer')).toContainText('Lines 29–31');
    await expect(page.locator('.diff-composer textarea')).toBeFocused();
    await page.keyboard.press('Escape');
    await expect(page.locator('.diff-composer')).toHaveCount(0);
    await expect(page.locator('.diff-line.picked')).toHaveCount(0);
  });

  test('chains in place: Resolve above the comment, then a resolved check; a reply and a comment on the review are sent', async ({
    page,
    tasksServer,
  }) => {
    await openDiffs(page, tasksServer);
    const inPlace = page.locator('.diff-view [data-chain="t1"]');
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
    await inPlace.getByRole('button', { name: /Resolve/ }).click();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({
        method: 'PATCH',
        path: '/CR-2/comments/c1',
        body: { resolved: true },
      });
    // The review's own chain, resolved: a check above it, folded until shown
    await page.getByRole('tab', { name: 'Overview' }).click();
    const own = page.locator('.review-conversation [data-chain="t3"]');
    await expect(own.locator('.chain-folded-note')).toContainText('Resolved');
    await own.getByRole('button', { name: 'Show' }).click();
    await expect(own.locator('.chain-state.resolved')).toContainText('Resolved');
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

  test('viewed folds a file, kept in the browser; its fold opens it again', async ({ page, tasksServer }) => {
    await openDiffs(page, tasksServer);
    await card(page, 'src/a.rs').getByLabel('Viewed src/a.rs').check();
    await expect(card(page, 'src/a.rs')).toHaveClass(/folded/);
    await expect(diffOf(page, 'src/a.rs')).toHaveCount(0);
    await expect(page.locator('.review-viewed-count')).toHaveText('1 of 3 viewed');
    await page.reload();
    await page.getByRole('tab', { name: /^Diffs/ }).click();
    await expect(card(page, 'src/a.rs')).toHaveClass(/folded/);
    await expect(page.locator('.review-files').getByLabel('Viewed src/a.rs')).toBeChecked();
    await page.getByRole('button', { name: 'Show src/a.rs' }).click();
    await expect(diffOf(page, 'src/a.rs')).toBeVisible();
  });

  test('Prev and Next go from change to change across the files; n and p too, ] [ the files, v the file at the top', async ({
    page,
    tasksServer,
  }) => {
    await page.setViewportSize({ width: 1200, height: 420 });
    await openDiffs(page, tasksServer);
    const current = page.locator('.review-current-change');
    const next = page.locator('#review-next-change');
    await next.click();
    await expect(current).toHaveAttribute('data-row', /.+/);
    await expect(current).toBeInViewport();
    const first = await current.getAttribute('data-row');
    await next.click();
    await expect(current).not.toHaveAttribute('data-row', first!);
    await expect(current).toBeInViewport();
    // Past src/a.rs's two changes, into the next file's
    await next.click();
    await expect(card(page, 'docs/b.md').locator('.review-current-change')).toBeInViewport();
    await page.locator('#review-prev-change').click();
    await expect(card(page, 'src/a.rs').locator('.review-current-change')).toBeInViewport();
    // The keys do the same
    await page.keyboard.press('n');
    await expect(card(page, 'docs/b.md').locator('.review-current-change')).toBeInViewport();
    await page.keyboard.press('p');
    await expect(card(page, 'src/a.rs').locator('.review-current-change')).toBeInViewport();
    await page.keyboard.press(']');
    await expect(page.locator('.review-file.current .review-file-name')).toHaveAttribute('data-file', 'docs/b.md');
    await page.keyboard.press('v');
    await expect(page.locator('.review-files').getByLabel('Viewed docs/b.md')).toBeChecked();
    await page.keyboard.press('[');
    await expect(page.locator('.review-file.current .review-file-name')).toHaveAttribute('data-file', 'src/a.rs');
    // Not while typing
    await page.locator('#review-round').focus();
    await page.keyboard.press(']');
    await expect(page.locator('.review-file.current .review-file-name')).toHaveAttribute('data-file', 'src/a.rs');
  });

  test('the splitter between the files and the diffs is dragged, and its width kept', async ({ page, tasksServer }) => {
    await openDiffs(page, tasksServer);
    const bar = page.getByRole('separator', { name: 'Resize the files' });
    const tree = page.locator('.review-files');
    const before = (await tree.boundingBox())!.width;
    const box = (await bar.boundingBox())!;
    await page.mouse.move(box.x + box.width / 2, box.y + 100);
    await page.mouse.down();
    await page.mouse.move(box.x + 120, box.y + 100, { steps: 5 });
    await page.mouse.up();
    await expect.poll(async () => (await tree.boundingBox())!.width).toBeGreaterThan(before + 100);
    const width = await bar.getAttribute('aria-valuenow');
    await page.reload();
    await page.getByRole('tab', { name: /^Diffs/ }).click();
    await expect(page.getByRole('separator', { name: 'Resize the files' })).toHaveAttribute('aria-valuenow', width!);
  });

  test('submit: a decision is sent; a refusal is shown, and nothing else changes', async ({ page, tasksServer }) => {
    await openReviews(page, tasksServer, 'CR-2');
    await page.getByRole('button', { name: 'Submit review' }).click();
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
    await page.getByRole('button', { name: 'Submit review' }).click();
    await page.getByRole('button', { name: 'Approve' }).click();
    await expect(page.getByRole('alert')).toHaveText("the review's author can't decide it");
    expect(tasksServer.reviewChanges.at(-1)).toMatchObject({ body: { decision: 'approve' } });
  });
});

test.describe('performance', () => {
  // As performance.spec.ts: inside the page, from the action until two frames after it is drawn
  const BUDGET_MS = 200 * (process.env.CI ? 2 : 1);

  test('a large review (300 files) goes to a file far down within budget, making only the diffs near the view', async ({
    page,
    tasksServer,
  }) => {
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
    for (const f of files) {
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
            { kind: 'code', language: 'rust', line_offset: 1 },
          ),
        );
    }
    tasksServer.setReview('CR-2', reviewPage({ chains: [], view: { round: 3, against: null, files } }));
    await page.goto(reviewPath(tasksServer, '/CR-2'));
    await page.getByRole('tab', { name: /^Diffs/ }).click();
    await expect(page.locator('.review-file')).toHaveCount(300);
    await expect(page.locator('.review-file-card')).toHaveCount(300);
    await expect(diffOf(page, 'src/m00/f000.rs').locator('.diff-line').first()).toBeVisible();
    // Only the cards near the view have their diff
    expect(await page.locator('.diff-view').count()).toBeLessThan(60);
    // From the click: its diff made, its lines drawn, two frames after
    const phases = await page.evaluate(async () => {
      const frame = () => new Promise((r) => requestAnimationFrame(r));
      const start = performance.now();
      const since = () => Math.round(performance.now() - start);
      (document.querySelector('.review-file-name[data-file="src/m07/f150.rs"]') as HTMLElement).click();
      while (!document.querySelector('.diff-view[data-path="src/m07/f150.rs"]')) await frame();
      const made = since();
      while (!document.querySelector('.diff-view[data-path="src/m07/f150.rs"] .diff-line')) await frame();
      const drawn = since();
      await frame();
      await frame();
      return { made, drawn, total: performance.now() - start };
    });
    console.log('review file far down (ms):', JSON.stringify({ ...phases, total: Math.round(phases.total) }));
    expect(phases.total).toBeLessThan(BUDGET_MS);
  });
});
