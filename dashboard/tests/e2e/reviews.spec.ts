import type { Page } from '@playwright/test';
import {
  addressOf,
  expect,
  loadFixture,
  openDashboard,
  projectDocument,
  summary,
  test,
  type TasksServer,
} from './helpers';
import { E2E_SESSION } from './session.mjs';

// The reviews tab and a review's page (docs/code-review-design.md §3.11, ATM-574), and the diff viewer in it (§3.7,
// ATM-573): the fixture plays atm serve's review routes (the list, a review's page for a view, a round's sides by
// revision) and records the changes the page sends. A review's page is a window of its own: the list opens it in a
// browser tab, with its Overview, Diffs and Comments tabs.

/** A test that runs in WebKit too (the `webkit` project, ATM-655): the app's engine on macOS, where the diff viewer's
 * layout and scrolling have differed from Chromium's */
const WEBKIT_TOO = { tag: '@webkit' };

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

const header = (n: number, status = 'waiting', more: Record<string, unknown> = {}) => ({
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

  test('what waits for you (ATM-606): a count on the Reviews tab, and a review answered since you wrote marked', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.withOwnSummary(
      summary({ needs_you: { review: 0, blocked_on_user: 0, together: 0, code_reviews: 2 }, needs_you_reviews: [] }),
    );
    await openReviews(page, tasksServer);
    const tab = page.locator('#tab-reviews');
    await expect(page.locator('#reviews-waiting')).toHaveText('2');
    await expect(tab).toHaveAccessibleName('Reviews, 2 waiting for you');
    // CR-1, decided by nobody yet: answered since you last wrote
    tasksServer.setReviews({
      reviews: [
        { review: header(2), branch: 'dev/1', chains: 3, open: 2, needs_you: true, waiting: 'review' },
        {
          review: header(3, 'changes_requested'),
          branch: null,
          chains: 1,
          open: 1,
          needs_you: false,
          waiting: 'answered',
        },
      ],
      you: 'Ana',
    });
    await page.reload();
    const answered = page.locator('.review-row[data-review="CR-3"]');
    await expect(answered).toHaveClass(/needs-you/);
    await expect(answered.locator('.review-needs')).toHaveText('New reply');
    await expect(page.locator('.review-row[data-review="CR-2"] .review-needs')).toHaveText('Needs you');
    // Nothing waiting: no count
    tasksServer.withOwnSummary(summary({ needs_you: { review: 0, blocked_on_user: 0, together: 0, code_reviews: 0 } }));
    await page.reload();
    await expect(page.locator('.review-row')).toHaveCount(2);
    await expect(page.locator('#reviews-waiting')).toHaveCount(0);
    await expect(tab).toHaveAccessibleName('Reviews');
  });

  // ATM-610: unfiltered, the reviews in groups, what waits for you first; a filter shows the plain list
  test('the list in groups: waiting for your review first, how long it has waited; a filter lists them plainly', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer);
    const daysAgo = (d: number) => new Date(Date.now() - d * 86_400_000).toISOString();
    tasksServer.setReviews({
      reviews: [
        {
          review: { ...header(4, 'approved'), updated: daysAgo(1) },
          branch: null,
          chains: 1,
          open: 0,
          needs_you: false,
        },
        {
          review: header(2),
          branch: 'dev/1',
          chains: 3,
          open: 2,
          needs_you: true,
          waiting: 'review',
          waiting_since: daysAgo(3),
        },
        { review: header(3, 'changes_requested'), branch: null, chains: 1, open: 1, needs_you: false },
      ],
      you: 'Ana',
    });
    await page.reload();
    const groups = page.locator('.review-group');
    await expect(groups).toHaveCount(3);
    await expect(groups.locator('.review-group-title')).toHaveText([
      'Waiting for your review 1',
      'With open comments 1',
      'Recently decided 1',
    ]);
    await expect(groups.first().locator('.review-row')).toHaveAttribute('data-review', 'CR-2');
    await expect(groups.first().locator('.review-waited')).toHaveText('Waited 3 days');
    // A filter: the plain list, in the server's order
    await page.getByRole('button', { name: 'Open', exact: true }).click();
    await expect(groups).toHaveCount(0);
    await expect(page.locator('.review-row')).toHaveCount(3);
  });

  // ATM-617: CR-13 written by a newer atm, with a status this one doesn't know: a row saying so, not a missing review
  test('a review atm can’t read is a row of its own, saying why, after the others', async ({ page, tasksServer }) => {
    await openReviews(page, tasksServer);
    tasksServer.setReviews({
      reviews: [{ review: header(2), branch: null, chains: 0, open: 0, needs_you: false }],
      unreadable: [{ folder: 'CR-13-3b8e', reference: 'CR-13', problem: 'no status frozen' }],
      you: 'Ana',
    });
    await page.reload();
    const rows = page.locator('.review-row');
    await expect(rows).toHaveCount(2);
    const unreadable = rows.nth(1);
    await expect(unreadable).toHaveAttribute('data-unreadable', 'CR-13');
    await expect(unreadable).toHaveText("CR-13 Can't be read no status frozen A newer atm may have written it.");
    await expect(unreadable.getByRole('link')).toHaveCount(0);
  });

  test('opens with the last filter chosen, kept in the browser (not the search words); Clear forgets it', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer);
    const open = page.getByRole('button', { name: 'Open', exact: true });
    await page.getByRole('searchbox', { name: 'Search reviews' }).fill('names');
    await open.click();
    await expect.poll(() => tasksServer.reviewRequests.at(-1)).toBe('?q=names&status=open');
    await page.reload();
    await expect(open).toHaveAttribute('aria-pressed', 'true');
    await expect(page.getByRole('searchbox', { name: 'Search reviews' })).toHaveValue('');
    await expect.poll(() => tasksServer.reviewRequests.at(-1)).toBe('?status=open');
    await page.getByRole('button', { name: 'Clear' }).click();
    await expect(open).toHaveAttribute('aria-pressed', 'false');
    await page.reload();
    await expect(page.locator('.review-row')).toHaveCount(2);
    await expect(open).toHaveAttribute('aria-pressed', 'false');
    expect(tasksServer.reviewRequests.at(-1)).toBe('');
  });

  test('opens a review in place, and back; the one needing you next too', async ({ page, tasksServer }) => {
    await openReviews(page, tasksServer);
    await page.locator('.review-row[data-review="CR-2"] .review-link').click();
    await expect(page.locator('#review-page')).toHaveAttribute('data-review', 'CR-2');
    expect(shown(page)).toBe(reviewPath(tasksServer, '/CR-2'));
    expect(page.context().pages()).toHaveLength(1);
    await page.getByRole('link', { name: 'Reviews', exact: true }).click();
    await expect(page.locator('#review-list')).toBeVisible();
    expect(shown(page)).toBe(reviewPath(tasksServer));
    await page.getByRole('button', { name: /Next review needing you/ }).click();
    await expect(page.locator('#review-page')).toHaveAttribute('data-review', 'CR-2');
    await page.goBack();
    await expect(page.locator('#review-list')).toBeVisible();
  });

  // ATM-686 (Hakan): the back button goes back to where the review was opened from, saying where, not always to the
  // project's reviews; through the history, so Forward comes back to the review
  test.describe('the back button goes back where the review was opened from', () => {
    /** The fixture's project with T-004 in review through CR-2, on `tab`, CR-2's page served */
    async function withReviewedTask(page: Page, tasksServer: TasksServer, tab: 'kanban' | 'list') {
      const data = loadFixture('standard');
      Object.assign(
        data.tasks.find((t) => t.id === 'T-004')!,
        {
          status: 'in_review',
          code_reviews: [{ id: 'r2', ref: 'CR-2' }],
        },
      );
      tasksServer
        .setReviews({
          reviews: [{ review: header(2), branch: 'dev/1', chains: 3, open: 2, needs_you: true }],
          you: 'Ana',
        })
        .setReview('CR-2', reviewPage());
      await openDashboard(page, tasksServer, data, { tab });
    }
    const back = (page: Page) => page.locator('.review-back');

    test('from a Kanban card: back to the board; Forward to the review again', async ({ page, tasksServer }) => {
      await withReviewedTask(page, tasksServer, 'kanban');
      await page.locator('.kanban-card[data-id="T-004"] .card-review .review-link').click();
      await expect(page.locator('#review-page')).toHaveAttribute('data-review', 'CR-2');
      await expect(back(page)).toHaveText('Kanban');
      await back(page).click();
      await expect(page.locator('#kanban')).toBeVisible();
      expect(new URL(page.url()).pathname).toBe(`/projects/${tasksServer.projectId}/kanban`);
      await page.goForward();
      await expect(page.locator('#review-page')).toHaveAttribute('data-review', 'CR-2');
    });

    test('from a task’s details: back to them, open over the page they were on', async ({ page, tasksServer }) => {
      await withReviewedTask(page, tasksServer, 'list');
      await page.locator('.task-card[data-id="T-004"] .task-name').first().click();
      await expect(page.locator('#detail-overlay')).toHaveClass(/open/);
      await page.locator('#detail-review .review-link').first().click();
      await expect(page.locator('#review-page')).toHaveAttribute('data-review', 'CR-2');
      await expect(back(page)).toHaveText('T-004');
      await back(page).click();
      await expect(page.locator('#detail-overlay')).toHaveClass(/open/);
      const url = new URL(page.url());
      expect([url.pathname, url.searchParams.get('task')]).toEqual([
        `/projects/${tasksServer.projectId}/list`,
        'T-004',
      ]);
    });

    test('with a task’s details opened over the review and closed, and another file, still back to the board', async ({
      page,
      tasksServer,
    }) => {
      await withReviewedTask(page, tasksServer, 'kanban');
      await page.locator('.kanban-card[data-id="T-004"] .card-review .review-link').click();
      await expect(page.locator('#review-page')).toHaveAttribute('data-review', 'CR-2');
      // A task named in the review, its details over it, closed
      await page.locator('#review-page a[data-task="T-001"]').first().click();
      await expect(page.locator('#detail-overlay')).toHaveClass(/open/);
      await page.keyboard.press('Escape');
      await expect(page.locator('#detail-overlay')).not.toHaveClass(/open/);
      await expect(back(page)).toHaveText('Kanban');
      await back(page).click();
      await expect(page.locator('#kanban')).toBeVisible();
    });

    test('opened on its own (its address typed, a link from elsewhere): up to its reviews', async ({
      page,
      tasksServer,
    }) => {
      await openReviews(page, tasksServer, 'CR-2');
      await expect(back(page)).toHaveText('Reviews');
      await back(page).click();
      await expect(page.locator('#review-list')).toBeVisible();
    });
  });

  test('⌘ or Ctrl opens a review in a browser tab of its own; the list stays', async ({ page, tasksServer }) => {
    // The new tab's own page has no fixture behind it (it is the page's): its address is what is checked here
    await openReviews(page, tasksServer);
    const [opened] = await Promise.all([
      page.waitForEvent('popup'),
      page.locator('.review-row[data-review="CR-2"] .review-link').click({ modifiers: ['ControlOrMeta'] }),
    ]);
    await opened.waitForURL(/\/reviews\/CR-2/);
    expect(shown(opened)).toBe(reviewPath(tasksServer, '/CR-2'));
    expect(await opened.evaluate(() => window.opener)).toBeNull();
    await opened.close();
    await expect(page.locator('#review-list')).toBeVisible();
    expect(shown(page)).toBe(reviewPath(tasksServer));
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
          review: header(8, 'waiting', { reference: 'CR-8.1', title: 'Theirs' }),
          branch: null,
          chains: 0,
          open: 0,
          needs_you: false,
        },
        {
          review: header(9, 'waiting', { reference: 'CR-8.2', title: 'Mine' }),
          branch: null,
          chains: 0,
          open: 0,
          needs_you: false,
        },
        { review: header(1, 'waiting'), branch: null, chains: 0, open: 0, needs_you: false },
      ],
      you: 'Ana',
    });
    await page.goto(reviewPath(tasksServer, '/CR-8'));
    await expect(page.getByRole('alert')).toContainText('CR-8 is 2 reviews');
    const choices = page.getByRole('list', { name: 'The reviews CR-8 names' });
    await expect(choices.getByRole('listitem')).toHaveText([/CR-8\.1\s+Theirs/, /CR-8\.2\s+Mine/]);
    tasksServer.setReview(
      'CR-8.2',
      reviewPage({ review: header(9, 'waiting', { reference: 'CR-8.2', title: 'Mine' }) }),
    );
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
    await page.getByRole('link', { name: 'Reviews', exact: true }).click();
    await expect(page.locator('#review-list')).toBeVisible();
    await expect(page.locator('#tab-reviews')).toHaveAttribute('aria-selected', 'true');
  });

  // ATM-593: a task named in a review opens its details over the review, closed back to the review as it was
  test('a task named in a review opens its details over it; closed, the review as it was; ⌘-click a tab of its own', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer);
    const own = reviewPage();
    tasksServer.setReview(
      'CR-2',
      reviewPage({
        chains: [
          ...own.chains,
          {
            id: 't4',
            anchor: { round: 3, path: null, side: 'new', lines: null, quote: null },
            comments: [comment('c4', 'Like T-002, not ZZZ-9.')],
            placed: null,
            drifted: null,
          },
        ],
      }),
    );
    await page.goto(reviewPath(tasksServer, '/CR-2'));
    const overlay = page.locator('#detail-overlay');
    // The Overview's Tasks fact: T-001's details over the review, its address the review's with ?task=
    await page.locator('.review-task[data-task="T-001"]').click();
    await expect(overlay).toHaveClass(/open/);
    await expect(page.locator('#detail-panel h3')).toContainText('T-001');
    await expect.poll(() => shown(page)).toBe(reviewPath(tasksServer, '/CR-2?task=T-001'));
    await expect(page.locator('#review-page')).toBeVisible();
    // Closed: the review as it was
    await page.keyboard.press('Escape');
    await expect(overlay).not.toHaveClass(/open/);
    await expect.poll(() => shown(page)).toBe(reviewPath(tasksServer, '/CR-2'));
    await expect(page.getByRole('tab', { name: 'Overview' })).toHaveAttribute('aria-selected', 'true');
    // A key written in a comment: a link only for a task there is
    const chain = page.locator('.review-conversation [data-chain="t4"]');
    await expect(chain.getByRole('link', { name: 'T-002' })).toBeVisible();
    await expect(chain.getByRole('link', { name: 'ZZZ-9' })).toHaveCount(0);
    await expect(chain).toContainText('ZZZ-9');
    await chain.getByRole('link', { name: 'T-002' }).click();
    await expect(page.locator('#detail-panel h3')).toContainText('T-002');
    await page.keyboard.press('Escape');
    // ⌘-click (Ctrl elsewhere): a browser tab of its own, at the task's project page with its details
    const [tab] = await Promise.all([
      page.context().waitForEvent('page'),
      page
        .locator('.review-task[data-task="T-001"]')
        .click({ modifiers: [process.platform === 'darwin' ? 'Meta' : 'Control'] }),
    ]);
    await expect.poll(() => new URL(tab.url()).searchParams.get('task')).toBe('T-001');
    expect(new URL(tab.url()).pathname).toMatch(/\/list$/);
    await tab.close();
    await expect(overlay).not.toHaveClass(/open/);
  });

  test('the reviews list: a row’s tasks are links to their details, the row’s own link the review', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer);
    const row = page.locator('.review-row[data-review="CR-2"]');
    await row.locator('.review-task-link[data-task="T-001"]').click();
    await expect(page.locator('#detail-overlay')).toHaveClass(/open/);
    await expect(page.locator('#detail-panel h3')).toContainText('T-001');
    // Still the list under it
    await expect(page.locator('.review-row')).toHaveCount(2);
  });

  // ATM-586: links in a review's texts to the project's documents: a file of the round opens its diff, anything else
  // the document viewer
  test('a link in the summary or a comment: a file the round changed opens its diff, another document the viewer', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer);
    const own = reviewPage();
    tasksServer.setReview(
      'CR-2',
      reviewPage({
        summary: 'Trims the names: see [the design](docs/design.md) and [the change](src/a.rs).',
        chains: [
          ...own.chains,
          {
            id: 't4',
            anchor: { round: 3, path: null, side: 'new', lines: null, quote: null },
            comments: [comment('c4', 'As [the plan](docs/plan.md) says.')],
            placed: null,
            drifted: null,
          },
        ],
      }),
    );
    tasksServer.setDocument(projectDocument('docs/design.md', '# The design\n\nHow.\n'));
    tasksServer.setDocument(projectDocument('docs/plan.md', '# The plan\n\nWhen.\n'));
    await page.goto(reviewPath(tasksServer, '/CR-2'));
    const summary = page.locator('.review-overview .review-main');
    await summary.getByRole('link', { name: 'the design' }).click();
    await expect(page.locator('#doc-title')).toHaveText('The design');
    await page.keyboard.press('Escape');
    await expect(page.locator('#doc-title')).toHaveCount(0);
    // A comment's link, the same
    await page.locator('.review-conversation [data-chain="t4"]').getByRole('link', { name: 'the plan' }).click();
    await expect(page.locator('#doc-title')).toHaveText('The plan');
    await page.keyboard.press('Escape');
    // A file the round changed: its diff, on the Diffs tab
    await summary.getByRole('link', { name: 'the change' }).click();
    await expect(page.getByRole('tab', { name: /^Diffs/ })).toHaveAttribute('aria-selected', 'true');
    await expect(card(page, 'src/a.rs')).toBeInViewport();
    await expect(page.locator('#doc-title')).toHaveCount(0);
  });

  // ATM-588: Wrap, in Unified: a long line wraps at the column's width, its row as tall as it, its number on its first
  // visual line, the next line's row right under it; no sideways scroll
  test(
    'Wrap: a long line wraps in Unified, the next line level with its text',
    WEBKIT_TOO,
    async ({ page, tasksServer }) => {
      const long = `fn long() { let x = "${'word '.repeat(80)}"; }`;
      const text = `fn a() {}\n${long}\nfn b() {}\n`;
      const patch =
        'diff --git a/src/long.rs b/src/long.rs\nnew file mode 100644\n--- /dev/null\n+++ b/src/long.rs\n@@ -0,0 +1,3 @@\n' +
        text
          .trimEnd()
          .split('\n')
          .map((l) => `+${l}`)
          .join('\n') +
        '\n';
      tasksServer.setRevisionDocument(
        'head3',
        projectDocument('src/long.rs', text, { kind: 'code', language: 'rust', line_offset: 1 }),
      );
      await openReviews(page, tasksServer);
      tasksServer.setReview(
        'CR-2',
        reviewPage({ view: { round: 3, against: null, files: [file('src/long.rs', patch, 'added', null)] } }),
      );
      await page.goto(reviewPath(tasksServer, '/CR-2'));
      await page.getByRole('tab', { name: /^Diffs/ }).click();
      const wrap = page.getByRole('button', { name: 'Wrap' });
      // Unified, wrapped
      await page.getByRole('button', { name: 'Unified' }).click();
      await wrap.click();
      await expect(wrap).toHaveAttribute('aria-pressed', 'true');
      const diff = diffOf(page, 'src/long.rs');
      await expect(diff.locator('.diff-code.wrap')).toBeVisible();
      await expect(diff.locator('[data-compmd="ready"]').first()).toBeAttached();
      const rows = () =>
        diff.locator('.diff-code.wrap').evaluate((code) => {
          const lines = [...code.querySelectorAll<HTMLElement>('.diff-line')].map((l) => ({
            top: parseFloat(l.style.top),
            height: parseFloat(l.style.height),
          }));
          const pre = code.querySelector('.compmd-page pre') as HTMLElement | null;
          return { lines, sideways: pre ? pre.scrollWidth > pre.clientWidth + 1 : true };
        });
      await expect.poll(async () => (await rows()).lines[1]?.height ?? 0).toBeGreaterThan(40);
      const { lines, sideways } = await rows();
      expect(lines).toHaveLength(3);
      // The long line is several of the others tall; the next line's row starts where it ends
      expect(lines[1].height).toBeGreaterThan(lines[0].height * 2);
      expect(Math.abs(lines[2].top - (lines[1].top + lines[1].height))).toBeLessThan(2);
      expect(sideways).toBe(false);
      // Kept for the next page
      await page.reload();
      await page.getByRole('tab', { name: /^Diffs/ }).click();
      await expect(page.getByRole('button', { name: 'Wrap' })).toHaveAttribute('aria-pressed', 'true');
    },
  );

  // ATM-616 (Hakan): Wrap in Split too. A line wrapped on one side makes its counterpart on the other as tall (CompMD
  // 0.8.0's line heights), so the lines after it stay level across the two sides, and each side's numbers stay on
  // their lines' first visual lines
  test(
    'Wrap in Split: a line wrapped on one side keeps the two sides level after it',
    WEBKIT_TOO,
    async ({ page, tasksServer }) => {
      const long = `fn long() { let x = "${'word '.repeat(80)}"; }`;
      // Names of their own, each one token, so each side's text can be found where it is drawn
      const oldText = 'fn alpha() {}\nfn short() {}\nfn beta() {}\nfn gamma() {}\n';
      const newText = `fn alpha() {}\n${long}\nfn beta() {}\nfn gamma() {}\n`;
      const patch =
        'diff --git a/src/long.rs b/src/long.rs\n--- a/src/long.rs\n+++ b/src/long.rs\n@@ -1,4 +1,4 @@\n fn alpha() {}\n' +
        `-fn short() {}\n+${long}\n fn beta() {}\n fn gamma() {}\n`;
      const code = (text: string) =>
        projectDocument('src/long.rs', text, { kind: 'code', language: 'rust', line_offset: 1 });
      tasksServer.setRevisionDocument('base1', code(oldText)).setRevisionDocument('head3', code(newText));
      await openReviews(page, tasksServer);
      tasksServer.setReview(
        'CR-2',
        reviewPage({ view: { round: 3, against: null, files: [file('src/long.rs', patch)] } }),
      );
      await page.goto(reviewPath(tasksServer, '/CR-2'));
      await page.getByRole('tab', { name: /^Diffs/ }).click();
      await page.getByRole('button', { name: 'Split' }).click();
      const wrap = page.getByRole('button', { name: 'Wrap' });
      await expect(wrap).toBeEnabled();
      await wrap.click();
      await expect(wrap).toHaveAttribute('aria-pressed', 'true');
      const diff = diffOf(page, 'src/long.rs');
      await expect(diff.locator('.diff-old .diff-code.wrap [data-compmd="ready"]').first()).toBeAttached();
      await expect(diff.locator('.diff-new .diff-code.wrap [data-compmd="ready"]').first()).toBeAttached();
      /** Where `needle`'s text is drawn on each side (its first character's middle), and each side's gutter rows */
      const sides = () =>
        diff.evaluate((root) => {
          const at = (side: string, needle: string) => {
            for (const pre of root.querySelectorAll(`.diff-${side} .compmd-page pre`)) {
              const walker = document.createTreeWalker(pre, NodeFilter.SHOW_TEXT);
              for (let n = walker.nextNode() as Text | null; n; n = walker.nextNode() as Text | null) {
                const i = n.data.indexOf(needle);
                if (i < 0) continue;
                const range = document.createRange();
                range.setStart(n, i);
                range.setEnd(n, i + 1);
                const r = range.getBoundingClientRect();
                return (r.top + r.bottom) / 2;
              }
            }
            return null;
          };
          const rows = (side: string) =>
            [...root.querySelectorAll<HTMLElement>(`.diff-${side} .diff-code .diff-line`)].map((l) => ({
              top: l.getBoundingClientRect().top,
              height: l.getBoundingClientRect().height,
              number: l.querySelector('.diff-number')?.getBoundingClientRect(),
            }));
          return {
            b: [at('old', 'beta'), at('new', 'beta')],
            c: [at('old', 'gamma'), at('new', 'gamma')],
            old: rows('old'),
            now: rows('new'),
          };
        });
      // The long line wraps on the new side, and the old side's short line is made as tall: the lines after it level
      await expect
        .poll(async () => {
          const s = await sides();
          return s.b[0] !== null && s.b[1] !== null ? Math.abs(s.b[0] - s.b[1]) : 999;
        })
        .toBeLessThan(2);
      const s = await sides();
      expect(Math.abs(s.c[0]! - s.c[1]!)).toBeLessThan(2);
      // The changed line's rows: several lines tall on both sides, the same height
      expect(s.now[1].height).toBeGreaterThan(s.now[0].height * 2);
      expect(Math.abs(s.old[1].height - s.now[1].height)).toBeLessThan(2);
      // Each side's rows meet its text: fn b's row starts where the changed line's ends, on both sides
      for (const rows of [s.old, s.now]) expect(Math.abs(rows[2].top - (rows[1].top + rows[1].height))).toBeLessThan(2);
      // Hakan: no line drawn between two lines. Each row on whole pixels, from where the one before it ends: a row's
      // edge inside a pixel was half covered by both rows, and their tints drew a seam there
      const tiled = async () => {
        const t = await sides();
        for (const rows of [t.old, t.now]) {
          const offset = rows[0].top % 1;
          for (const [k, r] of rows.entries()) {
            expect(Math.abs(((r.top - offset) % 1) - Math.round((r.top - offset) % 1))).toBeLessThan(0.01);
            expect(Math.abs(r.height - Math.round(r.height))).toBeLessThan(0.01);
            if (k > 0) expect(r.top - (rows[k - 1].top + rows[k - 1].height)).toBeCloseTo(0, 2);
          }
        }
      };
      await tiled();
      // Unwrapped again: one line height each, the two sides level as before
      await wrap.click();
      await expect
        .poll(async () => {
          const t = await sides();
          return t.now[1]?.height ?? 0;
        })
        .toBeLessThan(30);
      await tiled();
    },
  );

  // ATM-652 (Hakan): the numbers weren't level with their lines, and Wrap moved them up or down. Each number's text is
  // centred on its line's first visual line, within 1 px, wrapped or not, in Unified and Split
  test(
    'every line number is level with its line’s text, and Wrap doesn’t move it',
    WEBKIT_TOO,
    async ({ page, tasksServer }) => {
      const long = `fn long() { let x = "${'word '.repeat(80)}"; }`;
      const text = `fn a() {}\n${long}\nfn b() {}\n\n// done\n`;
      const patch =
        'diff --git a/src/long.rs b/src/long.rs\nnew file mode 100644\n--- /dev/null\n+++ b/src/long.rs\n@@ -0,0 +1,5 @@\n' +
        text
          .slice(0, -1)
          .split('\n')
          .map((l) => `+${l}`)
          .join('\n') +
        '\n';
      tasksServer.setRevisionDocument(
        'head3',
        projectDocument('src/long.rs', text, { kind: 'code', language: 'rust', line_offset: 1 }),
      );
      await openReviews(page, tasksServer);
      tasksServer.setReview(
        'CR-2',
        reviewPage({ view: { round: 3, against: null, files: [file('src/long.rs', patch, 'added', null)] } }),
      );
      await page.goto(reviewPath(tasksServer, '/CR-2'));
      await page.getByRole('tab', { name: /^Diffs/ }).click();
      const diff = diffOf(page, 'src/long.rs');
      await expect(diff.locator('[data-compmd="ready"]').first()).toBeAttached();
      /** Each numbered line: its number's text centre less its text's (the first character of its first visual line) */
      const offsets = () =>
        diff.evaluate((view) =>
          [...view.querySelectorAll<HTMLElement>('.diff-code')].flatMap((run) => {
            const code = run.querySelector<HTMLElement>('.compmd-page pre > code');
            if (!code) return [];
            // Where each line starts in the code's text
            const starts: { node: Text; offset: number }[] = [];
            const walker = document.createTreeWalker(code, NodeFilter.SHOW_TEXT);
            let atStart = true;
            for (let node = walker.nextNode() as Text | null; node; node = walker.nextNode() as Text | null)
              for (let at = 0; at <= node.data.length; at++) {
                if (atStart && at < node.data.length) {
                  starts.push({ node, offset: at });
                  atStart = false;
                }
                if (node.data[at] === '\n') atStart = true;
              }
            const centre = (r: DOMRect) => (r.top + r.bottom) / 2;
            const range = document.createRange();
            return [...run.querySelectorAll<HTMLElement>('.diff-line')].flatMap((line, i) => {
              const number = line.querySelector('.diff-number span:last-child')?.firstChild;
              const start = starts[i];
              if (!number || !start || start.node.data[start.offset] === '\n') return [];
              range.selectNodeContents(number);
              const n = centre(range.getBoundingClientRect());
              range.setStart(start.node, start.offset);
              range.setEnd(start.node, start.offset + 1);
              return [Math.round((n - centre(range.getClientRects()[0])) * 10) / 10];
            });
          }),
        );
      const level = async (what: string) => {
        const found = await offsets();
        expect(found.length, what).toBeGreaterThanOrEqual(4);
        for (const d of found) expect(Math.abs(d), `${what}: ${JSON.stringify(found)}`).toBeLessThanOrEqual(1);
        return found;
      };
      await page.getByRole('button', { name: 'Split' }).click();
      await level('split');
      await page.getByRole('button', { name: 'Unified' }).click();
      const plain = await level('unified');
      const wrap = page.getByRole('button', { name: 'Wrap' });
      await wrap.click();
      await expect(diff.locator('.diff-code.wrap')).toBeVisible();
      await expect.poll(async () => (await offsets()).length).toBe(plain.length);
      const wrapped = await level('unified, wrapped');
      // Wrap moves no number against its text (it was 4 px: 1.5 above it, then 2.5 below)
      wrapped.forEach((d, i) =>
        expect(Math.abs(d - plain[i]), JSON.stringify({ plain, wrapped })).toBeLessThanOrEqual(1),
      );
    },
  );

  test(
    'Rendered: every line number is level with its text, a heading’s and a paragraph’s',
    WEBKIT_TOO,
    async ({ page, tasksServer }) => {
      await openReviews(page, tasksServer);
      tasksServer.setReview(
        'CR-2',
        reviewPage({ view: { round: 3, against: null, files: [file('docs/plan.md', PATCH_D)] } }),
      );
      await page.goto(reviewPath(tasksServer, '/CR-2?file=docs/plan.md'));
      const view = diffOf(page, 'docs/plan.md');
      await expect(view.locator('.diff-line').first()).toBeVisible();
      await page.getByRole('button', { name: 'Rendered' }).click();
      const newSide = view.locator('.diff-pane.diff-new');
      await newSide.getByRole('button', { name: 'Show 2 unchanged lines' }).click();
      await expect(newSide.locator('.doc-gutter-lines [data-line="1"]')).toBeVisible();
      // Each number's text centre less its block's first line of text's (the text, not the block's box)
      const offsets = await newSide.evaluate((side) => {
        const centre = (r: DOMRect) => (r.top + r.bottom) / 2;
        const range = document.createRange();
        return (
          [
            [1, 'h1'],
            [3, 'p:nth-of-type(1)'],
            [5, 'p:nth-of-type(2)'],
          ] as const
        ).map(([line, block]) => {
          const number = side.querySelector(`.doc-gutter-lines [data-line="${line}"]`)!.firstChild!;
          range.selectNodeContents(number);
          const n = centre(range.getBoundingClientRect());
          range.selectNodeContents(side.querySelector(`.compmd-page ${block}`)!);
          return Math.round((n - centre(range.getClientRects()[0])) * 10) / 10;
        });
      });
      for (const d of offsets) expect(Math.abs(d), JSON.stringify(offsets)).toBeLessThanOrEqual(1);
    },
  );

  // ATM-656 (Hakan): a jump to a file from the tree sometimes ended elsewhere: the files above it drawn after the jump
  // grew and pushed it out of view. The file's header stays where the jump put it (the pane's top, or as near as the
  // pane can scroll), frame after frame, while the files around it are drawn
  test(
    'a jump to a file stays on it while the files above it are drawn: one in the middle, the last, one scrolled at once',
    WEBKIT_TOO,
    async ({ page, tasksServer }) => {
      // Shaped as the long review of the performance tests (94 files, a few long ones, some long lines)
      const sizes = Array.from({ length: 94 }, (_, i) => (i % 31 === 0 ? 600 : i % 7 === 0 ? 120 : 12));
      const files = sizes.map((added, i) => {
        const path = `src/d${String(Math.floor(i / 10)).padStart(2, '0')}/f${String(i).padStart(3, '0')}.rs`;
        const body = Array.from(
          { length: added },
          (_, n) => `+let value_${n} = "${'x'.repeat(n % 5 === 0 ? 240 : 30)}";`,
        ).join('\n');
        return {
          path,
          old_path: path,
          status: 'modified',
          binary: false,
          patch: `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -1,3 +1,${added + 3} @@\n line 1\n line 2\n line 3\n${body}\n`,
          before: `b${i}`,
          after: `a${i}`,
        };
      });
      await openReviews(page, tasksServer);
      files.forEach((f, i) => {
        const head = ['line 1', 'line 2', 'line 3'];
        const added = f.patch
          .split('\n')
          .filter((l) => l.startsWith('+') && !l.startsWith('+++'))
          .map((l) => l.slice(1));
        const doc = (text: string) => projectDocument(f.path, text, { kind: 'code', language: 'rust', line_offset: 1 });
        tasksServer
          .setRevisionDocument(`b${i}`, doc(head.join('\n') + '\n'))
          .setRevisionDocument(`a${i}`, doc([...head, ...added].join('\n') + '\n'));
      });
      tasksServer.setReview('CR-2', reviewPage({ chains: [], view: { round: 3, against: null, files } }));
      await page.goto(reviewPath(tasksServer, '/CR-2'));
      await page.getByRole('tab', { name: /^Diffs/ }).click();
      await expect(diffOf(page, files[0].path).locator('.diff-line').first()).toBeVisible();
      /** The file's card top against the pane's, as each frame was drawn (read right after it: a read in the frame's own
       * callback comes before the page holds the file in place for it), for 1.5 s from when the jump brought it in */
      const followed = (path: string) =>
        page.evaluate(
          (p) =>
            new Promise<number[]>((done) => {
              const pane = document.querySelector<HTMLElement>('.review-diffs')!;
              const tops: number[] = [];
              let start: number | null = null;
              const after = new MessageChannel();
              after.port1.onmessage = () => read();
              const frame = () => after.port2.postMessage(null);
              const read = () => {
                const card = document.querySelector(`.review-file-card[data-file="${CSS.escape(p)}"]`);
                const top = card
                  ? Math.round(card.getBoundingClientRect().top - pane.getBoundingClientRect().top)
                  : null;
                if (start === null && top !== null && top > -40 && top < pane.clientHeight) start = performance.now();
                if (start !== null && top !== null) tops.push(top);
                if (start === null || performance.now() - start < 1500) requestAnimationFrame(frame);
                else done(tops);
              };
              requestAnimationFrame(frame);
            }),
          path,
        );
      // A file in the middle, the last file, and one scrolled a little at once (the reader's own scroll ends the jump's
      // hold: what is in view stays put all the same while the cards above it are drawn)
      for (const [target, scrolled] of [
        [files[47], false],
        [files.at(-1)!, false],
        [files[70], true],
      ] as const) {
        const name = target.path.split('/').at(-1)!;
        const tops = followed(target.path);
        await page.locator('.review-file-name', { hasText: name }).first().click();
        if (scrolled) {
          const box = (await page.locator('.review-diffs').boundingBox())!;
          await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
          await page.mouse.wheel(0, 30);
        }
        // From the frame it reached the place it ends at (the first frames may catch it on its way there, or the wheel's
        // own scroll still landing)
        const all = await tops;
        const last = all.at(-1)!;
        const seen = all.slice(all.findIndex((t) => Math.abs(t - last) < 4));
        const pane = (await page.locator('.review-diffs').boundingBox())!;
        expect(last, `${name}: ${JSON.stringify(seen)}`).toBeLessThan(pane.height / 2);
        // At the top (scrolled: as far below it as the wheel went, when WebKit took the wheel in time)
        if (target !== files.at(-1)) {
          expect(last, `${name}: ${JSON.stringify(seen)}`).toBeLessThan(16);
          expect(last, `${name}: ${JSON.stringify(seen)}`).toBeGreaterThan(scrolled ? -60 : -16);
        }
        // Once there, it stays: no frame after it arrived moves it (it moved 400 px for a frame in WebKit, or for good)
        for (const top of seen) expect(Math.abs(top - last), `${name}: ${JSON.stringify(seen)}`).toBeLessThan(4);
        await expect(diffOf(page, target.path)).toBeInViewport();
      }
    },
  );

  // ATM-689 (Hakan, CR-60's screenshots): a binary file isn't in the round's diff, but an image shows its sides, each
  // as its revision has it; another binary file stays listed, not shown
  test('an image file shows its images: a new one its after side, a changed one both; another binary file is listed', async ({
    page,
    tasksServer,
  }) => {
    const png = Buffer.from(
      'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=',
      'base64',
    );
    const binary = (path: string, status: string, before: string | null, after: string | null) => ({
      path,
      old_path: path,
      status,
      binary: true,
      patch: '',
      before,
      after,
    });
    tasksServer.setImage('docs/new.png', 'image/png', png, 'head3');
    tasksServer.setImage('docs/shot.png', 'image/png', png, 'base1');
    tasksServer.setImage('docs/shot.png', 'image/png', png, 'head3');
    await openReviews(page, tasksServer);
    tasksServer.setReview(
      'CR-2',
      reviewPage({
        chains: [],
        view: {
          round: 3,
          against: null,
          files: [
            binary('docs/new.png', 'added', null, 'head3'),
            binary('docs/shot.png', 'modified', 'base1', 'head3'),
            binary('bin/tool.wasm', 'modified', 'base1', 'head3'),
          ],
        },
      }),
    );
    await page.goto(reviewPath(tasksServer, '/CR-2'));
    await page.getByRole('tab', { name: /^Diffs/ }).click();
    const sides = (path: string) => card(page, path).locator('.review-image');
    // Added: its after side; changed: before and after, side by side
    await expect(sides('docs/new.png')).toHaveCount(1);
    await expect(sides('docs/new.png').locator('figcaption')).toHaveText('After');
    await expect(sides('docs/shot.png').locator('figcaption')).toHaveText(['Before', 'After']);
    // Each image loaded, from its revision
    for (const path of ['docs/new.png', 'docs/shot.png'])
      await expect
        .poll(() =>
          sides(path)
            .locator('img')
            .evaluateAll((imgs) => imgs.map((i) => (i as HTMLImageElement).naturalWidth)),
        )
        .toEqual(path === 'docs/new.png' ? [1] : [1, 1]);
    expect(tasksServer.fileRequests).toEqual(
      expect.arrayContaining(['docs/new.png@head3', 'docs/shot.png@base1', 'docs/shot.png@head3']),
    );
    // Not an image: listed, not shown
    await expect(card(page, 'bin/tool.wasm')).toContainText('A binary file: listed, not shown.');
    await expect(sides('bin/tool.wasm')).toHaveCount(0);
  });

  // ATM-626 (Hakan, the app): the Rendered view of a new Markdown document showed only about its first page
  for (const added of [true, false])
    test(`Rendered: a long ${added ? 'new' : 'changed'} document is drawn to its end`, async ({
      page,
      tasksServer,
    }) => {
      const section = (n: number) =>
        `## Section ${n}\n\n${'A paragraph of the plan, long enough to wrap. '.repeat(8)}\n`;
      const after = ['# Plan', ...Array.from({ length: 30 }, (_, n) => section(n + 1))].join('\n') + '\n';
      const before = added ? null : after.replace('## Section 30', '## Section thirty');
      const lines = (t: string) => t.trimEnd().split('\n');
      const doc = 'docs/long-plan.md';
      const patch = added
        ? `diff --git a/${doc} b/${doc}\nnew file mode 100644\n--- /dev/null\n+++ b/${doc}\n@@ -0,0 +1,${lines(after).length} @@\n` +
          lines(after)
            .map((l) => `+${l}`)
            .join('\n') +
          '\n'
        : (() => {
            const at = lines(after).indexOf('## Section 30') + 1;
            return `diff --git a/${doc} b/${doc}\n--- a/${doc}\n+++ b/${doc}\n@@ -${at},1 +${at},1 @@\n-## Section thirty\n+## Section 30\n`;
          })();
      tasksServer.setRevisionDocument('head3', projectDocument(doc, after, { kind: 'markdown', line_offset: 1 }));
      if (before)
        tasksServer.setRevisionDocument('base1', projectDocument(doc, before, { kind: 'markdown', line_offset: 1 }));
      await openReviews(page, tasksServer);
      tasksServer.setReview(
        'CR-2',
        reviewPage({
          view: {
            round: 3,
            against: null,
            files: [added ? file(doc, patch, 'added', null) : file(doc, patch)],
          },
        }),
      );
      await page.goto(reviewPath(tasksServer, '/CR-2'));
      await page.getByRole('tab', { name: /^Diffs/ }).click();
      await page.getByRole('button', { name: 'Rendered', exact: true }).click();
      const side = diffOf(page, doc).locator('.doc-sheet').last();
      // A new document is all one change, drawn whole; a changed one shows its blocks near the change (ATM-591)
      if (added) await expect(side.getByRole('heading', { name: 'Plan', exact: true })).toBeAttached();
      // The last section is drawn, and in view once scrolled to
      const last = side.getByRole('heading', { name: 'Section 30' });
      await expect(last).toBeAttached();
      await last.scrollIntoViewIfNeeded();
      await expect(last).toBeInViewport();
      if (added) await expect(side.getByRole('heading', { name: /^Section \d+$/ })).toHaveCount(30);
    });

  // ATM-625 (Hakan, the app): + on a wrapped long line of a Markdown file sent the pane to near the end of the review
  for (const wrapped of [true, false])
    test(
      `+ on a line ${wrapped ? 'wrapped' : 'not wrapped'} opens the comment box where it is; the pane stays`,
      WEBKIT_TOO,
      async ({ page, tasksServer }) => {
        const long = (n: number) => `Line ${n}: ${'a long sentence of the plan, '.repeat(12)}`;
        const texts = ['docs/a.md', 'docs/b.md', 'docs/c.md'].map(
          (path, k) => [path, Array.from({ length: 40 }, (_, n) => long(k * 100 + n)).join('\n') + '\n'] as const,
        );
        const patchOf = (path: string, text: string) =>
          `diff --git a/${path} b/${path}\nnew file mode 100644\n--- /dev/null\n+++ b/${path}\n@@ -0,0 +1,40 @@\n` +
          text
            .trimEnd()
            .split('\n')
            .map((l) => `+${l}`)
            .join('\n') +
          '\n';
        for (const [path, text] of texts)
          tasksServer.setRevisionDocument('head3', projectDocument(path, text, { kind: 'markdown', line_offset: 1 }));
        await openReviews(page, tasksServer);
        tasksServer.setReview(
          'CR-2',
          reviewPage({
            view: {
              round: 3,
              against: null,
              files: texts.map(([path, text]) => file(path, patchOf(path, text), 'added', null)),
            },
          }),
        );
        await page.goto(reviewPath(tasksServer, '/CR-2'));
        await page.getByRole('tab', { name: /^Diffs/ }).click();
        await page.getByRole('button', { name: 'Unified' }).click();
        const wrap = page.getByRole('button', { name: 'Wrap' });
        if (wrapped !== ((await wrap.getAttribute('aria-pressed')) === 'true')) await wrap.click();
        const diff = diffOf(page, 'docs/a.md');
        await expect(diff.locator('[data-compmd="ready"]').first()).toBeAttached();
        // A line in the middle of the first file, in the middle of the pane
        const line = diff.locator('.diff-line').nth(20);
        await line.evaluate((l) => l.scrollIntoView({ block: 'center' }));
        const pane = page.locator('.review-diffs');
        const before = await pane.evaluate((p) => p.scrollTop);
        // Over the line's text (CompMD's code lies over the rows; the run finds the line from the pointer's height)
        await line.hover({ force: true });
        await diff.locator('.diff-plus').click();
        const box = page.getByRole('textbox', { name: /Comment/ }).first();
        await expect(box).toBeFocused();
        // The comment box opens under the line, in view; the pane stays (it moved 444 px, 1,564 wrapped: the run's old
        // drawing, still all 40 lines while the box under its first half took the focus). In WebKit (ATM-655) it moved
        // by the run's height a few frames later, the run's text replaced by its highlighted text: WebKit's scroll
        // anchoring, anchored inside the run
        await page.waitForTimeout(300);
        const after = await pane.evaluate((p) => p.scrollTop);
        expect(Math.abs(after - before), `the pane moved from ${before} to ${after}`).toBeLessThan(40);
        await expect(box).toBeInViewport();
        const below = await box.evaluate(
          (b, l) => b.getBoundingClientRect().top - (l as HTMLElement).getBoundingClientRect().bottom,
          await line.elementHandle(),
        );
        expect(below).toBeGreaterThanOrEqual(-1);
        expect(below).toBeLessThan(120);
      },
    );

  // ATM-589: a run that ends with an empty line, before the fold that ends the file: the empty line is drawn, its
  // number beside it, and nothing over the fold
  test('a run ending with an empty line before the last fold: its number on its line, nothing over the fold', async ({
    page,
    tasksServer,
  }) => {
    const old = lines(40, (i) => (i === 31 ? '' : null));
    const now = lines(40, (i) => (i === 29 ? 'LINE 29' : i === 31 ? '' : null));
    const patch = [
      'diff --git a/src/e.rs b/src/e.rs',
      '--- a/src/e.rs',
      '+++ b/src/e.rs',
      '@@ -28,4 +28,4 @@',
      ' line 28',
      '-line 29',
      '+LINE 29',
      ' line 30',
      ' ',
      '',
    ].join('\n');
    tasksServer
      .setRevisionDocument(
        'base1',
        projectDocument('src/e.rs', old, { kind: 'code', language: 'rust', line_offset: 1 }),
      )
      .setRevisionDocument(
        'head3',
        projectDocument('src/e.rs', now, { kind: 'code', language: 'rust', line_offset: 1 }),
      );
    await openReviews(page, tasksServer);
    tasksServer.setReview('CR-2', reviewPage({ view: { round: 3, against: null, files: [file('src/e.rs', patch)] } }));
    await page.goto(reviewPath(tasksServer, '/CR-2'));
    await page.getByRole('tab', { name: /^Diffs/ }).click();
    const diff = diffOf(page, 'src/e.rs');
    for (const mode of ['Split', 'Unified']) {
      await page.getByRole('button', { name: mode, exact: true }).click();
      await expect(diff.locator('[data-compmd="ready"]').first()).toBeAttached();
      const fold = diff.locator('.diff-fold').last();
      await expect(fold).toContainText('Show 9 unchanged lines');
      const placed = await diff.evaluate((view) => {
        const fold = [...view.querySelectorAll('.diff-fold')].at(-1)!.getBoundingClientRect();
        const run = [...view.querySelectorAll('.diff-run')].at(-1)!;
        const code = run.querySelector('.compmd-page pre')!.getBoundingClientRect();
        const rows = [...run.querySelectorAll('.diff-code')[0].querySelectorAll('.diff-line')].map((l) =>
          l.getBoundingClientRect(),
        );
        const first = rows[0];
        return {
          fold: fold.top,
          last: rows.at(-1)!.bottom,
          code: code.bottom,
          rows: rows.length,
          // The code's lines are as tall as the first row: the empty last line drawn, not the rows squeezed to fit
          lineHeight: first.height,
          text: run.querySelector('.compmd-page pre code')!.textContent,
        };
      });
      // The empty line 31 is a row of its own (Unified: the changed line 29 is two), drawn in the code: the code ends
      // with it, the last row with the code, above the fold
      expect(placed.rows).toBe(mode === 'Split' ? 4 : 5);
      expect(placed.text?.endsWith('line 30\n\n')).toBe(true);
      expect(placed.last).toBeLessThanOrEqual(placed.fold + 1);
      expect(placed.lineHeight).toBeGreaterThan(18);
    }
  });

  // ATM-590: a long line's horizontal scroll bar stays below its code, above the fold row after it
  test('a long line scrolls sideways, its scroll bar below the code: the fold row after it stays clickable', async ({
    page,
    tasksServer,
  }) => {
    const longLine = `let x = "${'word '.repeat(120)}";`;
    const old = lines(40);
    const now = lines(40, (i) => (i === 30 ? longLine : null));
    const patch = [
      'diff --git a/src/w.rs b/src/w.rs',
      '--- a/src/w.rs',
      '+++ b/src/w.rs',
      '@@ -29,3 +29,3 @@',
      ' line 29',
      '-line 30',
      `+${longLine}`,
      ' line 31',
      '',
    ].join('\n');
    tasksServer
      .setRevisionDocument(
        'base1',
        projectDocument('src/w.rs', old, { kind: 'code', language: 'rust', line_offset: 1 }),
      )
      .setRevisionDocument(
        'head3',
        projectDocument('src/w.rs', now, { kind: 'code', language: 'rust', line_offset: 1 }),
      );
    await openReviews(page, tasksServer);
    tasksServer.setReview('CR-2', reviewPage({ view: { round: 3, against: null, files: [file('src/w.rs', patch)] } }));
    await page.goto(reviewPath(tasksServer, '/CR-2'));
    await page.getByRole('tab', { name: /^Diffs/ }).click();
    const diff = diffOf(page, 'src/w.rs');
    for (const mode of ['Split', 'Unified']) {
      await page.getByRole('button', { name: mode, exact: true }).click();
      await expect(diff.locator('[data-compmd="ready"]').first()).toBeAttached();
      const fold = diff.locator('.diff-fold').last();
      await expect(fold).toContainText('Show 9 unchanged lines');
      await fold.scrollIntoViewIfNeeded();
      const hit = await fold.evaluate((button) => {
        const box = button.getBoundingClientRect();
        const at = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
        // And just inside its top edge, where a scroll bar above it would reach
        const top = document.elementFromPoint(box.left + box.width / 2, box.top + 2);
        const scroller = [...button.closest('.diff-view')!.querySelectorAll('.compmd-page pre')].find(
          (pre) => pre.scrollWidth > pre.clientWidth,
        );
        // A scroll bar laid over the content (macOS's) can't be had headless: what keeps one off the fold is that a
        // run draws nothing past its end, and the fold row is above whatever it would
        const run = scroller?.closest('.diff-code');
        return {
          centre: at === button || button.contains(at),
          top: top === button || button.contains(top),
          scrolls: !!scroller,
          below: scroller ? scroller.getBoundingClientRect().bottom <= box.top + 0.5 : true,
          clipped: run ? getComputedStyle(run).overflowY === 'clip' : false,
          above: getComputedStyle(button).zIndex === '1',
        };
      });
      expect(hit).toEqual({ centre: true, top: true, scrolls: true, below: true, clipped: true, above: true });
    }
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
      'docs',
      /A\s*b\.md/,
      'src',
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

  // ATM-596 (Hakan: CompMD's CR-1 had pnpm-lock.yaml last in the tree, mid-way in the diffs)
  test('the diffs come in the tree’s order, not git’s; the next file is the tree’s next', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer);
    // As git lists them, by path: README.md between nothing and the folders
    tasksServer.setReview(
      'CR-2',
      reviewPage({
        view: {
          round: 3,
          against: null,
          files: [file('README.md', PATCH_C), file('docs/b.md', PATCH_B, 'added', null), file('src/a.rs', PATCH_A)],
        },
      }),
    );
    await page.goto(reviewPath(tasksServer, '/CR-2'));
    await page.getByRole('tab', { name: /^Diffs/ }).click();
    const tree = ['docs/b.md', 'src/a.rs', 'README.md'];
    await expect
      .poll(() => page.locator('.review-file-name').evaluateAll((els) => els.map((e) => e.getAttribute('data-file'))))
      .toEqual(tree);
    await expect
      .poll(() => page.locator('.review-file-card').evaluateAll((els) => els.map((e) => e.getAttribute('data-file'))))
      .toEqual(tree);
    // Moving from the first file goes down the tree
    await page.locator('.review-file-name[data-file="docs/b.md"]').click();
    await page.locator('.review-diffs').focus();
    await page.keyboard.press(']');
    await expect(page.locator('.review-file.current .review-file-name')).toHaveAttribute('data-file', 'src/a.rs');
    await page.keyboard.press(']');
    await expect(page.locator('.review-file.current .review-file-name')).toHaveAttribute('data-file', 'README.md');
  });

  // ATM-595: a folder folds, all of them at once, kept across a reload; going to a file in a folded folder opens it
  test('the tree’s folders fold: one, or all at once, kept across a reload; going to a file opens its folder', async ({
    page,
    tasksServer,
  }) => {
    await openDiffs(page, tasksServer);
    const rows = page.locator('.review-files li');
    await page.getByRole('button', { name: 'Collapse src' }).click();
    await expect(rows).toHaveText(['docs', /A\s*b\.md/, /src\s*1 file/, /M\s*README\.md/]);
    await expect(page.locator('.review-folder[data-folder="src"]')).toHaveAttribute('aria-expanded', 'false');
    await page.reload();
    await page.getByRole('tab', { name: /^Diffs/ }).click();
    await expect(rows).toHaveText(['docs', /A\s*b\.md/, /src\s*1 file/, /M\s*README\.md/]);
    await page.getByRole('button', { name: 'Collapse all folders' }).click();
    await expect(rows).toHaveText([/docs\s*1 file/, /src\s*1 file/, /M\s*README\.md/]);
    // The next file, from the keyboard: src/a.rs, in a folded folder, opens it
    await page.locator('.review-diffs').focus();
    await page.keyboard.press(']');
    await expect(page.locator('.review-file.current .review-file-name')).toHaveAttribute('data-file', /./);
    const current = await page.locator('.review-file.current .review-file-name').getAttribute('data-file');
    const folder = current!.split('/')[0];
    await expect(page.locator(`.review-folder[data-folder="${folder}"]`)).toHaveAttribute('aria-expanded', 'true');
    await page.getByRole('button', { name: 'Expand all folders' }).click();
    await expect(rows).toHaveCount(5);
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
    await page.getByRole('button', { name: 'src/a.rs · Line 3', exact: true }).click();
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
    await expect(newSide.locator('.diff-block-mark.diff-added')).not.toHaveCount(0);
    // The heading is more than three lines from the change: folded, as the source view folds it (ATM-591); opened
    await expect(newSide.locator('.doc-gutter-lines [data-line="1"]')).toHaveCount(0);
    await newSide.getByRole('button', { name: 'Show 2 unchanged lines' }).click();
    await expect(newSide.locator('.doc-gutter-lines [data-line="1"]')).toBeVisible();
    // The document viewer's own page on a desk (ATM-645: DocumentPage), all shown, its last text inside it; the numbers
    // in its margin, clear of the text
    await expect(newSide).toHaveClass(/doc-sheet/);
    await expect(newSide.locator('.doc-fold')).toHaveCount(0);
    const sheet = (await newSide.locator('.doc-page').boundingBox())!;
    const last = (await newSide.locator('.compmd-page > :last-child').boundingBox())!;
    expect(last.y + last.height).toBeLessThanOrEqual(sheet.y + sheet.height);
    const number = (await newSide.locator('.doc-gutter-lines [data-line="1"]').boundingBox())!;
    const heading = (await newSide.locator('.compmd-page h1').boundingBox())!;
    expect(number.x + number.width).toBeLessThanOrEqual(heading.x);
    await page.getByRole('button', { name: 'Unified' }).click();
    await expect(view.locator('.diff-pane')).toHaveCount(1);
    await expect(view.locator('.diff-pane')).toHaveClass(/diff-new/);
  });

  // ATM-651 (Hakan): a comment on rendered Markdown shows on the page, under its block (the page cut there), as the
  // source view shows a chain under its lines: open and resolved ones, in Split and Unified; a new one once sent
  test(
    'Rendered: each chain shows under its block, open or resolved; a new comment shows there once sent',
    WEBKIT_TOO,
    async ({ page, tasksServer }) => {
      const chain = (id: string, lines: [number, number], body: string, resolved: boolean | null = false) => ({
        id,
        anchor: { round: 3, path: 'docs/plan.md', side: 'new', lines, quote: null },
        comments: [{ ...comment(`c-${id}`, body, resolved) }],
        placed: lines,
        drifted: null,
      });
      const chains = [chain('t5', [5, 5], 'Why this ending?'), chain('t6', [3, 3], 'The first step is fine.', true)];
      await openReviews(page, tasksServer);
      tasksServer.setReview(
        'CR-2',
        reviewPage({ chains, view: { round: 3, against: null, files: [file('docs/plan.md', PATCH_D)] } }),
      );
      await page.goto(reviewPath(tasksServer, '/CR-2?file=docs/plan.md'));
      const view = diffOf(page, 'docs/plan.md');
      await expect(view.locator('.diff-line').first()).toBeVisible();
      await page.getByRole('button', { name: 'Rendered' }).click();
      const newSide = view.locator('.diff-pane.diff-new');
      /** The chain's box right under the paragraph it is on, on that side's page; none after the document */
      const under = async (id: string, text: string, room = 60) => {
        const box = newSide.locator(`.doc-after [data-chain="${id}"]`);
        await expect(box).toBeVisible();
        const para = (await newSide.locator('.compmd-page p', { hasText: text }).boundingBox())!;
        const at = (await box.boundingBox())!;
        expect(at.y).toBeGreaterThanOrEqual(para.y + para.height - 1);
        expect(at.y - (para.y + para.height)).toBeLessThan(room);
      };
      for (const layout of ['Split', 'Unified'] as const) {
        await page.getByRole('button', { name: layout }).click();
        await under('t5', 'New ending.');
        await under('t6', 'First step.');
        // The resolved one folded, as in the source view; nothing after the document
        await expect(newSide.locator('[data-chain="t6"]')).toHaveAttribute('data-resolved', '');
        await expect(view.locator('.diff-elsewhere [data-chain]')).toHaveCount(0);
      }
      // A new comment: written under its block, and shown there once the page has it
      await page.getByRole('button', { name: 'Split' }).click();
      const plus = newSide.locator('.doc-gutter-lines [data-line="3"]');
      await plus.hover();
      await newSide.getByRole('button', { name: 'Comment on line 3' }).click();
      const composer = view.locator('.doc-after .diff-composer');
      await expect(composer).toBeVisible();
      await composer.getByRole('textbox').fill('Say why.');
      tasksServer.setReview(
        'CR-2',
        reviewPage({
          chains: [...chains, chain('t7', [3, 3], 'Say why.')],
          view: { round: 3, against: null, files: [file('docs/plan.md', PATCH_D)] },
        }),
      );
      await composer.getByRole('button', { name: 'Comment' }).click();
      await expect.poll(() => tasksServer.reviewChanges.at(-1)?.path).toBe('/CR-2/comments');
      // Under the same block as the resolved one there, after it
      await under('t7', 'First step.', 140);
      // The lines below the chains numbered and marked where their text is now, not where it was before them
      const ending = (await newSide.locator('.compmd-page p', { hasText: 'New ending.' }).boundingBox())!;
      await expect
        .poll(async () => {
          const number = (await newSide.locator('.doc-gutter-lines [data-line="5"]').boundingBox())!;
          return Math.abs(number.y + number.height / 2 - (ending.y + ending.height / 2));
        })
        .toBeLessThan(4);
      // Its change mark on its text (the glyphs' box, inside the paragraph's line), not where the text was
      await expect
        .poll(() =>
          newSide
            .locator('.diff-block-mark.diff-added')
            .evaluateAll(
              (marks, top) => marks.some((m) => Math.abs(m.getBoundingClientRect().top - top) < 8),
              ending.y,
            ),
        )
        .toBe(true);
    },
  );

  test('Rendered (ATM-592): a document of several pages grows its page; its last paragraph is there, numbered beside it', async ({
    page,
    tasksServer,
  }) => {
    // 120 paragraphs (240 lines), the last one changed
    const paragraphs = (last: string) =>
      Array.from({ length: 120 }, (_, i) => (i === 119 ? last : `Paragraph ${i + 1} of a long plan.`)).join('\n\n') +
      '\n';
    const before = paragraphs('The old ending.');
    const after = paragraphs('The new ending.');
    const patch =
      'diff --git a/docs/long.md b/docs/long.md\n--- a/docs/long.md\n+++ b/docs/long.md\n' +
      '@@ -237,3 +237,3 @@\n Paragraph 119 of a long plan.\n \n-The old ending.\n+The new ending.\n';
    await openReviews(page, tasksServer);
    tasksServer
      .setRevisionDocument('base1', projectDocument('docs/long.md', before))
      .setRevisionDocument('head3', projectDocument('docs/long.md', after));
    tasksServer.setReview(
      'CR-2',
      reviewPage({
        view: { round: 3, against: null, files: [{ ...file('docs/long.md', patch), after: 'head3' }] },
      }),
    );
    await page.goto(reviewPath(tasksServer, '/CR-2?file=docs/long.md'));
    const view = diffOf(page, 'docs/long.md');
    await expect(view.locator('.diff-line').first()).toBeVisible();
    await page.getByRole('button', { name: 'Rendered' }).click();
    const newSide = view.locator('.diff-pane.diff-new');
    const ending = newSide.locator('.compmd-page p', { hasText: 'The new ending.' });
    // Folded (ATM-591): the change and what is near it, the rest behind one fold; opened, the whole document
    await expect(ending).toBeVisible();
    await expect(newSide.locator('.compmd-page p', { hasText: 'Paragraph 1 of' })).toHaveCount(0);
    await newSide.getByRole('button', { name: /^Show \d+ unchanged lines$/ }).click();
    await expect(newSide.locator('.compmd-page p', { hasText: 'Paragraph 1 of' })).toBeVisible();
    await ending.scrollIntoViewIfNeeded();
    await expect(ending).toBeVisible();
    // Inside its page, which is as tall as its document (more than one letter page)
    const sheet = (await newSide.locator('.doc-page').boundingBox())!;
    const text = (await ending.boundingBox())!;
    expect(text.y + text.height).toBeLessThanOrEqual(sheet.y + sheet.height);
    expect(sheet.height).toBeGreaterThan((sheet.width * 11) / 8.5 + 100);
    // Its line number beside it, marked as changed
    const number = newSide.locator('.doc-gutter-lines [data-line="239"]');
    await expect(number).toHaveClass(/marked/);
    const numberBox = (await number.boundingBox())!;
    expect(Math.abs(numberBox.y - text.y)).toBeLessThan(text.height);
    // ATM-645: in Unified, as wide as the document viewer's page (820 px where the desk has room), its margin the
    // viewer's: one component
    await page.getByRole('button', { name: 'Unified' }).click();
    const one = view.locator('.diff-pane.diff-new');
    const widths = await one.evaluate((desk) => {
      const page = desk.querySelector<HTMLElement>('.doc-page')!;
      const style = getComputedStyle(desk);
      const room = desk.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
      return {
        page: Math.round(page.getBoundingClientRect().width),
        room: Math.round(room),
        max: getComputedStyle(page).maxWidth,
      };
    });
    expect(widths.max).toBe('820px');
    expect(widths.page).toBe(Math.min(820, widths.room));
  });

  // ATM-627 (Hakan, the app): writing a comment on the rendered view, the document was drawn over the comment box
  for (const unified of [false, true])
    test(`Rendered${unified ? ', unified' : ''}: text selected opens its comment box, on top and near it`, async ({
      page,
      tasksServer,
    }) => {
      const paragraphs = (last: string) =>
        Array.from({ length: 120 }, (_, i) => (i === 119 ? last : `Paragraph ${i + 1} of a long plan.`)).join('\n\n') +
        '\n';
      const patch =
        'diff --git a/docs/long.md b/docs/long.md\n--- a/docs/long.md\n+++ b/docs/long.md\n' +
        '@@ -237,3 +237,3 @@\n Paragraph 119 of a long plan.\n \n-The old ending.\n+The new ending.\n';
      await openReviews(page, tasksServer);
      tasksServer
        .setRevisionDocument('base1', projectDocument('docs/long.md', paragraphs('The old ending.')))
        .setRevisionDocument('head3', projectDocument('docs/long.md', paragraphs('The new ending.')));
      tasksServer.setReview(
        'CR-2',
        reviewPage({
          view: { round: 3, against: null, files: [{ ...file('docs/long.md', patch), after: 'head3' }] },
        }),
      );
      await page.goto(reviewPath(tasksServer, '/CR-2?file=docs/long.md'));
      const view = diffOf(page, 'docs/long.md');
      await expect(view.locator('.diff-line').first()).toBeVisible();
      if (unified) await page.getByRole('button', { name: 'Unified' }).click();
      await page.getByRole('button', { name: 'Rendered' }).click();
      const newSide = view.locator('.diff-pane.diff-new');
      // The whole document, so the text picked is far from its end
      await newSide.getByRole('button', { name: /^Show \d+ unchanged lines$/ }).click();
      const picked = newSide.locator('.compmd-page p', { hasText: /^Paragraph 3 of/ });
      await picked.scrollIntoViewIfNeeded();
      // Its text selected, as a drag over it selects it
      await picked.evaluate((p) => {
        const range = document.createRange();
        range.selectNodeContents(p);
        window.getSelection()!.removeAllRanges();
        window.getSelection()!.addRange(range);
        p.dispatchEvent(new MouseEvent('mouseup', { bubbles: true }));
      });
      const box = page.getByRole('textbox', { name: /^Comment on/ });
      await expect(box).toBeFocused();
      await expect(box).toBeInViewport();
      // The box is what is drawn at its centre, not the document
      const composer = view.locator('.diff-composer');
      const centre = (await composer.boundingBox())!;
      const top = await page.evaluate(
        ([x, y]) => document.elementFromPoint(x, y)?.closest('.diff-composer') !== null,
        [centre.x + centre.width / 2, centre.y + centre.height / 2],
      );
      expect(top).toBe(true);
      // Under the text it comments on, not at the end of the document
      const text = (await picked.boundingBox())!;
      expect(centre.y).toBeGreaterThanOrEqual(text.y + text.height - 1);
      expect(centre.y - (text.y + text.height)).toBeLessThan(200);
    });

  // ATM-650 (Hakan): the rendered view takes comments as the source view does: the + of the line under the pointer
  test('Rendered: the + of a block under the pointer, clicked, opens the comment box under it, on its lines', async ({
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
    const newSide = view.locator('.diff-pane.diff-new');
    await newSide.getByRole('button', { name: 'Show 2 unchanged lines' }).click();
    // No + until the pointer is over a block; then the + of that block's line, beside its number
    await expect(newSide.locator('.diff-plus')).toHaveCount(0);
    const heading = newSide.locator('.compmd-page h1');
    const text = (await heading.boundingBox())!;
    await page.mouse.move(text.x + 20, text.y + text.height / 2);
    const plus = newSide.getByRole('button', { name: 'Comment on line 1' });
    await expect(plus).toBeVisible();
    await expect(newSide.locator('.diff-plus')).toHaveCount(1);
    const number = (await newSide.locator('.doc-gutter-lines [data-line="1"]').boundingBox())!;
    const plusBox = (await plus.boundingBox())!;
    expect(plusBox.x).toBeGreaterThanOrEqual(number.x + number.width);
    expect(plusBox.x + plusBox.width).toBeLessThanOrEqual(text.x);
    expect(Math.abs(plusBox.y + plusBox.height / 2 - (number.y + number.height / 2))).toBeLessThan(2);
    // A blank line between blocks has no +
    const blank = (await newSide.locator('.doc-gutter-lines [data-line="2"]').boundingBox())!;
    await page.mouse.move(text.x + 20, blank.y + blank.height / 2);
    await expect(newSide.locator('.diff-plus')).toHaveCount(0);
    // Clicked: the box, on line 1, right under the heading; the + stays while the comment is written
    await page.mouse.move(text.x + 20, text.y + text.height / 2);
    await plus.click();
    const composer = view.locator('.diff-composer');
    await expect(composer).toContainText('Line 1');
    await expect(page.getByRole('textbox', { name: 'Comment on line 1' })).toBeFocused();
    const box = (await composer.boundingBox())!;
    expect(box.y).toBeGreaterThanOrEqual(text.y + text.height - 1);
    expect(box.y - (text.y + text.height)).toBeLessThan(40);
    await page.mouse.move(0, 0);
    await expect(newSide.locator('.diff-plus.held')).toHaveCount(1);
    await expect(newSide.locator('.diff-block-mark.diff-picked')).not.toHaveCount(0);
    // Its number, pressed, picks it too
    await composer.getByRole('button', { name: 'Cancel' }).click();
    await expect(composer).toHaveCount(0);
    await newSide.locator('.doc-gutter-lines [data-line="3"]').click();
    await expect(view.locator('.diff-composer')).toContainText('Line 3');
  });

  test('Rendered: the + dragged over two blocks picks their lines; the comment is sent on them and shows in the source view', async ({
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
    const newSide = view.locator('.diff-pane.diff-new');
    await newSide.getByRole('button', { name: 'Show 2 unchanged lines' }).click();
    const first = (await newSide.locator('.compmd-page p', { hasText: 'First step.' }).boundingBox())!;
    const last = (await newSide.locator('.compmd-page p', { hasText: 'New ending.' }).boundingBox())!;
    await page.mouse.move(first.x + 10, first.y + first.height / 2);
    const plus = (await newSide.getByRole('button', { name: 'Comment on line 3' }).boundingBox())!;
    await page.mouse.move(plus.x + plus.width / 2, plus.y + plus.height / 2);
    await page.mouse.down();
    await page.mouse.move(plus.x + plus.width / 2, last.y + last.height / 2, { steps: 4 });
    await page.mouse.up();
    const composer = view.locator('.diff-composer');
    await expect(composer).toContainText('Lines 3–5');
    // The two blocks keep their + (the blank line between them draws nothing)
    await expect(newSide.locator('.diff-plus.held')).toHaveCount(2);
    await composer.getByRole('textbox').fill('Both steps.');
    await composer.getByRole('button', { name: 'Comment' }).click();
    await expect.poll(() => tasksServer.reviewChanges.length).toBe(1);
    expect(tasksServer.reviewChanges[0]).toMatchObject({
      method: 'POST',
      path: '/CR-2/comments',
      body: {
        path: 'docs/plan.md',
        lines: [3, 5],
        side: 'new',
        quote: 'First step.\n\nNew ending.',
        round: 3,
        body: 'Both steps.',
      },
    });
    // The page shows it again: in the source view, under the same lines
    tasksServer.setReview(
      'CR-2',
      reviewPage({
        view: { round: 3, against: null, files: [file('docs/plan.md', PATCH_D)] },
        chains: [
          {
            id: 'tr',
            anchor: { round: 3, path: 'docs/plan.md', side: 'new', lines: [3, 5], quote: 'First step.' },
            comments: [comment('cr', 'Both steps.')],
            placed: [3, 5],
            drifted: null,
          },
        ],
      }),
    );
    await page.evaluate(() => window.dispatchEvent(new Event('focus')));
    await page.getByRole('button', { name: 'Source' }).click();
    const chain = view.locator('[data-chain="tr"]');
    await expect(chain).toBeVisible();
    const lineFive = (await view.locator('.diff-new [data-row]').last().boundingBox())!;
    expect((await chain.boundingBox())!.y).toBeGreaterThanOrEqual(lineFive.y + lineFive.height - 1);
  });

  // ATM-591: the rendered view folds as the source view does: whole blocks near the change, the rest folded
  test('Rendered folds: the blocks near a change, with their own line numbers; the rest folded, each fold opened by a click', async ({
    page,
    tasksServer,
  }) => {
    // 40 paragraphs, each on its line with a blank line after: paragraph k on line 2k-1; paragraph 20 (line 39) changed
    const doc = (twenty: string) =>
      Array.from({ length: 40 }, (_, i) => (i === 19 ? twenty : `Paragraph ${i + 1}.`)).join('\n\n') + '\n';
    const patch =
      'diff --git a/docs/mid.md b/docs/mid.md\n--- a/docs/mid.md\n+++ b/docs/mid.md\n' +
      '@@ -36,7 +36,7 @@\n \n Paragraph 19.\n \n-The old twentieth.\n+The new twentieth.\n \n Paragraph 21.\n \n';
    await openReviews(page, tasksServer);
    tasksServer
      .setRevisionDocument('base1', projectDocument('docs/mid.md', doc('The old twentieth.')))
      .setRevisionDocument('head3', projectDocument('docs/mid.md', doc('The new twentieth.')));
    tasksServer.setReview(
      'CR-2',
      reviewPage({ view: { round: 3, against: null, files: [file('docs/mid.md', patch)] } }),
    );
    await page.goto(reviewPath(tasksServer, '/CR-2?file=docs/mid.md'));
    const view = diffOf(page, 'docs/mid.md');
    await expect(view.locator('.diff-line').first()).toBeVisible();
    await page.getByRole('button', { name: 'Rendered' }).click();
    const newSide = view.locator('.diff-pane.diff-new');
    const paragraphs = newSide.locator('.compmd-page p');
    // The change and the blocks within three lines of it, whole: paragraphs 19 (line 37), 20 (39) and 21 (41)
    await expect(paragraphs).toHaveText(['Paragraph 19.', 'The new twentieth.', 'Paragraph 21.']);
    // Their line numbers are the source's
    await expect(newSide.locator('.doc-gutter-lines [data-line="39"]')).toHaveClass(/marked/);
    await expect(newSide.locator('.doc-gutter-lines [data-line="37"]')).toBeVisible();
    await expect(newSide.locator('.doc-gutter-lines [data-line="1"]')).toHaveCount(0);
    // A fold before and one after, each saying how many lines it holds
    const folds = newSide.locator('.doc-fold');
    // (the document's 79 lines: lines 1–36 before, 43–79 after)
    await expect(folds).toHaveText(['Show 36 unchanged lines', 'Show 37 unchanged lines']);
    // Opened, the fold's blocks join the change's, numbered from line 1
    await folds.first().click();
    await expect(paragraphs.first()).toHaveText('Paragraph 1.');
    await expect(paragraphs).toHaveCount(21);
    await expect(newSide.locator('.doc-gutter-lines [data-line="1"]')).toBeVisible();
    await expect(newSide.locator('.doc-fold')).toHaveText(['Show 37 unchanged lines']);
    // The other side folds the same way, on its own
    await expect(view.locator('.diff-pane.diff-old .doc-fold')).toHaveCount(2);
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

  test('a comment on a line inside another comment’s lines is sent, and both show, each on its own', async ({
    page,
    tasksServer,
  }) => {
    const wide = {
      id: 'tw',
      anchor: { round: 3, path: 'src/a.rs', side: 'new', lines: [2, 4], quote: null },
      comments: [comment('cw', 'Three lines here.')],
      placed: [2, 4],
      drifted: null,
    };
    await openReviews(page, tasksServer);
    tasksServer.setReview('CR-2', reviewPage({ chains: [wide] }));
    await page.goto(reviewPath(tasksServer, '/CR-2?file=src/a.rs'));
    const view = diffOf(page, 'src/a.rs');
    await expect(view.locator('[data-chain="tw"]')).toBeVisible();
    await view.locator('.diff-new').getByRole('button', { name: 'Line 3', exact: true }).first().click();
    await page.locator('.diff-composer textarea').fill('Just this one.');
    await page.locator('.diff-composer').getByRole('button', { name: 'Comment' }).click();
    await expect.poll(() => tasksServer.reviewChanges.at(-1)).toMatchObject({ body: { lines: [3, 3], side: 'new' } });
    const narrow = {
      id: 'tn',
      anchor: { round: 3, path: 'src/a.rs', side: 'new', lines: [3, 3], quote: null },
      comments: [comment('cn', 'Just this one.')],
      placed: [3, 3],
      drifted: null,
    };
    tasksServer.setReview('CR-2', reviewPage({ chains: [wide, narrow] }));
    await page.evaluate(() => window.dispatchEvent(new Event('focus')));
    await expect(view.locator('[data-chain="tn"]')).toBeVisible();
    await expect(view.locator('[data-chain="tw"]')).toBeVisible();
  });

  // ATM-638: your own comments and replies are edited in place; another's have no Edit
  test('edit your own reply in place; another’s comment has no Edit', async ({ page, tasksServer }) => {
    await openReviews(page, tasksServer);
    const theirs = {
      ...comment('c1', 'Why upper case?'),
      author: 'claude:a',
      replies: [comment('c9', 'Because.', null)],
    };
    tasksServer.setReview(
      'CR-2',
      reviewPage({
        chains: [
          {
            id: 't1',
            anchor: { round: 3, path: null, side: 'new', lines: null, quote: null },
            comments: [theirs],
            placed: null,
            drifted: null,
          },
        ],
      }),
    );
    await page.goto(reviewPath(tasksServer, '/CR-2'));
    const conversation = page.locator('.review-conversation');
    // c1's own header: its replies are inside it, and your reply (c9) has Edit. Once c9 has it, the page knows you,
    // so c1 having none is said, not just not yet drawn
    const mine = conversation.locator('[data-comment="c9"]');
    await expect(mine.getByRole('button', { name: 'Edit comment' })).toBeVisible();
    await expect(
      conversation.locator('[data-comment="c1"] > .chain-meta').getByRole('button', { name: 'Edit comment' }),
    ).toHaveCount(0);
    // Pressed until its box opens: on a busy machine a press while the page is still settling was lost once (WebKit,
    // the whole spec on five workers; never alone, and a refresh while editing keeps the edit: the test below)
    await expect(async () => {
      if (!(await mine.getByLabel('Edit your comment').count()))
        await mine.getByRole('button', { name: 'Edit comment' }).click({ timeout: 1000 });
      await expect(mine.getByLabel('Edit your comment')).toBeVisible({ timeout: 1000 });
    }).toPass();
    await mine.getByLabel('Edit your comment').fill('Because of the style guide.');
    await mine.getByRole('button', { name: 'Save' }).click();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({ method: 'PATCH', path: '/CR-2/comments/c9', body: { body: 'Because of the style guide.' } });
    await expect(mine.getByLabel('Edit your comment')).toHaveCount(0);
  });

  test('a refresh of the review while you edit your reply keeps the edit', async ({ page, tasksServer }) => {
    await openReviews(page, tasksServer);
    const theirs = {
      ...comment('c1', 'Why upper case?'),
      author: 'claude:a',
      replies: [comment('c9', 'Because.', null)],
    };
    const page_ = reviewPage({
      chains: [
        {
          id: 't1',
          anchor: { round: 3, path: null, side: 'new', lines: null, quote: null },
          comments: [theirs],
          placed: null,
          drifted: null,
        },
      ],
    });
    tasksServer.setReview('CR-2', page_);
    await page.goto(reviewPath(tasksServer, '/CR-2'));
    const mine = page.locator('.review-conversation [data-comment="c9"]');
    await mine.getByRole('button', { name: 'Edit comment' }).click();
    await mine.getByLabel('Edit your comment').fill('Half written');
    const asked = tasksServer.reviewRequests.filter((r) => r === 'CR-2').length;
    // The review changed elsewhere (another comment), and the page reads it again
    tasksServer.setReview('CR-2', { ...page_, review: { ...page_.review, title: 'Review 2, retitled' } });
    await page.evaluate(() => window.dispatchEvent(new Event('focus')));
    await expect.poll(() => tasksServer.reviewRequests.filter((r) => r === 'CR-2').length).toBeGreaterThan(asked);
    await expect(page.getByText('Review 2, retitled').first()).toBeVisible();
    await expect(mine.getByLabel('Edit your comment')).toHaveValue('Half written');
  });

  // ATM-612: drafts, only the user's, sent together with their decision
  test('drafts: one added from a diff’s lines; shown marked and counted; edited, removed, and sent with Comment', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer);
    await page.goto(reviewPath(tasksServer, '/CR-2?file=src/a.rs'));
    const view = diffOf(page, 'src/a.rs');
    await view.locator('.diff-new').getByRole('button', { name: 'Line 2', exact: true }).first().click();
    await page.locator('.diff-composer textarea').fill('Later, maybe.');
    await page.locator('.diff-composer').getByRole('button', { name: 'Add to my review' }).click();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({
        method: 'POST',
        path: '/CR-2/drafts',
        body: { path: 'src/a.rs', lines: [2, 2], side: 'new', round: 3, body: 'Later, maybe.' },
      });
    // The page as atm then serves it: the draft among its comments, placed on its line
    const draft = {
      id: 'd1',
      at: '2026-11-19T10:00:00Z',
      body: 'Later, maybe.',
      kind: 'comment',
      path: 'src/a.rs',
      lines: [2, 2],
      side: 'new',
      quote: 'line 2',
      round: 3,
      placed: [2, 2],
      drifted: null,
    };
    tasksServer.setReview('CR-2', reviewPage({ drafts: [draft] }));
    await page.evaluate(() => window.dispatchEvent(new Event('focus')));
    const box = view.locator('[data-chain="draft-d1"]');
    await expect(box.locator('.chain-draft')).toHaveText('Draft');
    await expect(page.locator('#review-pending')).toHaveText('1');
    // With a draft, a new comment's box adds one more by default (⌘ Enter)
    await view.locator('.diff-new').getByRole('button', { name: 'Line 4', exact: true }).first().click();
    await page.locator('.diff-composer textarea').fill('And this.');
    await page.locator('.diff-composer textarea').press('ControlOrMeta+Enter');
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({ path: '/CR-2/drafts', body: { lines: [4, 4] } });
    // Edited in place, and removed
    await box.getByRole('button', { name: 'Edit draft' }).click();
    await box.getByLabel('Edit your draft').fill('Later.');
    await box.getByRole('button', { name: 'Save' }).click();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({ method: 'PATCH', path: '/CR-2/drafts/d1', body: { body: 'Later.' } });
    await box.getByRole('button', { name: 'Remove draft' }).click();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({ method: 'DELETE', path: '/CR-2/drafts/d1', body: {} });
    // Sent with Comment, no summary needed
    await page.locator('.review-submit-toggle').click();
    await expect(page.locator('#review-pending-note')).toHaveText(/1 draft will be sent with it/);
    await page.locator('[data-decision="comment"]').click();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({ method: 'POST', path: '/CR-2/decisions', body: { decision: 'comment' } });
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
    // The files in the tree's order (ATM-596): docs/b.md (one change), src/a.rs (two), README.md (one)
    const current = page.locator('.review-current-change');
    const next = page.locator('#review-next-change');
    await next.click();
    await expect(card(page, 'docs/b.md').locator('.review-current-change')).toBeInViewport();
    await next.click();
    await expect(card(page, 'src/a.rs').locator('.review-current-change')).toBeInViewport();
    const first = await current.getAttribute('data-row');
    await next.click();
    await expect(current).not.toHaveAttribute('data-row', first!);
    await expect(card(page, 'src/a.rs').locator('.review-current-change')).toBeInViewport();
    // Past src/a.rs's two changes, into the next file's
    await next.click();
    await expect(card(page, 'README.md').locator('.review-current-change')).toBeInViewport();
    await page.locator('#review-prev-change').click();
    await expect(card(page, 'src/a.rs').locator('.review-current-change')).toBeInViewport();
    // The keys do the same
    await page.keyboard.press('n');
    await expect(card(page, 'README.md').locator('.review-current-change')).toBeInViewport();
    await page.keyboard.press('p');
    await expect(card(page, 'src/a.rs').locator('.review-current-change')).toBeInViewport();
    await page.keyboard.press(']');
    await expect(page.locator('.review-file.current .review-file-name')).toHaveAttribute('data-file', 'README.md');
    await page.keyboard.press('v');
    await expect(page.locator('.review-files').getByLabel('Viewed README.md')).toBeChecked();
    await page.keyboard.press('[');
    await expect(page.locator('.review-file.current .review-file-name')).toHaveAttribute('data-file', 'src/a.rs');
    // Not while typing
    await page.locator('#review-round').focus();
    await page.keyboard.press(']');
    await expect(page.locator('.review-file.current .review-file-name')).toHaveAttribute('data-file', 'src/a.rs');
  });

  test('Prev and Next comment go from chain to chain across the files; c and Shift C too', async ({
    page,
    tasksServer,
  }) => {
    await page.setViewportSize({ width: 1200, height: 420 });
    const elsewhere = {
      id: 't4',
      anchor: { round: 3, path: 'README.md', side: 'new', lines: [1, 1], quote: null },
      comments: [comment('c4', 'And here.')],
      placed: [1, 1],
      drifted: null,
    };
    await openReviews(page, tasksServer);
    const base = reviewPage();
    tasksServer.setReview('CR-2', reviewPage({ chains: [...(base.chains as object[]), elsewhere] }));
    await page.goto(reviewPath(tasksServer, '/CR-2?file=src/a.rs'));
    await expect(diffOf(page, 'src/a.rs').locator('[data-chain="t1"]')).toBeVisible();
    const current = page.locator('.chain.review-current-change');
    // src/a.rs's two (t1 in place, t2 on lines folded away, below the file), then README.md's
    await page.locator('#review-next-comment').click();
    await expect(current).toHaveAttribute('data-chain', 't1');
    await page.locator('#review-next-comment').click();
    await expect(current).toHaveAttribute('data-chain', 't2');
    await page.locator('#review-next-comment').click();
    await expect(current).toHaveAttribute('data-chain', 't4');
    await expect(current).toBeInViewport();
    await page.locator('#review-prev-comment').click();
    await expect(current).toHaveAttribute('data-chain', 't2');
    await page.keyboard.press('c');
    await expect(current).toHaveAttribute('data-chain', 't4');
    await page.keyboard.press('Shift+C');
    await expect(current).toHaveAttribute('data-chain', 't2');
  });

  test('a chain hovered or clicked marks its lines; it shows its lines only, not its file or text', async ({
    page,
    tasksServer,
  }) => {
    await openDiffs(page, tasksServer);
    const chain = diffOf(page, 'src/a.rs').locator('[data-chain="t1"]');
    await expect(chain.locator('.chain-where')).toHaveText('Line 3');
    await expect(chain).not.toContainText('src/a.rs');
    await expect(chain.locator('.chain-quote')).toHaveCount(0);
    await chain.hover();
    await expect(diffOf(page, 'src/a.rs').locator('.diff-new .diff-line.commented')).toHaveCount(1);
    await page.mouse.move(0, 0);
    await expect(page.locator('.diff-line.commented')).toHaveCount(0);
    await chain.locator('.chain-body').click();
    await page.mouse.move(0, 0);
    await expect(diffOf(page, 'src/a.rs').locator('.diff-new .diff-line.commented')).toHaveCount(1);
  });

  test('every comment resolved: Submit review is Approve, and one click approves', async ({ page, tasksServer }) => {
    await openReviews(page, tasksServer);
    const resolved = reviewPage().chains.map((c) => ({
      ...c,
      comments: c.comments.map((x) => ({ ...x, resolved: true })),
    }));
    tasksServer.setReview('CR-2', reviewPage({ chains: resolved }));
    await page.goto(reviewPath(tasksServer, '/CR-2'));
    await expect(page.getByRole('button', { name: 'Submit review' })).toHaveCount(0);
    await page.locator('#review-approve').click();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({ method: 'POST', path: '/CR-2/decisions', body: { decision: 'approve' } });
  });

  test('a resolved comment is reopened from its fold', async ({ page, tasksServer }) => {
    await openReviews(page, tasksServer, 'CR-2');
    const own = page.locator('.review-conversation [data-chain="t3"]');
    await own.getByRole('button', { name: 'Reopen' }).click();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({ method: 'PATCH', path: '/CR-2/comments/c3', body: { resolved: false } });
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

  // ATM-611: a poke, from the review's page with a message, and from its row on the list
  test('poke: from the page with a message, and from the list; each is sent as the user', async ({
    page,
    tasksServer,
  }) => {
    await openReviews(page, tasksServer, 'CR-2');
    await page.locator('#review-poke').click();
    await page.getByLabel('Poke message').fill('Where are we?');
    await page.getByRole('form', { name: 'Poke' }).getByRole('button', { name: 'Poke', exact: true }).click();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({ method: 'POST', path: '/CR-2/pokes', body: { body: 'Where are we?' } });
    await expect(page.getByLabel('Poke message')).toHaveCount(0);
    // From the list, with no message
    await page.goto(reviewPath(tasksServer, ''));
    await page.locator('.review-poke[data-review="CR-2"]').click();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({ method: 'POST', path: '/CR-2/pokes', body: {} });
    await expect(page.locator('.review-poke[data-review="CR-2"]')).toHaveText('Poked');
  });

  // ATM-643: a poke has no Resolve, and is never open: with only a poke (answered) and a resolved comment left, the
  // review can be approved, as atm's rules let it
  test('a poke never keeps the review from an approval, nor counts as open', async ({ page, tasksServer }) => {
    await openReviews(page, tasksServer);
    const poke = {
      ...comment('c2', '', null),
      author: 'Ana',
      poke: true,
      replies: [comment('c3', 'Nearly there.', null)],
    };
    tasksServer.setReview(
      'CR-2',
      reviewPage({
        chains: [
          {
            id: 't1',
            anchor: { round: 3, path: null, side: 'new', lines: null, quote: null },
            comments: [{ ...comment('c1', 'Why upper case?'), resolved: true }],
            placed: null,
            drifted: null,
          },
          {
            id: 't2',
            anchor: { round: 3, path: null, side: 'new', lines: null, quote: null },
            comments: [poke],
            placed: null,
            drifted: null,
          },
        ],
      }),
    );
    await page.goto(reviewPath(tasksServer, '/CR-2'));
    await expect(page.locator('.review-conversation [data-comment="c3"]')).toBeVisible();
    await expect(page.locator('[data-comment="c2"] .chain-resolve')).toHaveCount(0);
    // Nothing open: Approve is the decision at hand, pressable
    const approve = page.locator('#review-approve');
    await expect(approve).toBeEnabled();
    // The Comments tab counts none open (ATM-614): its total only
    await expect(page.locator('#review-tab-comments .review-count-open')).toHaveCount(0);
    await approve.click();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({ method: 'POST', path: '/CR-2/decisions', body: { decision: 'approve' } });
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
    // Its comments open, Approve can't be pressed, and says why (ATM-601)
    const approve = page.getByRole('button', { name: 'Approve' });
    await expect(approve).toBeDisabled();
    await expect(approve).toHaveAccessibleDescription(/still open: resolve them first/);
    await page.getByLabel('Summary').fill('One more.');
    await page.getByRole('button', { name: 'Comment', exact: true }).click();
    await expect(page.getByRole('alert')).toHaveText("the review's author can't decide it");
    expect(tasksServer.reviewChanges.at(-1)).toMatchObject({ body: { decision: 'comment' } });
  });
});

test.describe('a live review (ATM-576)', () => {
  /** CR-5, live: one checkpoint, and the working copy now, after it, with `files` */
  const livePage = (...files: ReturnType<typeof file>[]) =>
    reviewPage({
      review: header(5, 'live'),
      rounds: [{ number: 1, kind: { kind: 'working' }, files: [], by: 'claude:a', at: '' }],
      chains: [],
      view: { round: 2, against: null, now: true, files },
      needs_you: false,
    });

  test('follows the working copy: a file written while the page is open shows; a comment is sent as on any review', async ({
    page,
    tasksServer,
  }) => {
    await page.clock.install();
    tasksServer.setReview('CR-5', livePage(file('src/a.rs', PATCH_A)));
    await openReviews(page, tasksServer, 'CR-5');
    await expect(page.locator('#review-following')).toHaveText('Following the working copy');
    await page.getByRole('tab', { name: /^Diffs/ }).click();
    await expect(card(page, 'src/a.rs')).toBeVisible();
    await expect(card(page, 'docs/b.md')).toHaveCount(0);
    await expect(page.locator('#review-round')).toHaveValue('now');
    // The agent writes another file: the next ask, a moment later, shows it
    tasksServer.setReview('CR-5', livePage(file('src/a.rs', PATCH_A), file('docs/b.md', PATCH_B, 'added', null)));
    await page.clock.fastForward(2000);
    await expect(card(page, 'docs/b.md')).toBeVisible();
    // A comment: the server keeps what was seen as a checkpoint (atm-lib's tests); the page sends it as on any review
    await page.getByRole('tab', { name: 'Overview' }).click();
    await page.getByRole('button', { name: 'Comment on the review' }).click();
    await page.locator('#review-comment-body').fill('Keep going.');
    await page.locator('.review-composer').getByRole('button', { name: 'Comment' }).click();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({ method: 'POST', path: '/CR-5/comments', body: { body: 'Keep going.' } });
  });

  // ATM-687 (Hakan): the working copy changing under a reader moves nothing and blanks nothing. Each change is a new
  // tree, so every file's `after` is another revision, its text mostly the same
  for (const view of ['source', 'rendered'] as const)
    for (const grown of ['far above', 'just above', 'here'] as const)
      test(
        `an update never blanks a file or moves either pane: the file being read stays put (${view}, ${grown === 'here' ? 'it grown below the place read' : `a file ${grown} it grown`})`,
        WEBKIT_TOO,
        async ({ page, tasksServer }) => {
          const head = ['line 1', 'line 2', 'line 3'];
          const added = (n: number, more = 0) => Array.from({ length: n + more }, (_, k) => `let value_${k} = ${k};`);
          /** File i: a Rust file adding 30 lines (more, `more`), or the plan, a Markdown document; its after side `rev` */
          const entry = (i: number, rev: string, more = 0) => {
            const md = i === 8;
            const path = md ? 'docs/plan.md' : `src/f${String(i).padStart(2, '0')}.rs`;
            const lines = md
              ? Array.from({ length: 30 + more }, (_, k) =>
                  k % 3 === 0 ? `## Part ${k}` : `Paragraph ${k} of the plan.`,
                )
              : added(30, more);
            const body = lines.map((l) => `+${l}`).join('\n');
            const patch = `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -1,3 +1,${lines.length + 3} @@\n line 1\n line 2\n line 3\n${body}\n`;
            const doc = (text: string) =>
              md
                ? projectDocument(path, text)
                : projectDocument(path, text, { kind: 'code', language: 'rust', line_offset: 1 });
            tasksServer
              .setRevisionDocument(`b${i}`, doc(head.join('\n') + '\n'))
              .setRevisionDocument(rev, doc([...head, ...lines].join('\n') + '\n'));
            return { ...file(path, patch, 'modified', `b${i}`), after: rev };
          };
          const files = (rev: string, grown = -1) =>
            Array.from({ length: 14 }, (_, i) => entry(i, `${rev}${i}`, i === grown ? 20 : 0));
          tasksServer.setReview('CR-5', livePage(...files('a')));
          await openReviews(page, tasksServer, 'CR-5');
          await page.getByRole('tab', { name: /^Diffs/ }).click();
          if (view === 'rendered') await page.getByRole('button', { name: 'Rendered', exact: true }).click();
          const reading = files('a')[8].path;
          // The plan chosen on the left, then read a little into it
          await page.locator('.review-file-name', { hasText: 'plan.md' }).first().click();
          await expect(diffOf(page, reading)).toBeInViewport();
          await page.waitForTimeout(600);
          const box = (await page.locator('.review-diffs').boundingBox())!;
          await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
          await page.mouse.wheel(0, 150);
          await page.waitForTimeout(400);
          /** Every frame for `ms`: the read card's top in the pane, both panes' scroll, and whether any diff is blank */
          const sample = (ms: number) =>
            page.evaluate(
              ([p, ms]) =>
                new Promise<{ top: number; list: number; tree: number; blank: number; changed: number }[]>((done) => {
                  const pane = document.querySelector<HTMLElement>('.review-diffs')!;
                  const tree = document.querySelector<HTMLElement>('.review-files')!;
                  const card = () => document.querySelector(`.review-file-card[data-file="${CSS.escape(p)}"]`)!;
                  let changed = 0;
                  const seen = new MutationObserver((records) => (changed += records.length));
                  seen.observe(card(), { childList: true, subtree: true, characterData: true });
                  const out: { top: number; list: number; tree: number; blank: number; changed: number }[] = [];
                  const start = performance.now();
                  const read = () => {
                    out.push({
                      top: Math.round(card().getBoundingClientRect().top - pane.getBoundingClientRect().top),
                      list: Math.round(pane.scrollTop),
                      tree: Math.round(tree.scrollTop),
                      blank: document.querySelectorAll('.diff-loading').length,
                      changed,
                    });
                    if (performance.now() - start < ms) requestAnimationFrame(read);
                    else {
                      seen.disconnect();
                      done(out);
                    }
                  };
                  requestAnimationFrame(read);
                }),
              [reading, ms] as const,
            );
          // The working copy changes: a new tree (every file's after side renamed, its text the same), and a file grows
          // by 20 lines at its end: one far above the one read (not drawn), the one just above it (drawn), or the one
          // read, below the place read (in WebKit, anchored inside the card, the pane jumped 1,033 px). The page asks
          // again within LIVE_MS
          const which = { 'far above': 2, 'just above': 7, here: 8 }[grown];
          const frames = sample(4500);
          tasksServer.setReview('CR-5', livePage(...files('n', which)));
          const all = await frames;
          // The update arrived: the grown file's header counts its 50 lines
          await expect(card(page, files('n')[which].path).locator('.review-file-head')).toContainText('+50');
          const first = all[0];
          const why = JSON.stringify(all.filter((f, i) => i === 0 || JSON.stringify(f) !== JSON.stringify(all[i - 1])));
          for (const f of all) {
            expect(f.blank, why).toBe(0);
            expect(Math.abs(f.top - first.top), why).toBeLessThan(3);
            expect(f.tree, why).toBe(first.tree);
          }
          // The plan's own text didn't change: nothing in its card was drawn again
          if (grown !== 'here') expect(all.at(-1)!.changed, why).toBe(0);
        },
      );

  test('ends with Approve or Close review, never sent back; Close sends why', async ({ page, tasksServer }) => {
    tasksServer.setReview('CR-5', livePage(file('README.md', PATCH_C)));
    await openReviews(page, tasksServer, 'CR-5');
    await expect(page.getByRole('button', { name: 'Approve', exact: true })).toBeEnabled();
    await page.getByRole('button', { name: 'Other decisions' }).click();
    await expect(page.getByRole('button', { name: 'Request changes' })).toHaveCount(0);
    const close = page.getByRole('button', { name: 'Close review' });
    await expect(close).toBeDisabled();
    await page.getByLabel('Summary').fill('Going another way.');
    await close.click();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({ method: 'PATCH', path: '/CR-5', body: { status: 'closed', reason: 'Going another way.' } });
  });

  test('New live review on the list opens one against the commit given, about its task, and shows it', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.setReview('CR-5', livePage(file('README.md', PATCH_C)));
    await openReviews(page, tasksServer);
    tasksServer.answerReviewChanges(201, { reference: 'CR-5' });
    await page.getByRole('button', { name: 'New live review' }).click();
    const form = page.getByRole('form', { name: 'New live review' });
    await form.getByLabel('Title').fill('Watching the port');
    await form.getByLabel('Task').fill('T-001');
    await form.getByLabel('Against the commit').fill('main');
    await form.getByRole('button', { name: 'Start' }).click();
    await expect
      .poll(() => tasksServer.reviewChanges.at(-1))
      .toMatchObject({
        method: 'POST',
        path: '',
        body: { title: 'Watching the port', start: 'live', change: { base: 'main' }, tasks: ['T-001'] },
      });
    await expect.poll(() => shown(page)).toBe(reviewPath(tasksServer, '/CR-5'));
    await expect(page.locator('#review-following')).toBeVisible();
  });
});

// Timed (@timed): in a project of their own, after the others (playwright.config.ts, ATM-641)
test.describe('performance', { tag: '@timed' }, () => {
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

  // ATM-644: the pointer over a long file's lines, its + following it: each step from the pointer's move until two
  // frames after, on the old side of Split and in Unified wrapped
  test('the + follows the pointer over a long file at once, on either side, wrapped too (measured)', async ({
    page,
    tasksServer,
  }) => {
    const path = 'src/long.rs';
    const n = 600;
    const old = lines(n);
    const now = lines(n, (i) => (i % 3 === 0 ? `CHANGED ${i} ${'y'.repeat(i % 5 === 0 ? 200 : 20)}` : null));
    const patch =
      `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -1,${n} +1,${n} @@\n` +
      old
        .trimEnd()
        .split('\n')
        .map((l, i) => {
          const changed = now.trimEnd().split('\n')[i];
          return changed === l ? ` ${l}` : `-${l}\n+${changed}`;
        })
        .join('\n') +
      '\n';
    const f = { path, old_path: path, status: 'modified', binary: false, patch, before: 'base1', after: 'head3' };
    await openReviews(page, tasksServer);
    tasksServer
      .setRevisionDocument('base1', projectDocument(path, old, { kind: 'code', language: 'rust', line_offset: 1 }))
      .setRevisionDocument('head3', projectDocument(path, now, { kind: 'code', language: 'rust', line_offset: 1 }));
    tasksServer.setReview('CR-2', reviewPage({ chains: [], view: { round: 3, against: null, files: [f] } }));
    await page.goto(reviewPath(tasksServer, '/CR-2'));
    await page.getByRole('tab', { name: /^Diffs/ }).click();
    await expect(diffOf(page, path).locator('.diff-line').first()).toBeVisible();

    /** The pointer over 80 lines of the first run of `side` in turn: each step's time until two frames after, and
     * whether the + was on that line then */
    const sweep = (side: string) =>
      page.evaluate(async (side) => {
        const frame = () => new Promise((r) => requestAnimationFrame(r));
        const code = document.querySelector<HTMLElement>(`.diff-view .diff-${side} .diff-code, .diff-view .diff-code`)!;
        const rows = [...code.querySelectorAll<HTMLElement>('.diff-line:not(.diff-pad)')].slice(0, 80);
        const steps: number[] = [];
        let followed = 0;
        // Each forced style read is a restyle of the whole page in the app's WebKit (ATM-644): the pointer forces none
        let reads = 0;
        const read = window.getComputedStyle;
        window.getComputedStyle = (el, pseudo) => {
          reads++;
          return read.call(window, el, pseudo);
        };
        for (const row of rows) {
          const box = row.getBoundingClientRect();
          const start = performance.now();
          code.dispatchEvent(
            new MouseEvent('mousemove', { bubbles: true, clientX: box.left + 40, clientY: box.top + box.height / 2 }),
          );
          await frame();
          await frame();
          steps.push(performance.now() - start);
          if (row.querySelector('.diff-plus')) followed++;
        }
        window.getComputedStyle = read;
        steps.sort((a, b) => a - b);
        return {
          steps: steps.length,
          followed,
          reads,
          median: Math.round(steps[steps.length >> 1]),
          slowest: Math.round(steps.at(-1)!),
          total: Math.round(steps.reduce((a, b) => a + b, 0)),
        };
      }, side);

    // Every run drawn first (CompMD highlights them in turn, ATM-587), so what is counted is the pointer's alone
    const drawn = () =>
      expect(diffOf(page, path).locator('.compmd[data-compmd="loading"]')).toHaveCount(0, { timeout: 20_000 });
    await drawn();
    const split = await sweep('old');
    await page.getByRole('button', { name: 'Unified' }).click();
    await page.getByRole('button', { name: 'Wrap' }).click();
    await expect(diffOf(page, path).locator('.diff-code.wrap').first()).toBeVisible();
    await drawn();
    const wrapped = await sweep('unified');
    console.log('pointer over lines (ms):', JSON.stringify({ split, wrapped }));
    for (const s of [split, wrapped]) {
      expect(s.followed).toBe(s.steps);
      expect(s.reads, 'computed-style reads while the pointer moves').toBe(0);
      // Two frames are ~33 ms; a step is that and its own work
      expect(s.median).toBeLessThan(BUDGET_MS / 4);
      expect(s.slowest).toBeLessThan(BUDGET_MS);
    }
  });

  // ATM-646: a review shaped as CompMD's CR-7 (15 files, one long: ~100,000 px of cards not made yet; CR-7's was 500,000,
  // which only costs this test time). A jump to its last file must not restyle the whole page over and over: with
  // CompMD 0.7.1 each CompMD page opened or closed set the document's adopted style sheets and read a computed style,
  // a full restyle each (~1 s in the app's WebKit). CompMD 0.8.0 shares one style sheet per document and reads the
  // colour scheme only for a frame or a component (ATM-648): the jump restyles nothing, and the page itself forces none
  test('a jump to the last file of a long review restyles the page only for the CompMD pages it shows (measured)', async ({
    page,
    tasksServer,
  }) => {
    const sizes = Array.from({ length: 15 }, (_, i) => (i === 7 ? 5_000 : i % 3 === 0 ? 120 : 30));
    const files = sizes.map((added, i) => {
      const path = `src/f${String(i).padStart(2, '0')}.rs`;
      const body = Array.from({ length: added }, (_, n) => `+let v${n} = ${n};`).join('\n');
      return {
        path,
        old_path: path,
        status: 'modified',
        binary: false,
        patch: `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -1,3 +1,${added + 3} @@\n line 1\n line 2\n line 3\n${body}\n`,
        before: `b${i}`,
        after: `a${i}`,
      };
    });
    await page.addInitScript(() => {
      const w = window as unknown as { restyles: Record<string, number> };
      w.restyles = { adopted: 0, computed: 0, pages: 0 };
      const desc = Object.getOwnPropertyDescriptor(Document.prototype, 'adoptedStyleSheets');
      if (desc?.set)
        Object.defineProperty(Document.prototype, 'adoptedStyleSheets', {
          ...desc,
          set(v) {
            w.restyles.adopted++;
            desc.set!.call(this, v);
          },
        });
      const read = window.getComputedStyle.bind(window);
      window.getComputedStyle = (el, pseudo) => {
        w.restyles.computed++;
        return read(el, pseudo);
      };
      // A CompMD page opened: its element marked compmd-page by CompMD
      new MutationObserver((list) => {
        for (const m of list)
          if (m.type === 'attributes' && (m.target as Element).classList.contains('compmd-page')) w.restyles.pages++;
      }).observe(document, { subtree: true, attributes: true, attributeFilter: ['class'] });
    });
    await openReviews(page, tasksServer);
    files.forEach((f) => {
      const added = f.patch
        .split('\n')
        .filter((l) => l.startsWith('+') && !l.startsWith('+++'))
        .map((l) => l.slice(1));
      const head = ['line 1', 'line 2', 'line 3'];
      tasksServer
        .setRevisionDocument(
          f.before,
          projectDocument(f.path, head.join('\n') + '\n', { kind: 'code', language: 'rust', line_offset: 1 }),
        )
        .setRevisionDocument(
          f.after,
          projectDocument(f.path, [...head, ...added].join('\n') + '\n', {
            kind: 'code',
            language: 'rust',
            line_offset: 1,
          }),
        );
    });
    tasksServer.setReview('CR-2', reviewPage({ chains: [], view: { round: 3, against: null, files } }));
    await page.goto(reviewPath(tasksServer, '/CR-2'));
    await page.getByRole('tab', { name: /^Diffs/ }).click();
    await expect(diffOf(page, files[0].path).locator('.diff-line').first()).toBeVisible();
    const height = await page.locator('.review-diffs').evaluate((el) => el.scrollHeight);
    expect(height).toBeGreaterThan(100_000);
    await page.waitForTimeout(1500);
    const counts = () =>
      page.evaluate(() => ({ ...(window as unknown as { restyles: Record<string, number> }).restyles }));
    const before = await counts();
    // The last file, from the tree; then a moment for whatever follows the jump
    await page.locator('.review-file-name').last().click();
    await expect(diffOf(page, files.at(-1)!.path).locator('.diff-line').first()).toBeVisible();
    await page.waitForTimeout(3000);
    const after = await counts();
    const jump = {
      adopted: after.adopted - before.adopted,
      computed: after.computed - before.computed,
      pages: after.pages - before.pages,
    };
    console.log('jump to the last file, restyle triggers:', JSON.stringify({ before, jump }));
    // The cards in view and near it, a page a side of each run: no more (the cards passed over aren't made)
    expect(jump.pages).toBeGreaterThan(0);
    expect(jump.pages).toBeLessThanOrEqual(8);
    // No page sets the document's sheets as it opens or closes (CompMD 0.8.0's one sheet a document; with 0.7.1, 10 for
    // 7 pages), and nothing reads a computed style (0.7.1 read 28: its scheme, four a page; the review's own code forced
    // 68 more before ATM-644)
    expect(jump.adopted).toBe(0);
    expect(jump.computed).toBe(0);
  });

  // ATM-587: a review shaped as CompMD's CR-1 (94 files, ~3,000 lines added: a few long files, most short, some long
  // lines), measured: the Diffs tab opened, then its pane scrolled from top to bottom
  test('a long review (94 files, ~3,000 lines): the Diffs tab opens and its pane scrolls, measured', async ({
    page,
    tasksServer,
  }) => {
    const sizes = Array.from({ length: 94 }, (_, i) => (i % 31 === 0 ? 600 : i % 7 === 0 ? 120 : 12));
    const files = sizes.map((added, i) => {
      const path = `src/d${String(Math.floor(i / 10)).padStart(2, '0')}/f${String(i).padStart(3, '0')}.rs`;
      const longLine = (n: number) => `let value_${n} = "${'x'.repeat(n % 5 === 0 ? 240 : 30)}";`;
      const body = Array.from({ length: added }, (_, n) => `+${longLine(n)}`).join('\n');
      return {
        path,
        old_path: path,
        status: 'modified',
        binary: false,
        patch: `diff --git a/${path} b/${path}\n--- a/${path}\n+++ b/${path}\n@@ -1,3 +1,${added + 3} @@\n line 1\n line 2\n line 3\n${body}\n`,
        before: `b${i}`,
        after: `a${i}`,
      };
    });
    await openReviews(page, tasksServer);
    files.forEach((f, i) => {
      const head = ['line 1', 'line 2', 'line 3'];
      const added = f.patch
        .split('\n')
        .filter((l) => l.startsWith('+') && !l.startsWith('+++'))
        .map((l) => l.slice(1));
      const doc = (text: string) => projectDocument(f.path, text, { kind: 'code', language: 'rust', line_offset: 1 });
      tasksServer
        .setRevisionDocument(`b${i}`, doc(head.join('\n') + '\n'))
        .setRevisionDocument(`a${i}`, doc([...head, ...added].join('\n') + '\n'));
    });
    tasksServer.setReview('CR-2', reviewPage({ chains: [], view: { round: 3, against: null, files } }));
    await page.goto(reviewPath(tasksServer, '/CR-2'));
    await expect(page.locator('#review-tab-diffs')).toBeVisible();
    const opened = await page.evaluate(async () => {
      const frame = () => new Promise((r) => requestAnimationFrame(r));
      const start = performance.now();
      (document.querySelector('#review-tab-diffs') as HTMLElement).click();
      while (!document.querySelector('.diff-view .diff-line')) await frame();
      const firstDrawn = performance.now() - start;
      await frame();
      await frame();
      return { firstDrawn: Math.round(firstDrawn), settled: Math.round(performance.now() - start) };
    });
    await expect(page.locator('.review-file-card')).toHaveCount(94);
    // Scrolled in steps of a screen, as a reader does, each step waiting for its frame: the slowest frames, long
    // tasks and the whole time
    const scrolled = await page.evaluate(async () => {
      const pane = document.querySelector('.review-diffs') as HTMLElement;
      const long: number[] = [];
      const observer = new PerformanceObserver((list) => list.getEntries().forEach((e) => long.push(e.duration)));
      try {
        observer.observe({ type: 'longtask', buffered: false });
      } catch {
        // no long-task timing in this browser
      }
      const frame = () => new Promise((r) => requestAnimationFrame(r));
      const frames: number[] = [];
      const start = performance.now();
      let last = start;
      for (let steps = 0; steps < 2000 && pane.scrollTop + pane.clientHeight < pane.scrollHeight - 2; steps++) {
        pane.scrollTop += pane.clientHeight;
        await frame();
        const now = performance.now();
        frames.push(now - last);
        last = now;
      }
      observer.disconnect();
      // The first steps run into the page settling (the cards near the first screen made once it is idle, in one
      // frame of about 100 ms); each step's frame after them
      const settling = Math.round(Math.max(0, ...frames.slice(0, 3)));
      const afterSettled = Math.round(Math.max(0, ...frames.slice(3)));
      frames.sort((a, b) => b - a);
      return {
        steps: frames.length,
        total: Math.round(performance.now() - start),
        settling,
        afterSettled,
        slowest: frames.slice(0, 3).map(Math.round),
        median: Math.round(frames[Math.floor(frames.length / 2)] ?? 0),
        longTasks: long.length,
        longest: Math.round(Math.max(0, ...long)),
        rows: document.querySelectorAll('.diff-line').length,
        // Rows drawn more than FAR (3,200 px) and a screen away from the view: the page keeps none
        farRows: (() => {
          const view = pane.getBoundingClientRect();
          const reach = 3200 + view.height;
          return [...document.querySelectorAll('.review-file-card')].filter((card) => {
            const box = card.getBoundingClientRect();
            const far = box.bottom < view.top - reach || box.top > view.bottom + reach;
            return far && card.querySelector('.diff-line');
          }).length;
        })(),
        nodes: document.querySelectorAll('*').length,
        // Cards standing shorter than a header (ATM-615: the pane's column squeezed every skipped card to its border)
        collapsed: (() => {
          const head = (document.querySelector('.review-file-head') as HTMLElement).offsetHeight;
          return [...document.querySelectorAll('.review-file-card')].filter(
            (card) => card.getBoundingClientRect().height < head,
          ).length;
        })(),
      };
    });
    console.log('long review (ms):', JSON.stringify({ ...opened, ...scrolled }));
    // The end is reached; budgets follow from the measurements (docs/review-performance.md): the first file drawn
    // and the page settled within the interaction budget (measured: about 65 and 110 ms), no step after that a frame
    // of more than half of it (measured: about 30 ms), and the page holding the rows near the view, not every file's
    // (measured: 30 to 200 at the end; 7,788 before ATM-587)
    await expect(diffOf(page, files.at(-1)!.path)).toBeAttached();
    expect(opened.firstDrawn).toBeLessThan(BUDGET_MS);
    expect(scrolled.settling).toBeLessThan(BUDGET_MS);
    expect(scrolled.afterSettled).toBeLessThan(BUDGET_MS / 2);
    // Rows bounded by where the view is, not by a count: a 600-line file in view is 1,200 rows in split
    expect(scrolled.farRows).toBe(0);
    expect(scrolled.collapsed).toBe(0);

    // ATM-620: memory stays bounded by where the view is, not by every card ever made. Back to the top in steps of a
    // screen, the page settled, a garbage collection forced: the DOM nodes still alive (DevTools' count, detached ones
    // too) within a bound set from the measurements
    await page.evaluate(async () => {
      const pane = document.querySelector('.review-diffs') as HTMLElement;
      const frame = () => new Promise((r) => requestAnimationFrame(r));
      for (let steps = 0; steps < 2000 && pane.scrollTop > 0; steps++) {
        pane.scrollTop -= pane.clientHeight;
        await frame();
      }
    });
    await page.waitForTimeout(1000);
    const cdp = await page.context().newCDPSession(page);
    await cdp.send('Performance.enable');
    await cdp.send('HeapProfiler.collectGarbage');
    const { metrics } = await cdp.send('Performance.getMetrics');
    const alive = metrics.find((m) => m.name === 'Nodes')!.value;
    console.log('long review, DOM nodes alive after down and back:', alive, 'on the page:', scrolled.nodes);
    // Measured (docs/diff-viewer-profile.md): 93,240 before the fixes (a render after its text was gone kept its
    // card; content-visibility kept every card's old layout), 21,840 after them; the page itself about 5,400
    expect(alive).toBeLessThan(30_000);
  });
});
