import { expect, loadFixture, openDashboard, summary, test } from './helpers';

// ATM-575: every project's reviews on the overview's Reviews tab (Hakan: a fourth tab on the overview), with a project
// picker; and the reviews waiting on the user in the needs-you chip
const header = (n: number, status = 'waiting', title = `Review ${n}`) => ({
  id: `r${n}-${title}`,
  reference: `CR-${n}`,
  number: n,
  title,
  status,
  tasks: [],
  author: 'claude:a',
  reviewers: [],
  created: '2026-11-17T10:00:00Z',
  updated: '2026-11-17T10:00:00Z',
});
const entry = (n: number, title: string, more: Record<string, unknown> = {}) => ({
  review: { ...header(n, 'waiting', title), ...more },
  branch: null,
  chains: 1,
  open: 1,
  needs_you: false,
});

test.describe('Projects Overview: the Reviews tab', () => {
  test('every project’s reviews, each saying its project; the picker narrows to one; a row opens it in its project', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.addProject('second', 'standard', 'Second Project');
    tasksServer.setReviews({ reviews: [entry(1, 'Ours', { updated: '2026-11-18T10:00:00Z' })], you: 'Ana' });
    tasksServer.setProjectReviews('second', { reviews: [entry(1, 'Theirs')], you: 'Ana' });
    await page.goto('/projects');
    await page.locator('#item-reviews').click();
    await expect(page).toHaveURL(/\/reviews(\?|$)/);
    const rows = page.locator('#overview-reviews .review-row');
    await expect(rows).toHaveCount(2);
    // Both are CR-1, each in its own project
    await expect(rows.locator('.review-project')).toHaveText(['fixture', 'Second Project']);
    await expect(rows.nth(1).getByRole('link')).toHaveAttribute('href', /\/projects\/second\/reviews\/CR-1/);
    // The picker: one project's
    await page.locator('#overview-reviews-project').selectOption('second');
    await expect(rows).toHaveCount(1);
    await expect(rows.first()).toContainText('Theirs');
    await rows.first().getByRole('link').click();
    await expect(page).toHaveURL(/\/projects\/second\/reviews\/CR-1/);
  });

  // ATM-686 (Hakan): a review opened from every project's reviews goes back there, not to its project's list
  test('a review opened here goes back here, saying so', async ({ page, tasksServer }) => {
    tasksServer.addProject('second', 'standard', 'Second Project');
    tasksServer.setReviews({ reviews: [entry(1, 'Ours')], you: 'Ana' });
    tasksServer.setReview('CR-1', {
      review: header(1, 'waiting', 'Ours'),
      summary: '',
      branch: null,
      rounds: [{ number: 1, kind: { kind: 'files' }, files: [], by: 'claude:a', at: '' }],
      chains: [],
      view: { round: 1, against: null, files: [] },
      needs_you: false,
      you: 'Ana',
    });
    await page.goto('/reviews');
    await page.locator('#overview-reviews .review-row').first().getByRole('link').click();
    await expect(page.locator('#review-page')).toHaveAttribute('data-review', 'CR-1');
    const back = page.locator('.review-back');
    await expect(back).toHaveText('All reviews');
    await back.click();
    await expect(page).toHaveURL(/\/reviews(\?|$)/);
    await expect(page.locator('#overview-reviews')).toBeVisible();
    // Forward: the review again
    await page.goForward();
    await expect(page.locator('#review-page')).toHaveAttribute('data-review', 'CR-1');
  });

  test('the needs-you chip counts the reviews waiting on you: one, and nothing else, it opens', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.withOwnSummary(
      summary({
        needs_you: { review: 0, blocked_on_user: 0, together: 0, code_reviews: 1 },
        needs_you_reviews: [{ id: 'r7', reference: 'CR-7', title: 'The editor', why: 'review' }],
      }),
    );
    await page.goto('/projects');
    // On its card: one review waits, and no task: the chip opens it (several are listed under it, after the tasks)
    const chip = page.locator('.project-card').first().locator('.attention-chip');
    await expect(chip).toHaveText('1 needs you');
    await expect(chip).toHaveAttribute('title', 'Open CR-7 The editor');
    await chip.click();
    await expect(page).toHaveURL(/\/projects\/[^/]+\/reviews\/CR-7/);
  });

  test('on a project, a task in review through a review waiting on you is listed once, as the review (ATM-640)', async ({
    page,
    tasksServer,
  }) => {
    // T-004 is in review through CR-7, which waits on you; T-005 is blocked on you
    const data = loadFixture('v4');
    data.tasks = data.tasks.map((t) =>
      t.id === 'T-004'
        ? { ...t, code_reviews: [{ id: 'r7', ref: 'CR-7' }] }
        : t.id === 'T-006'
          ? { ...t, waiting_on: 'agent' }
          : t,
    );
    tasksServer.withOwnSummary(
      summary({
        needs_you: { review: 0, blocked_on_user: 1, together: 0, code_reviews: 1 },
        needs_you_reviews: [{ id: 'r7', reference: 'CR-7', title: 'The editor', why: 'review' }],
      }),
    );
    await openDashboard(page, tasksServer, data, { tab: 'list' });
    const chip = page.locator('#attention-count');
    await expect(chip).toHaveText('2 need you');
    await chip.click();
    const rows = page.locator('#needs-you-menu .needs-you-row');
    await expect(rows.locator('.needs-you-id')).toHaveText(['T-005', 'CR-7']);
  });

  test('a project whose reviews can’t be read is named; the others are listed', async ({ page, tasksServer }) => {
    tasksServer.addProject('second', 'standard', 'Second Project');
    tasksServer.setReviews({ reviews: [entry(1, 'Ours')], you: 'Ana' });
    await page.goto('/reviews');
    await expect(page.locator('#overview-reviews .review-row')).toHaveCount(1);
    await expect(page.locator('#overview-reviews .overview-reviews-problem')).toHaveText(
      "Couldn't read the reviews of Second Project.",
    );
  });
});
