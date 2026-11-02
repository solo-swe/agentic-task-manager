import path from 'node:path';
import type { Page } from '@playwright/test';
import { board, boardCard, expect, loadFixture, openDashboard, sprintNumbers, summary, test } from './helpers';

/**
 * Screenshot of the view shown, its panel only: not the tabs or the title bar above it, so a change there (spacing,
 * the tabs' look) doesn't change every view's baseline (T-345). The section is scrolled into view first, and the
 * title bar is given time to compact (it does once the page scrolls, from a scroll handler): otherwise, on a busy
 * machine, the screenshot could land between the scroll and the compacting, with everything shifted.
 */
async function sectionScreenshot(page: Page, name: string) {
  const section = page.locator('.view-panels');
  await section.scrollIntoViewIfNeeded();
  await expect
    .poll(() => page.evaluate(() => document.querySelector('.topbar')!.classList.contains('scrolled') === scrollY > 8))
    .toBe(true);
  await expect(section).toHaveScreenshot(name, {
    // A long view reaches under the sticky title bar (and the status bar): its versions are masked, like on the
    // full-page shots, so a release doesn't change these baselines
    mask: [page.locator('#version-chip'), page.locator('#footer-version')],
    stylePath: [
      path.join(import.meta.dirname, 'screenshot.css'),
      path.join(import.meta.dirname, 'section-screenshot.css'),
    ],
  });
}

// The version chip and the footer's version are masked: they show the version (and a "new" marker) and would
// otherwise change every page baseline on each release; whats-new.spec and footer.spec cover them. They also get a
// fixed width for the screenshot, so a longer version number (0.9.0 -> 0.10.0) doesn't move the mask's edge.
// The auto-refresh interval box is masked too (when its menu is open): headless Chromium places the text inside a
// number input at a slightly different sub-pixel position from run to run. refresh.spec covers its value.
// Baselines are Linux only (T-345): made and checked in the pinned Playwright container, the same as CI's, so there
// is one set and no round trip through CI. With Docker running:
//   npm run test:e2e:visual           compare
//   npm run test:e2e:visual:update    regenerate after an intended UI change (once a sprint, in the release task)
// Elsewhere (macOS) the tests skip. Screenshots are of parts (a panel, a menu, a card) rather than whole pages, so one
// change touches few baselines; the standard fixture's page, dark and light, is the only whole page.
test.skip(process.platform !== 'linux', 'Linux baselines: npm run test:e2e:visual (Docker)');

for (const fixture of ['standard', 'large-90', 'empty', 'edge-cases']) {
  test.describe(`visual: ${fixture}`, () => {
    test.beforeEach(async ({ page, tasksServer }) => {
      // On the graph (the default is the Kanban since T-251)
      await openDashboard(page, tasksServer, fixture, { tab: 'graph' });
      await expect(page.locator('#subtitle')).toContainText('loaded from tasks.json');
    });

    if (fixture === 'standard') {
      test('full page', async ({ page }) => {
        await expect(page).toHaveScreenshot(`${fixture}-page.png`, {
          fullPage: true,
          mask: [
            page.locator('#version-chip'),
            page.locator('#footer-version'),
            page.locator('#auto-refresh-interval'),
          ],
          stylePath: path.join(import.meta.dirname, 'screenshot.css'),
        });
      });
    }

    test('dependency graph', async ({ page }) => {
      await sectionScreenshot(page, `${fixture}-graph.png`);
    });
  });
}

test('visual: detail panel', async ({ page, tasksServer }) => {
  await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
  await page.locator('.task-card[data-id="T-004"] .task-name').click();
  await expect(page.locator('#detail-panel')).toHaveScreenshot('standard-detail.png');
});

for (const fixture of ['standard', 'kanban']) {
  test(`visual: kanban board (${fixture})`, async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer.withAtmSummary(), fixture);
    await page.locator('#tab-kanban').click();
    await expect(page.locator('#kanban .kanban-card').first()).toBeVisible();
    await sectionScreenshot(page, `${fixture}-kanban.png`);
  });
}

test('visual: progress tab', async ({ page, tasksServer }) => {
  await page.clock.setFixedTime(new Date('2026-04-06T12:00:00Z'));
  await openDashboard(page, tasksServer, 'progress');
  await page.locator('#tab-progress').click();
  await expect(page.locator('#burndown svg')).toBeVisible();
  await sectionScreenshot(page, 'progress-progress.png');
});

