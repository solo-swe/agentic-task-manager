// Several projects (docs/multi-project-dashboard-proposal.md 3.2, 3.5): the landing page, #project= links, switching
// between projects with their own saved views, and a project the server doesn't list.
import { expect, openDashboard, sprintNumbers, summary, test } from './helpers';

test.describe('projects', () => {
  test('with several projects, the dashboard lands on the overview; a project opens from it', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.addProject('second', 'minimal', 'Second Project');
    await openDashboard(page, tasksServer, 'standard');
    await expect(page.locator('#landing')).toHaveAttribute('data-page', 'overview');
    await expect(page).toHaveTitle('Projects Overview');
    await expect(page.locator('#projects-grid .project-card')).toHaveCount(2);
    await page.locator('.project-card[data-id="second"] .project-card-open').click();
    await expect(page).toHaveURL(/#project=second$/);
    await expect(page.locator('#dashboard-title')).toHaveText('Minimal Project');
    await page.goBack();
    await expect(page.locator('#landing')).toHaveAttribute('data-page', 'overview');
  });

  test("About names the open project, and has no project section on the overview (it was the last project's)", async ({
    page,
    tasksServer,
  }) => {
    tasksServer.addProject('second', 'minimal', 'Second Project');
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('.project-card[data-id="second"] .project-card-open').click();
    await page.locator('#footer-about').click();
    await expect(page.locator('#about-project')).toHaveText('Minimal Project');
    await expect(page.locator('#about-project .project-logo')).toHaveCount(1);
    await expect(page.locator('#about-vocab')).toBeVisible();
    await page.keyboard.press('Escape');

    // To All projects in the same page, so the last project's data is still loaded
    await page.locator('#project-selector').click();
    await page.locator('#project-option-overview').click();
    await expect(page.locator('#landing')).toHaveAttribute('data-page', 'overview');
    await page.locator('#footer-about').click();
    await expect(page.locator('#about-panel')).toContainText('Version');
    await expect(page.locator('#about-project')).toHaveCount(0);
    await expect(page.locator('#about-vocab')).toHaveCount(0);
  });

  test('the title bar is the same on the overview and on a project: controls at the same place', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.addProject('second', 'minimal', 'Second Project');
    await openDashboard(page, tasksServer, 'standard');
    await expect(page.locator('#landing')).toHaveAttribute('data-page', 'overview');
    await expect(page.locator('#version-chip')).toBeVisible();
    const onOverview = (await page.locator('.topbar-controls').boundingBox())!;
    await page.locator('.project-card[data-id="second"] .project-card-open').click();
    await expect(page.locator('#dashboard-title')).toHaveText('Minimal Project');
    const onProject = (await page.locator('.topbar-controls').boundingBox())!;
    expect(onOverview.x + onOverview.width).toBeCloseTo(onProject.x + onProject.width, 0);
    expect(onOverview.y).toBeCloseTo(onProject.y, 0);
  });

  test('the status bar sits at the bottom of the window on every page, however short', async ({
    page,
    tasksServer,
  }) => {
    const footerAtBottom = async () => {
      const box = (await page.locator('#site-footer').boundingBox())!;
      const height = page.viewportSize()!.height;
      expect(box.y + box.height).toBeCloseTo(height, 0);
    };
    // The project page
    await openDashboard(page, tasksServer, 'minimal');
    await expect(page.locator('#dashboard-title')).toHaveText('Minimal Project');
    await footerAtBottom();
    // The overview, with one project card
    await page.goto('tasks-dashboard.html#overview');
    await expect(page.locator('#landing')).toHaveAttribute('data-page', 'overview');
    await expect(page.locator('#projects-grid .project-card')).toHaveCount(1);
    await footerAtBottom();
    // No projects yet
    tasksServer.noProjects();
    await page.goto('tasks-dashboard.html#');
    await page.reload();
    await expect(page.locator('#no-projects')).toBeVisible();
    await footerAtBottom();
  });

  test('the overview’s tabs sit right under the title bar, as on a project’s page (T-334)', async ({
    page,
    tasksServer,
  }) => {
    const tabsTop = async () => {
      const main = (await page.locator('#app-main').boundingBox())!;
      const tab = (await page.locator('.view-tab[aria-selected="true"]').first().boundingBox())!;
      return tab.y - main.y;
    };
    await openDashboard(page, tasksServer, 'minimal');
    await expect(page.locator('#dashboard-title')).toHaveText('Minimal Project');
    const onProject = await tabsTop();
    await page.goto('tasks-dashboard.html#overview');
    await expect(page.locator('#projects-grid .project-card')).toHaveCount(1);
    expect(await tabsTop()).toBe(onProject);
  });

  test('with one project, the dashboard opens it right away', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    await expect(page.locator('#landing')).toHaveCount(0);
  });

  test('#project= links to a project; switching keeps each project’s own view', async ({ page, tasksServer }) => {
    tasksServer.addProject('second', 'minimal', 'Second Project');
    tasksServer.use('standard');
    await page.goto('tasks-dashboard.html#project=second');
    await expect(page.locator('#dashboard-title')).toHaveText('Minimal Project');
    await page.locator('#task-search').fill('nothing matches this');
    await expect(page.locator('.task-card')).toHaveCount(0);

    await page.evaluate((id) => (location.hash = `#project=${id}`), tasksServer.projectId);
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    await expect(page.locator('#task-search')).toHaveValue('');

    await page.evaluate(() => (location.hash = '#project=second'));
    await expect(page.locator('#dashboard-title')).toHaveText('Minimal Project');
    await expect(page.locator('#task-search')).toHaveValue('nothing matches this');
  });

  test('the selector lists Projects Overview and the projects, with progress; choosing one opens it', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.addProject('second', 'minimal', 'Second Project', summary({ percent: 75 }));
    tasksServer.use('standard');
    await page.goto(`tasks-dashboard.html#project=second`);
    await page.locator('#project-selector').click();
    await expect(page.locator('#project-filter')).toBeFocused();
    await expect(page.locator('#project-options [role=option]')).toHaveText([
      'Projects Overview',
      'Standard Project',
      'Second Project',
    ]);
    await expect(page.locator('[data-id="second"]')).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('[data-id="second"] .project-option-fill')).toHaveAttribute('style', /width: 75%/);
    await page.locator(`.project-option[data-id="${tasksServer.projectId}"]`).click();
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    await expect(page.locator('#project-menu')).toHaveCount(0);
  });

  test('the selector by keyboard: type to filter, arrows, Enter; Escape closes', async ({ page, tasksServer }) => {
    tasksServer.addProject('second', 'minimal', 'Second Project').addProject('third', 'minimal', 'Third Project');
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('.project-card[data-id="second"] .project-card-open').click();
    await page.locator('#project-selector').click();
    await page.keyboard.type('thi');
    await expect(page.locator('#project-options [role=option]')).toHaveText(['Third Project']);
    await page.keyboard.press('Enter');
    await expect(page).toHaveURL(/#project=third(&sprint=[^&]+)?$/);

    await page.locator('#project-selector').click();
    await page.keyboard.press('Home');
    await expect(page.locator('#project-filter')).toHaveAttribute('aria-activedescendant', 'project-option-overview');
    await page.keyboard.press('Escape');
    await expect(page.locator('#project-menu')).toHaveCount(0);
    await expect(page.locator('#project-selector')).toBeFocused();
    await page.keyboard.press('Enter');
    await page.keyboard.press('Home');
    await page.keyboard.press('Enter');
    await expect(page.locator('#landing')).toHaveAttribute('data-page', 'overview');
  });

  test('a badge on the selector counts what needs you in the other projects', async ({ page, tasksServer }) => {
    tasksServer.addProject('second', 'minimal', 'Second', summary({ needs_you: { review: 2, blocked_on_user: 1 } }));
    tasksServer.addProject('third', 'minimal', 'Third', summary());
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('.project-card[data-id="third"] .project-card-open').click();
    await expect(page.locator('#other-projects-need')).toHaveText('3');
    await page.locator('#project-selector').click();
    await expect(page.locator('[data-id="second"] .project-option-dot')).toHaveCount(1);
    await expect(page.locator('[data-id="third"] .project-option-dot')).toHaveCount(0);
    await page.locator('.project-option[data-id="second"]').click();
    await expect(page.locator('#other-projects-need')).toHaveCount(0);
  });

  test('overview cards: progress, agents, last activity; needs you first', async ({ page, tasksServer }) => {
    await page.clock.setFixedTime(new Date('2026-01-01T12:00:00Z'));
    tasksServer
      .addProject(
        'busy',
        'minimal',
        'Busy',
        summary({
          total: 20,
          done: 9,
          skipped: 2,
          percent: 50,
          agents_working: 2,
          // A server before 0.18 sends no working list (undefined isn't serialized): the card counts the agents
          working: undefined,
          last_activity: '2026-01-01T11:48:00Z',
        }),
      )
      .addProject('asks', 'minimal', 'Asks', summary({ needs_you: { review: 1, blocked_on_user: 1 } }));
    await openDashboard(page, tasksServer, 'standard');
    const cards = page.locator('.project-card');
    await expect(cards.locator('.project-card-name')).toHaveText(['Asks', 'Busy', 'Standard Project']);
    const busy = page.locator('.project-card[data-id="busy"]');
    // No sprints: one ring for all its tasks
    await expect(busy.locator('.project-card-ring svg')).toHaveAttribute('aria-label', 'All tasks: 9 of 18 done');
    await expect(busy.locator('.project-card-progress')).toHaveAttribute('title', '9 / 18 done · 2 skipped (50%)');
    await expect(busy.locator('.project-card-phase')).toHaveText('No sprints yet');
    await expect(busy.locator('.project-card-agents')).toHaveText('2 agents working');
    await expect(busy.locator('.project-card-last')).toHaveText('12 m ago');
    await expect(page.locator('.project-card[data-id="asks"] .project-card-need')).toHaveText('2 need you');
  });

  // T-193: up to 4 rings, one per recent sprint, never wrapping, at the narrowest card (the grid's 280px minimum, two
  // columns) and on a phone (one column, narrower still)
  for (const width of [620, 340]) {
    test(`overview cards: the recent sprints' rings sit on one row (${width}px window)`, async ({
      page,
      tasksServer,
    }) => {
      await page.setViewportSize({ width, height: 800 });
      const recent_sprints = [14, 15, 16, 17].map((n) => sprintNumbers(`Sprint name ${n}`, 3, 4));
      tasksServer.addProject('busy', 'minimal', 'Busy', summary({ recent_sprints }));
      await openDashboard(page, tasksServer, 'standard');
      const card = page.locator('.project-card[data-id="busy"]');
      await expect(card.locator('.project-card-ring')).toHaveCount(4);
      await expect(card.locator('.project-card-ring svg').first()).toHaveAttribute(
        'aria-label',
        'Sprint name 14: 3 of 4 done',
      );
      const layout = await card.evaluate((el) => {
        const row = el.querySelector('.project-card-rings')!;
        const boxes = [...row.querySelectorAll('.project-card-ring')].map((r) => r.getBoundingClientRect());
        const inner = el.getBoundingClientRect();
        return {
          cardWidth: Math.round(inner.width),
          oneRow: new Set(boxes.map((b) => Math.round(b.top))).size === 1,
          inside: boxes.every((b) => b.left >= inner.left && b.right <= inner.right),
          ringsVisible: boxes.every((b) => b.width >= 32),
        };
      });
      expect(layout.cardWidth).toBeLessThanOrEqual(width === 620 ? 280 : 292);
      expect(layout).toMatchObject({ oneRow: true, inside: true, ringsVisible: true });
    });
  }

  // T-190: a long sprint name is cut off on one line instead of wrapping and pushing the foot down
  test('overview cards: a long sprint name stays on one line; the feet of a row line up', async ({
    page,
    tasksServer,
  }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    const name = 'A very long sprint name that would never fit on one line of a project card, not even close';
    const older = { working: undefined, next: undefined, last_done: undefined, git: undefined };
    const active = { ...sprintNumbers(name, 2, 9, 'active'), goal: null };
    tasksServer
      .addProject('long', 'minimal', 'Long', summary({ ...older, recent_sprints: [active], active_sprint: active }))
      .addProject('short', 'minimal', 'Short', summary({ ...older, agents_working: 1 }));
    await openDashboard(page, tasksServer, 'standard');
    const line = page.locator('.project-card[data-id="long"] .project-card-phase');
    await expect(line.locator('.project-card-phase-name')).toHaveText(name);
    await expect(line.locator('.project-card-phase-count')).toBeVisible();
    const geometry = await page.evaluate(() => {
      const phase = document.querySelector('.project-card[data-id="long"] .project-card-phase')!;
      const count = phase.querySelector('.project-card-phase-count')!.getBoundingClientRect();
      const feet = [...document.querySelectorAll('.project-card')]
        .map((c) => c.querySelector('.project-card-foot')?.getBoundingClientRect())
        .filter((b) => b !== undefined);
      return {
        oneLine: phase.getBoundingClientRect().height < 24,
        countInside: count.right <= phase.getBoundingClientRect().right + 0.5,
        feetLevel: new Set(feet.map((b) => Math.round(b.top))).size === 1,
        cards: feet.length,
      };
    });
    expect(geometry).toEqual({ oneLine: true, countInside: true, feetLevel: true, cards: 2 });
  });

  // T-209: the approved order, and every section at the same height on the cards of a row, whatever they hold
  test('overview cards: the sections in the approved order, level across a row', async ({ page, tasksServer }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    const claim = { task: 'T-1', name: 'Build', agent: 'claude:e5ea9db0', branch: 'dev/x' };
    tasksServer
      .addProject(
        'full',
        'minimal',
        'Full',
        summary({
          recent_sprints: [sprintNumbers('One', 1, 2)],
          working: [claim],
          git: { branch: 'dev/x', pr: { number: 4, url: null, state: 'open' } },
          last_done: { id: 'T-0', name: 'Setup', completed_at: '2026-01-01T00:00:00Z' },
          description: 'What this project is for',
        }),
      )
      .addProject('empty', 'minimal', 'Empty', summary());
    await openDashboard(page, tasksServer, 'standard');
    const sections = [
      '.project-card-head',
      '.project-card-progress',
      '.project-card-work',
      '.project-card-git',
      '.project-card-foot',
    ];
    const layout = await page.evaluate((selectors) => {
      const tops = (id: string) => {
        const card = document.querySelector(`.project-card[data-id="${id}"]`)!;
        return selectors.map((s) => Math.round(card.querySelector(s)!.getBoundingClientRect().top));
      };
      return { full: tops('full'), empty: tops('empty') };
    }, sections);
    // Top to bottom in that order, and level with the other card's
    expect(layout.full).toEqual([...layout.full].sort((a, b) => a - b));
    expect(layout.empty).toEqual(layout.full);
    // The whole count lives in the rings' tooltip; the description on the name
    await expect(page.locator('.project-card[data-id="full"] .project-card-counts')).toHaveCount(0);
    await expect(page.locator('.project-card[data-id="full"] .project-card-open')).toHaveAttribute(
      'title',
      /^Full\nWhat this project is for\n/,
    );
  });

  // T-194 review: two lines for every card's name, so a long one doesn't push its card's rings below the others'
  test('overview cards: a long project name takes at most two lines; the rings of a row line up', async ({
    page,
    tasksServer,
  }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    const recent_sprints = [sprintNumbers('One', 1, 2)];
    const long = 'A project with a very long name that would take three or more lines on a card of this width';
    tasksServer
      .addProject('long', 'minimal', long, summary({ recent_sprints }))
      .addProject('short', 'minimal', 'Short', summary({ recent_sprints }));
    await openDashboard(page, tasksServer, 'standard');
    const name = page.locator('.project-card[data-id="long"] .project-card-name');
    await expect(name.locator('.project-card-open')).toHaveAttribute('title', new RegExp(`^${long}\\n`));
    const geometry = await page.evaluate(() => {
      const card = (id: string) => document.querySelector(`.project-card[data-id="${id}"]`)!;
      const lineHeight = parseFloat(getComputedStyle(card('long').querySelector('.project-card-name')!).lineHeight);
      const nameHeight = (id: string) => card(id).querySelector('.project-card-name')!.getBoundingClientRect().height;
      const ringsTop = (id: string) => card(id).querySelector('.project-card-rings')!.getBoundingClientRect().top;
      return {
        twoLines: Math.round(nameHeight('long') / lineHeight) <= 2 && nameHeight('long') <= 2 * lineHeight + 4,
        sameNameHeight: Math.round(nameHeight('long')) === Math.round(nameHeight('short')),
        ringsLevel: Math.round(ringsTop('long')) === Math.round(ringsTop('short')),
      };
    });
    expect(geometry).toEqual({ twoLines: true, sameNameHeight: true, ringsLevel: true });
  });

  // T-207: who works on what, by name, or what comes next; two lines on every card
  test('overview cards: the tasks at work or next, by name, in a section of the same height on every card', async ({
    page,
    tasksServer,
  }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    const claim = (task: string, name: string) => ({ task, name, agent: 'claude:e5ea9db0', branch: 'dev/x' });
    tasksServer
      .addProject(
        'busy',
        'minimal',
        'Busy',
        summary({
          agents_working: 3,
          working: [claim('T-1', 'Build the board'), claim('T-2', 'Write the tests'), claim('T-3', 'Polish')],
        }),
      )
      .addProject('idle', 'minimal', 'Idle', summary({ next: [{ id: 'T-9', name: 'Next thing' }] }))
      .addProject('done', 'minimal', 'Done', summary());
    await openDashboard(page, tasksServer, 'standard');
    const work = (id: string) => page.locator(`.project-card[data-id="${id}"] .project-card-work-line`);
    await expect(work('busy')).toHaveText([/T-1\s*Build the board/, /T-2\s*Write the tests\s*\+1 more/]);
    await expect(work('idle')).toHaveText([/Next:\s*T-9\s*Next thing/]);
    await expect(work('done')).toHaveCount(0);
    const tops = await page.evaluate(() =>
      ['busy', 'idle', 'done'].map((id) => {
        const card = document.querySelector(`.project-card[data-id="${id}"]`)!;
        const box = card.querySelector('.project-card-work')!.getBoundingClientRect();
        return [Math.round(box.top), Math.round(box.height)];
      }),
    );
    expect(new Set(tops.map((t) => t.join(','))).size).toBe(1);
  });

  // T-197: each project's logo (p/<id>/logo), or its monogram, in the cards, the selector and the title bar
  test('project logos: the image when there is one, else the monogram, also when the image fails', async ({
    page,
    tasksServer,
  }) => {
    const svg =
      '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"><rect width="8" height="8" fill="red"/></svg>';
    tasksServer
      .addProject('withlogo', 'minimal', 'With Logo', summary())
      .addProject('broken', 'minimal', 'Broken Logo', summary())
      .logo('withlogo', svg)
      .logo('broken', null);
    await openDashboard(page, tasksServer, 'standard');
    const logo = (id: string) => page.locator(`.project-card[data-id="${id}"] .project-logo`);
    await expect(logo('withlogo').locator('img')).toHaveJSProperty('complete', true);
    expect(
      await logo('withlogo')
        .locator('img')
        .evaluate((img: HTMLImageElement) => img.naturalWidth),
    ).toBeGreaterThan(0);
    await expect(logo('broken')).toHaveAttribute('data-initials', 'BL');
    await expect(logo('broken').locator('img')).toHaveCount(0);
    await expect(logo(tasksServer.projectId)).toHaveAttribute('data-initials', 'SP');
    for (const id of ['withlogo', 'broken', tasksServer.projectId]) {
      const box = await logo(id).boundingBox();
      expect([id, Math.round(box!.width), Math.round(box!.height)]).toEqual([id, 24, 24]);
    }

    await page.locator('#project-selector').click();
    await expect(page.locator('.project-option[data-id="withlogo"] .project-logo img')).toHaveCount(1);
    await expect(page.locator('.project-option[data-id="broken"] .project-logo')).toHaveAttribute(
      'data-initials',
      'BL',
    );
    await page.locator('.project-option[data-id="withlogo"]').click();
    await expect(page.locator('.title-row .project-logo img')).toHaveAttribute('src', 'p/withlogo/logo');
  });

  test('without the task list (an older server), “needs you” opens the Kanban filtered to review and blocked', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.addProject('asks', 'v4', 'Asks', summary({ needs_you: { review: 1, blocked_on_user: 1 } }));
    await openDashboard(page, tasksServer, 'standard');
    await page.locator('.project-card[data-id="asks"] .project-card-need').click();
    await expect(page).toHaveURL(/project=asks/);
    await expect(page.locator('#tab-kanban')).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('.filter-chip')).toHaveText(['Status: In Review, Blocked']);
  });

  /** The detail panel is open on task `id`. */
  const expectDetail = (page: import('@playwright/test').Page, id: string) =>
    expect(page.locator('#detail-overlay.open #detail-panel h3 > span').first()).toHaveText(id);
  const T4 = { id: 'T-004', name: 'Account settings page', reason: 'review' };
  const T5 = { id: 'T-005', name: 'Choose the email provider', reason: 'blocked' };

  test('“needs you” with one task opens the project on that task', async ({ page, tasksServer }) => {
    const one = summary({ needs_you: { review: 1, blocked_on_user: 0 }, needs_you_tasks: [T4] });
    tasksServer.addProject('asks', 'v4', 'Asks', one);
    await openDashboard(page, tasksServer, 'standard');
    const chip = page.locator('.project-card[data-id="asks"] .project-card-need');
    await expect(chip).toHaveText('1 needs you');
    await chip.click();
    await expect(page).toHaveURL(/project=asks/);
    await expectDetail(page, 'T-004');
  });

  test('“needs you” with several lists them on the card; a row opens the project on it', async ({
    page,
    tasksServer,
  }) => {
    const two = summary({ needs_you: { review: 1, blocked_on_user: 1 }, needs_you_tasks: [T4, T5] });
    tasksServer.addProject('asks', 'v4', 'Asks', two);
    await openDashboard(page, tasksServer, 'standard');
    const card = page.locator('.project-card[data-id="asks"]');
    await card.locator('.project-card-need').click();
    const rows = card.locator('.needs-you-menu .needs-you-row');
    await expect(rows.locator('.needs-you-reason')).toHaveText(['in review', 'blocked on a question']);
    await expect(rows.first()).toBeFocused();
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Enter');
    await expect(page).toHaveURL(/project=asks/);
    await expectDetail(page, 'T-005');
  });

  // T-332: on a card in the first column, the list (lined up with the chip's right edge) ran off the window's left
  for (const width of [1280, 700, 420]) {
    test(`the “needs you” list stays inside the window, on the first card and the last (${width}px window)`, async ({
      page,
      tasksServer,
    }) => {
      await page.setViewportSize({ width, height: 900 });
      const two = summary({ needs_you: { review: 1, blocked_on_user: 1 }, needs_you_tasks: [T4, T5] });
      tasksServer.addProject('asks', 'v4', 'Asks', two).addProject('more', 'v4', 'More asks', two);
      await openDashboard(page, tasksServer, 'standard');
      for (const id of ['asks', 'more']) {
        const card = page.locator(`.project-card[data-id="${id}"]`);
        await card.locator('.project-card-need').click();
        const menu = card.locator('.needs-you-menu');
        await expect(menu).toBeVisible();
        await expect
          .poll(async () => {
            const box = (await menu.boundingBox())!;
            return box.x >= 0 && box.x + box.width <= width;
          })
          .toBe(true);
        await page.keyboard.press('Escape');
        await expect(menu).toBeHidden();
      }
    });
  }

  test('Active only hides projects with nothing open and nothing done lately; remembered', async ({
    page,
    tasksServer,
  }) => {
    await page.clock.setFixedTime(new Date('2026-03-01T12:00:00Z'));
    const idle = summary({
      total: 5,
      done: 5,
      percent: 100,
      last_activity: '2026-01-01T00:00:00Z',
    });
    tasksServer.addProject('idle', 'minimal', 'Idle', idle).addProject('open', 'minimal', 'Open', summary());
    await openDashboard(page, tasksServer, 'standard');
    await expect(page.locator('#active-projects-only')).toBeChecked();
    await expect(page.locator('.project-card[data-id="idle"]')).toHaveCount(0);
    await expect(page.locator('#projects-hidden')).toHaveText('1 inactive project hidden');
    await page.locator('#active-projects-only').uncheck();
    await expect(page.locator('.project-card[data-id="idle"]')).toHaveCount(1);
    await page.reload();
    await expect(page.locator('#active-projects-only')).not.toBeChecked();
  });

  test('overview cards: one line of stats; About links to the Project tab (T-255)', async ({ page, tasksServer }) => {
    await page.clock.setFixedTime(new Date('2026-10-26T12:00:00Z'));
    const stats = { started: '2026-08-12T12:00:00Z', tasks: 236, agent_minutes: 4440 };
    tasksServer
      .addProject('busy', 'minimal', 'Busy', summary({ stats }))
      .addProject('old', 'minimal', 'Old', summary());
    await openDashboard(page, tasksServer, 'standard');
    await expect(page.locator('.project-card[data-id="busy"] .project-card-stats')).toHaveText(
      // The date in the browser's locale
      /^started (12 Aug|Aug 12) · 236 tasks · 74 h agent time$/,
    );
    // An older server sends no stats: no line
    await expect(page.locator('.project-card[data-id="old"] .project-card-stats')).toHaveCount(0);

    await page.locator('.project-card[data-id="busy"] .project-card-open').click();
    await page.locator('#footer-about').click();
    await page.locator('#about-project-details').click();
    await expect(page.locator('#about-overlay.open')).toHaveCount(0);
    await expect(page.locator('#tab-project')).toHaveAttribute('aria-selected', 'true');
    await expect(page).toHaveURL(/view=project/);
  });

  test('a project whose folder is gone says how to remove it', async ({ page, tasksServer }) => {
    tasksServer.addProject('open', 'minimal', 'Open', summary()).goneProject('old1234', 'Old Project');
    await openDashboard(page, tasksServer, 'standard');
    await expect(page.locator('.project-card[data-id="old1234"]')).toContainText('atm projects remove old1234');
  });

  test('a project the server doesn’t list has its own page, with a way back to all projects', async ({
    page,
    tasksServer,
  }) => {
    tasksServer.use('standard');
    await page.goto('tasks-dashboard.html#project=gone123');
    await expect(page.locator('#unknown-project')).toContainText('No project “gone123”');
    await page.locator('#unknown-project a').click();
    await expect(page.locator('#landing')).toHaveAttribute('data-page', 'overview');
  });
});
