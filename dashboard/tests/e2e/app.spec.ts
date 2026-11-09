// The dashboard in the atm app (T-457): the same page, its data from the app's commands over Tauri's IPC (mocked here
// as `@tauri-apps/api/mocks` does, backed by the fixture's data), never from HTTP.
import { addressOf, expect, test } from './helpers';

test.describe('the dashboard in the atm app', () => {
  test('loads its projects and their tasks through the app’s commands, with no request to a server', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    await page.goto('/');
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    await expect(page.locator('#kanban .kanban-card').first()).toBeVisible();
    expect(tasksServer.httpRequests).toEqual([]);
  });

  test('Connect your agents (T-458): opening it only reads the status; a click connects, saying what it wrote where', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    await page.goto('/');
    await page.locator('#footer-agents').click();
    const harness = page.locator('.agents-harness[data-harness="claude"]');
    await expect(harness.locator('.agents-harness-state')).toHaveText('Not connected');
    expect(tasksServer.agentCalls).toEqual(['agents_status']);

    await harness.locator('.agents-connect').click();
    await expect(page.locator('#agents-result li')).toHaveText([
      '/Users/me/.claude/skills/atm-do-task (installed, linked)',
      '/Users/me/.claude/skills/atm-serve (installed, linked)',
    ]);
    await expect(harness.locator('.agents-harness-state')).toHaveText('Connected');
    await expect(harness.locator('.agents-connect')).toHaveText('Connect Claude Code again');
    expect(tasksServer.agentCalls).toEqual(['agents_status', 'connect_agents claude', 'agents_status']);
  });

  test('deep links (T-459): the one the app was started with opens its page; a later one too; anything else is ignored', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.addProject('second', 'minimal', 'Second Project');
    tasksServer.use('standard');
    tasksServer.link('/projects/second/list');
    await page.goto('/');
    await expect(page.locator('#dashboard-title')).toHaveText('Minimal Project');
    await expect.poll(() => addressOf(page)).toMatch(/^\/projects\/second\/list(\?|$)/);

    await tasksServer.pushLink(`/projects/${tasksServer.projectId}/kanban`);
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
    // Not a page of the dashboard: the page stays where it is
    await tasksServer.pushLink('/api/projects');
    await expect.poll(() => addressOf(page)).toMatch(new RegExp(`^/projects/${tasksServer.projectId}/kanban(\\?|$)`));
    expect(tasksServer.httpRequests).toEqual([]);
  });

  test('in the browser there is no button: the dialog gives the command instead', async ({ page, tasksServer }) => {
    tasksServer.use('standard');
    await page.goto('/?agents');
    await expect(page.locator('#footer-agents')).toHaveCount(0);
    await expect(page.locator('#agents-panel .agents-intro')).toContainText('atm agents install');
    await expect(page.locator('#agents-panel .agents-connect')).toHaveCount(0);
  });

  test('a project’s logo comes through the app too, as a data: URL the page’s CSP allows', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.addProject('second', 'minimal', 'Second Project');
    tasksServer.logo(
      'second',
      '<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8"><rect width="8" height="8"/></svg>',
    );
    tasksServer.use('standard');
    await page.goto('/projects');
    const logo = page.locator('.project-card[data-id="second"] .project-logo img');
    await expect(logo).toHaveAttribute('src', /^data:image\/svg\+xml;base64,/);
    expect(tasksServer.httpRequests).toEqual([]);
  });
});