test('visual: list tab', async ({ page, tasksServer }) => {
  await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
  await sectionScreenshot(page, 'standard-list.png');
});

// The Agents tab and the top-bar widgets depend on the time: fixtures/agents.md, with a fixed clock
const AGENTS_NOW = new Date('2026-04-01T12:00:00Z');

test('visual: agents tab', async ({ page, tasksServer }) => {
  await page.clock.setFixedTime(AGENTS_NOW);
  await openDashboard(page, tasksServer, 'agents', { tab: 'agents' });
  await expect(page.locator('.agent-bar').first()).toBeVisible();
  await sectionScreenshot(page, 'agents-agents.png');
});

test('visual: agents tab, zoomed in', async ({ page, tasksServer }) => {
  await page.clock.setFixedTime(AGENTS_NOW);
  await openDashboard(page, tasksServer, 'agents', { tab: 'agents' });
  await page.getByRole('button', { name: 'Zoom in' }).click();
  await sectionScreenshot(page, 'agents-agents-zoomed.png');
});

test('visual: project tab', async ({ page, tasksServer }) => {
  await page.clock.setFixedTime(AGENTS_NOW);
  await openDashboard(page, tasksServer, 'agents', { tab: 'project' });
  await expect(page.locator('#project-info')).toBeVisible();
  await sectionScreenshot(page, 'agents-project.png');
});

test('visual: progress and activity details', async ({ page, tasksServer }) => {
  await page.clock.setFixedTime(AGENTS_NOW);
  await openDashboard(page, tasksServer, 'agents');
  await page.locator('#progress-card').click();
  await expect(page.locator('#progress-panel')).toHaveScreenshot('agents-progress-panel.png');
  await page.locator('#activity-card').click();
  await expect(page.locator('#activity-panel')).toHaveScreenshot('agents-activity-panel.png');
});

test('visual: top bar with the filter panel open', async ({ page, tasksServer }) => {
  await openDashboard(page, tasksServer, 'standard');
  await page.locator('#filters-toggle').click();
  await page.locator('.filter-btn[data-filter="type"][data-value="feature"]').click();
  await page.locator('#task-search').fill('user');
  await expect(page.locator('header.topbar')).toHaveScreenshot('standard-topbar-filters.png', {
    mask: [page.locator('#version-chip')],
  });
  // The popup itself (T-323), which hangs below the title bar
  await expect(page.locator('#filter-panel')).toHaveScreenshot('standard-filter-panel.png');
});

// FR-2.6: the main views in light mode (the OS setting is light; Playwright emulates it).
test.describe('visual: light mode', () => {
  test.use({ colorScheme: 'light' });

  test('full page', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'graph' });
    await expect(page.locator('html')).toHaveAttribute('data-mode', 'light');
    await expect(page).toHaveScreenshot('light-standard-page.png', {
      fullPage: true,
      mask: [page.locator('#version-chip'), page.locator('#footer-version'), page.locator('#auto-refresh-interval')],
      stylePath: path.join(import.meta.dirname, 'screenshot.css'),
    });
  });

  test('dependency graph', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'large-90', { tab: 'graph' });
    await sectionScreenshot(page, 'light-large-90-graph.png');
  });

  test('kanban board', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer.withAtmSummary(), 'kanban');
    await page.locator('#tab-kanban').click();
    await expect(page.locator('#kanban .kanban-card').first()).toBeVisible();
    await sectionScreenshot(page, 'light-kanban.png');
  });

  test('progress tab', async ({ page, tasksServer }) => {
    await page.clock.setFixedTime(new Date('2026-04-06T12:00:00Z'));
    await openDashboard(page, tasksServer, 'progress');
    await page.locator('#tab-progress').click();
    await expect(page.locator('#burndown svg')).toBeVisible();
    await sectionScreenshot(page, 'light-progress.png');
  });

  test('agents tab', async ({ page, tasksServer }) => {
    await page.clock.setFixedTime(AGENTS_NOW);
    await openDashboard(page, tasksServer, 'agents', { tab: 'agents' });
    await expect(page.locator('.agent-bar').first()).toBeVisible();
    await sectionScreenshot(page, 'light-agents.png');
  });

  test('activity details', async ({ page, tasksServer }) => {
    await page.clock.setFixedTime(AGENTS_NOW);
    await openDashboard(page, tasksServer, 'agents');
    await page.locator('#activity-card').click();
    await expect(page.locator('#activity-panel')).toHaveScreenshot('light-agents-activity-panel.png');
  });

  test('progress details', async ({ page, tasksServer }) => {
    await page.clock.setFixedTime(AGENTS_NOW);
    await openDashboard(page, tasksServer, 'agents');
    await page.locator('#progress-card').click();
    await expect(page.locator('#progress-panel')).toHaveScreenshot('light-agents-progress-panel.png');
  });

  test('detail panel', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard', { tab: 'list' });
    await page.locator('.task-card[data-id="T-004"] .task-name').click();
    await expect(page.locator('#detail-panel')).toHaveScreenshot('light-standard-detail.png');
  });
});

