// The dashboard in the atm app (T-457): the same page, its data from the app's commands over Tauri's IPC (mocked here
// as `@tauri-apps/api/mocks` does, backed by the fixture's data), never from HTTP.
import { expect, test } from './helpers';

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
