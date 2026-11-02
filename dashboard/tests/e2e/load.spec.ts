import { expect, openDashboard, test } from './helpers';

/** A project's tasks, as the dashboard loads them (docs/multi-project-api.md §2.5) */
const V1_TASKS = /\/api\/v1\/projects\/[^/]+\/tasks(\?.*)?$/;

test.describe('loading tasks.json', () => {
  test('uses meta.project for the page and heading titles', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await expect(page).toHaveTitle('Standard Project');
    await expect(page.locator('#dashboard-title')).toHaveText('Standard Project');
  });

  test('subtitle counts tasks and sprints (here the phases, read as sprints)', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'standard');
    await expect(page.locator('#subtitle')).toHaveText('12 tasks across 3 sprints — loaded from tasks.json');
  });

  test('subtitle uses singular "sprint" (but always "tasks")', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'minimal');
    await expect(page.locator('#subtitle')).toHaveText('1 tasks across 1 sprint — loaded from tasks.json');
  });

  test('falls back to "Task Dashboard" without meta', async ({ page, tasksServer }) => {
    await openDashboard(page, tasksServer, 'no-meta');
    await expect(page).toHaveTitle('Task Dashboard');
    await expect(page.locator('#dashboard-title')).toHaveText('Task Dashboard');
  });

  test('fetches the tasks without a cache-buster, then asks whether they changed (If-None-Match)', async ({
    page,
    tasksServer,
  }) => {
    const first = page.waitForRequest(V1_TASKS);
    await openDashboard(page, tasksServer, 'minimal');
    expect((await first).headers()['if-none-match']).toBeUndefined();
    const second = page.waitForRequest(V1_TASKS);
    await page.locator('#reload-btn').click();
    expect((await second).headers()['if-none-match']).toBe('"v1"');
    await expect.poll(() => tasksServer.notModified).toBe(1);
  });

  test('loads the project list, then the project’s tasks from the v1 route (T-365)', async ({ page, tasksServer }) => {
    const projects = page.waitForRequest(/\/api\/projects$/);
    const tasks = page.waitForRequest(V1_TASKS);
    await openDashboard(page, tasksServer, 'minimal');
    await projects;
    await tasks;
    await expect(page.locator('#subtitle')).toContainText('1 tasks across 1 sprint');
  });

  test('shows how to start the server when tasks.json is missing', async ({ page, tasksServer }) => {
    tasksServer.fail(404);
    await page.goto('tasks-dashboard.html');
    const error = page.locator('#load-error');
    await expect(error).toContainText('Failed to load tasks.json');
    await expect(error).toContainText('HTTP 404');
    await expect(error).toContainText('/atm:serve');
    await expect(error).toContainText('atm serve --open');
  });

  test('says how to add a project when none is registered', async ({ page, tasksServer }) => {
    tasksServer.noProjects();
    await page.goto('tasks-dashboard.html');
    await expect(page.locator('#no-projects')).toContainText('/atm:create-tasks');
    await expect(page.locator('#no-projects')).toContainText('atm projects add <path>');
  });

  test('shows the error box for invalid JSON', async ({ page, tasksServer }) => {
    tasksServer.use('minimal');
    await page.route(V1_TASKS, (route) => route.fulfill({ body: '{ not json' }));
    await page.goto('tasks-dashboard.html');
    await expect(page.locator('#load-error')).toContainText('Failed to load tasks.json');
    await expect(page.locator('#load-error')).toContainText('JSON');
  });
});