// v4 (docs/v4/dashboard.md D-6.2): priorities, labels, stages, In Review and Blocked, notes and actors, the dialogs,
// in both modes. fixtures/v4.json at a fixed time, so the waiting times are stable.
const V4_NOW = new Date('2026-04-10T12:00:00Z');

for (const mode of ['dark', 'light'] as const) {
  test.describe(`visual: v4 (${mode})`, () => {
    test.use({ colorScheme: mode });
    test.beforeEach(async ({ page, tasksServer }) => {
      await page.clock.setFixedTime(V4_NOW);
      tasksServer.archive('archive');
    });

    test('kanban board: review, blocked, stages in Done', async ({ page, tasksServer }) => {
      await openDashboard(page, tasksServer.withAtmSummary(), 'v4', { tab: 'kanban' });
      await expect(page.locator('#kanban .kanban-card').first()).toBeVisible();
      await sectionScreenshot(page, `v4-${mode}-kanban.png`);
    });

    test('list: labels, priorities, stages', async ({ page, tasksServer }) => {
      await openDashboard(page, tasksServer, 'v4', { tab: 'list' });
      await sectionScreenshot(page, `v4-${mode}-list.png`);
    });

    test('detail panel: notes, actors, stage', async ({ page, tasksServer }) => {
      await openDashboard(page, tasksServer, 'v4', { tab: 'list' });
      await page.locator('.task-card[data-id="T-002"] .task-name').click();
      await expect(page.locator('#detail-panel')).toHaveScreenshot(`v4-${mode}-detail.png`);
    });

    test('detail panel: a blocked task and its questions', async ({ page, tasksServer }) => {
      await openDashboard(page, tasksServer, 'v4', { tab: 'list' });
      await page.locator('.task-card[data-id="T-005"] .task-name').click();
      await expect(page.locator('#detail-panel')).toHaveScreenshot(`v4-${mode}-detail-blocked.png`);
    });

    test('header and footer', async ({ page, tasksServer }) => {
      await openDashboard(page, tasksServer, 'v4');
      await expect(page.locator('header.topbar')).toHaveScreenshot(`v4-${mode}-header.png`, {
        mask: [page.locator('#version-chip')],
      });
      await expect(page.locator('#site-footer')).toHaveScreenshot(`v4-${mode}-footer.png`, {
        mask: [page.locator('#footer-version')],
      });
    });

    test('About and Help', async ({ page, tasksServer }) => {
      await openDashboard(page, tasksServer, 'v4');
      await page.locator('#footer-about').click();
      // The version changes with every release: masked
      await expect(page.locator('#about-panel')).toHaveScreenshot(`v4-${mode}-about.png`, {
        mask: [page.locator('#about-panel dd').nth(0)],
      });
      await page.keyboard.press('Escape');
      await page.locator('#footer-help').click();
      await expect(page.locator('#help-panel')).toHaveScreenshot(`v4-${mode}-help.png`);
    });
  });
}

// The ⋯ settings menu (T-323): auto-refresh, the mode switch, the theme combo box with the current theme's swatch,
// and resetting the project's view
for (const mode of ['dark', 'light'] as const) {
  test(`visual: settings menu (${mode})`, async ({ page, tasksServer }) => {
    await page.addInitScript(
      (m) => localStorage.setItem('atm-dashboard', JSON.stringify({ theme: 'gruvbox', mode: m, autoRefresh: false })),
      mode,
    );
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('#settings-btn').click();
    await expect(page.locator('#settings-menu')).toHaveScreenshot(`settings-menu-${mode}.png`, {
      mask: [page.locator('#last-loaded')],
    });
  });
}

// The Progress panel with skipped tasks: the thin outer arc on the overall ring, Phase 2's and Docs'
for (const mode of ['dark', 'light'] as const) {
  test(`visual: progress panel with skipped tasks (${mode})`, async ({ page, tasksServer }) => {
    await page.emulateMedia({ colorScheme: mode });
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('#progress-card').click();
    await expect(page.locator('#progress-panel')).toHaveScreenshot(`progress-panel-skipped-${mode}.png`);
  });
}

// The "new version is ready" toast in the bottom right corner, over the cards and clear of the title bar (T-185)
for (const mode of ['dark', 'light'] as const) {
  test(`visual: update notice (${mode})`, async ({ page, tasksServer }) => {
    await page.emulateMedia({ colorScheme: mode });
    await tasksServer.serverBuild('newer-build');
    await openDashboard(page, tasksServer.withAtmSummary(), 'standard');
    await expect(page.locator('#subtitle')).toContainText('loaded from tasks.json');
    await expect(page.locator('#update-notice')).toBeVisible();
    await expect(page).toHaveScreenshot(`update-notice-${mode}.png`, {
      mask: [page.locator('#version-chip'), page.locator('#footer-version')],
      stylePath: path.join(import.meta.dirname, 'screenshot.css'),
    });
  });
}

// The project selector, open, with progress, a "needs you" dot and a project that is gone
for (const mode of ['dark', 'light'] as const) {
  test(`visual: project selector (${mode})`, async ({ page, tasksServer }) => {
    await page.emulateMedia({ colorScheme: mode });
    const third = summary({ percent: 20, needs_you: { review: 1, blocked_on_user: 0 } });
    tasksServer
      .addProject('second', 'minimal', 'Second Project', summary({ percent: 75 }))
      .addProject('third', 'minimal', 'Third Project', third);
    await openDashboard(page, tasksServer, 'standard');
    await page.evaluate(() => (location.hash = '#project=second'));
    await expect(page.locator('#subtitle')).toContainText('loaded from tasks.json');
    await page.locator('#project-selector').click();
    await expect(page.locator('#project-menu')).toHaveScreenshot(`project-selector-${mode}.png`);
  });
}

// The "need you" chip, open on the list of the tasks that need you (fixtures/v4.json: one in review, one blocked)
for (const mode of ['dark', 'light'] as const) {
  test(`visual: needs-you list (${mode})`, async ({ page, tasksServer }) => {
    await page.emulateMedia({ colorScheme: mode });
    await openDashboard(page, tasksServer, 'v4', { tab: 'list' });
    await page.locator('#attention-count').click();
    await expect(page.locator('#needs-you-menu')).toHaveScreenshot(`needs-you-menu-${mode}.png`);
  });
}

// The overview of all projects: cards with sprints or without, agents, activity, a "needs you" and a gone project
for (const mode of ['dark', 'light'] as const) {
  test(`visual: projects overview (${mode})`, async ({ page, tasksServer }) => {
    await page.emulateMedia({ colorScheme: mode });
    await page.clock.setFixedTime(new Date('2026-01-01T12:00:00Z'));
    tasksServer
      .addProject(
        'busy',
        'minimal',
        'Busy Project',
        summary({
          total: 20,
          done: 9,
          skipped: 2,
          percent: 50,
          recent_sprints: [
            sprintNumbers('Setup', 5, 5, 'closed'),
            sprintNumbers('Core', 3, 7, 'closed'),
            sprintNumbers('Polish', 1, 4, 'active'),
            sprintNumbers('Ada Lovelace', 0, 2),
          ],
          active_sprint: { ...sprintNumbers('Polish', 1, 4, 'active'), due: '2026-01-04', goal: 'Card polish' },
          agents_working: 2,
          working: [
            { task: 'T-14', name: 'Kanban card polish', agent: 'claude:e5ea9db0', branch: 'dev/polish' },
            {
              task: 'T-15',
              name: 'Theme picker with a much longer name that ends in …',
              agent: 'alice',
              branch: 'main',
            },
          ],
          last_activity: '2026-01-01T11:48:00Z',
          last_done: { id: 'T-13', name: 'Status bar pinned to the bottom', completed_at: '2026-01-01T11:48:00Z' },
          stats: { started: '2025-11-12T09:00:00Z', tasks: 20, agent_minutes: 4440 },
          git: { branch: 'dev/polish', pr: { number: 12, url: 'https://github.com/acme/busy/pull/12', state: 'open' } },
        }),
      )
      .addProject(
        'asks',
        'minimal',
        'Waiting Project',
        summary({
          needs_you: { review: 1, blocked_on_user: 1 },
          next: [{ id: 'T-3', name: 'Sign-in page' }],
        }),
      )
      .goneProject('old1234', 'Old Project')
      .logo(
        'busy',
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="7" fill="#2f81f7"/>' +
          '<path d="m4.5 8.2 2.3 2.3 4.7-5" fill="none" stroke="#fff" stroke-width="2"/></svg>',
      );
    await openDashboard(page, tasksServer, 'standard');
    await expect(page.locator('.project-card')).toHaveCount(4);
    await expect(page.locator('.project-card[data-id="busy"] .project-logo img')).toHaveJSProperty('complete', true);
    // The whole page: the title bar has to look exactly like the project page's (controls at the right)
    await expect(page).toHaveScreenshot(`projects-overview-${mode}.png`, {
      mask: [page.locator('#version-chip'), page.locator('#footer-version')],
      stylePath: path.join(import.meta.dirname, 'screenshot.css'),
    });
    // The narrowest cards (two columns of about 280 px): every section still fits and the cards line up (T-209)
    await page.setViewportSize({ width: 640, height: 900 });
    await expect(page.locator('#landing')).toHaveScreenshot(`projects-overview-narrow-${mode}.png`, {
      mask: [page.locator('#version-chip')],
    });
  });
}

// The overview's Agents tab (T-333): two projects' agents on one timeline, each lane naming its projects
test('visual: projects overview agents', async ({ page, tasksServer }) => {
  await page.clock.setFixedTime(new Date('2026-04-02T12:00:00Z'));
  tasksServer.use('agents');
  tasksServer.addProject('second', 'agents', 'Second Project');
  await page.goto('tasks-dashboard.html#overview&view=agents');
  await expect(page.locator('#overview-agents .agent-info').first()).toBeVisible();
  await expect(page.locator('#overview-agents')).toHaveScreenshot('overview-agents.png');
});

// The All projects board (T-259): cards from two projects with their chips, each column's line, an empty column and
// "+ N more"
for (const mode of ['dark', 'light'] as const) {
  test(`visual: all projects board (${mode})`, async ({ page, tasksServer }) => {
    await page.emulateMedia({ colorScheme: mode });
    await page.clock.setFixedTime(new Date('2026-03-01T12:00:00Z'));
    tasksServer
      .addProject(
        'shop',
        'minimal',
        'Shop',
        summary({
          last_activity: '2026-03-01T11:00:00Z',
          board: board(
            {
              todo: [boardCard('T-008', 'Password reset', { labels: ['api'] }), boardCard('T-009', 'Email templates')],
              active: [boardCard('T-007', 'Checkout flow', { by: 'claude:e5ea9db0', since: '2026-03-01T10:00:00Z' })],
              blocked: [
                boardCard('T-005', 'Choose the email provider', {
                  since: '2026-03-01T06:00:00Z',
                  waiting_on: 'agent',
                  questions: 1,
                }),
              ],
              done: [boardCard('T-003', 'Product page', { type: 'bugfix', completed_at: '2026-03-01T11:30:00Z' })],
            },
            { todo: 12 },
          ),
        }),
      )
      .addProject(
        'blog',
        'minimal',
        'Blog',
        summary({
          last_activity: '2026-03-01T11:50:00Z',
          board: board({
            todo: [boardCard('B-1', 'RSS feed', { priority: 'P1', phase: 3, labels: ['ux', 'api', 'seo'] })],
            blocked: [
              boardCard('B-2', 'Pick a theme', { since: '2026-03-01T11:00:00Z', waiting_on: 'user', questions: 2 }),
            ],
            done: [boardCard('B-4', 'Drafts', { type: 'docs', completed_at: '2026-03-01T11:55:00Z' })],
          }),
        }),
      );
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('#overview-tab-board').click();
    await expect(page.locator('#projects-overview')).toHaveScreenshot(`all-projects-board-${mode}.png`);
    // Grouped by project (T-260): a lane each, no chips on the cards
    await page.locator('#board-group-by-project').check();
    await expect(page.locator('#projects-overview')).toHaveScreenshot(`all-projects-board-grouped-${mode}.png`);
  });
}

// FR-3: per theme and mode, a compact sample to catch regressions in any palette: a Kanban column, with its cards,
// chips and badges (T-345: not the whole view, which a layout change would touch in every theme)
for (const theme of [
  'nord',
  'solarized',
  'catppuccin',
  'tokyo-night',
  'ayu',
  'night-owl',
  'gruvbox',
  'noctis',
  'sublime-material',
  'high-contrast',
  'sticky-notes',
  'hand-drawn',
]) {
  for (const mode of ['dark', 'light']) {
    test(`visual: theme ${theme} (${mode})`, async ({ page, tasksServer }) => {
      await page.addInitScript(
        ([t, m]) => localStorage.setItem('atm-dashboard', JSON.stringify({ theme: t, mode: m, autoRefresh: false })),
        [theme, mode],
      );
      await openDashboard(page, tasksServer.withAtmSummary(), 'kanban');
      await page.locator('#tab-kanban').click();
      await expect(page.locator('#kanban .kanban-card').first()).toBeVisible();
      await expect(page.locator('#kanban .kanban-column').first()).toHaveScreenshot(`theme-${theme}-${mode}.png`);
    });
  }
}

// The scope bar, its menu and the release view (T-323, SR-7.2, SR-7.5), in fixtures/sprints.json: "Ada Lovelace" is
// active, due 2026-04-12, so two days overdue here
for (const colorScheme of ['dark', 'light'] as const) {
  test.describe(colorScheme, () => {
    test.use({ colorScheme });

    test(`visual: scope bar (${colorScheme})`, async ({ page, tasksServer }) => {
      await page.clock.setFixedTime(new Date('2026-04-14T12:00:00Z'));
      await openDashboard(page, tasksServer, 'sprints');
      await expect(page.locator('#scope-bar')).toHaveScreenshot(`scope-bar-${colorScheme}.png`);
    });

    test(`visual: list grouped by sprint (${colorScheme})`, async ({ page, tasksServer }) => {
      await page.clock.setFixedTime(new Date('2026-04-14T12:00:00Z'));
      await openDashboard(page, tasksServer, 'sprints', { tab: 'list' });
      await page.locator('#scope-trigger').click();
      await page.locator('.scope-choice[data-scope="all"]').click();
      await page.locator('#list-closed-toggle').click();
      await expect(page.locator('#panel-list')).toHaveScreenshot(`list-by-sprint-${colorScheme}.png`);
    });

    test(`visual: velocity (${colorScheme})`, async ({ page, tasksServer }) => {
      await page.clock.setFixedTime(new Date('2026-04-14T12:00:00Z'));
      const data = loadFixture('sprints');
      const meta = data.meta as { sprints: object[] };
      const closed = (id: string, done: number) => ({
        id,
        state: 'closed',
        report: { done: Array.from({ length: done }, (_, i) => `${id}-${i}`), carried: done % 2 ? ['x'] : [] },
      });
      meta.sprints = [closed('Alpha', 6), closed('Beta', 9), closed('Gamma', 4), closed('Delta', 11), ...meta.sprints];
      await openDashboard(page, tasksServer, data, { tab: 'progress' });
      await expect(page.locator('#velocity')).toHaveScreenshot(`velocity-${colorScheme}.png`);
    });

    test(`visual: scope menu (${colorScheme})`, async ({ page, tasksServer }) => {
      await page.clock.setFixedTime(new Date('2026-04-14T12:00:00Z'));
      await openDashboard(page, tasksServer, 'sprints');
      await page.locator('#scope-trigger').click();
      await expect(page.locator('#scope-menu')).toHaveScreenshot(`scope-menu-${colorScheme}.png`);
    });

    test(`visual: release view (${colorScheme})`, async ({ page, tasksServer }) => {
      await page.clock.setFixedTime(new Date('2026-04-14T12:00:00Z'));
      // Tall enough for the whole tab: scrolled down, the view would sit under the fixed status bar
      await page.setViewportSize({ width: 1280, height: 1800 });
      await openDashboard(page, tasksServer, 'sprints', { tab: 'progress' });
      await page.locator('.release-row[data-release="1.0"] summary').click();
      await expect(page.locator('#release-view')).toHaveScreenshot(`release-view-${colorScheme}.png`);
    });
  });
}
