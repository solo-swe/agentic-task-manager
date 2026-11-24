import type { Page } from '@playwright/test';
import { expect, openDashboard, test, type FixtureInboxItem, type TasksServer } from './helpers';
import { E2E_SESSION } from './session.mjs';

// The Inbox page (docs/inbox-design.md §6, ATM-762), with the fixture playing atm's inbox routes and commands

const ITEMS: FixtureInboxItem[] = [
  { id: 'IN-3', text: 'Look at how others triage', state: 'created', created_by: 'Hakan' },
  { id: 'IN-2', text: 'a dark sidebar?', state: 'discarded', reason: 'The themes set it.', created_by: 'Hakan' },
  { id: 'IN-1', text: 'Remember the diff position', state: 'injected', tasks: ['T-004'], created_by: 'Hakan' },
];

async function openInbox(page: Page, tasksServer: TasksServer, items = ITEMS) {
  tasksServer.inbox = items.map((i) => ({ ...i }));
  await openDashboard(page, tasksServer, 'standard', { tab: 'inbox' });
  await expect(page.locator('#inbox-page')).toBeVisible();
}

const rows = (page: Page) => page.locator('#inbox-items .inbox-item');

test.describe('the Inbox page', () => {
  test('lists the open items; Enter in the box adds one, as the user, keeping the box focused', async ({
    page,
    tasksServer,
  }) => {
    await openInbox(page, tasksServer);
    await expect(rows(page)).toHaveCount(1);
    await expect(page.locator('.inbox-chip[data-state="created"]')).toHaveText('Open1');
    const box = page.locator('#inbox-add-text');
    await expect(box).toBeFocused();
    await box.fill('Scroll to the moved card');
    await box.press('Enter');
    await expect(rows(page)).toHaveCount(2);
    await expect(rows(page).first()).toContainText('IN-4');
    await expect(rows(page).first()).toContainText('Scroll to the moved card');
    await expect(box).toHaveValue('');
    await expect(box).toBeFocused();
    expect(tasksServer.inboxChanges).toEqual([
      { method: 'POST', path: '', body: { text: 'Scroll to the moved card' }, session: E2E_SESSION },
    ]);
  });

  test('Discard asks why; the item is then in Discarded with its reason, and Restore brings it back', async ({
    page,
    tasksServer,
  }) => {
    await openInbox(page, tasksServer);
    await rows(page).first().locator('[data-action="discard"]').click();
    const confirm = page.locator('#inbox-discard-confirm');
    await expect(confirm).toBeDisabled();
    await page.locator('#inbox-discard-reason').fill('Not now.');
    await confirm.click();
    await expect(page.locator('#inbox-empty')).toBeVisible();
    // The filter is the address's: Discarded, kept over a reload
    await page.locator('.inbox-chip[data-state="discarded"]').click();
    await expect(page).toHaveURL(/\/inbox\?(.*&)?state=discarded$/);
    await page.reload();
    await expect(rows(page)).toHaveCount(2);
    const it = page.locator('[data-item="IN-3"]');
    await expect(it).toContainText('discarded: Not now.');
    await it.locator('[data-action="restore"]').click();
    await expect(rows(page)).toHaveCount(1);
    // Back: Open again
    await page.goBack();
    await expect(page.locator('.inbox-chip[data-state="created"]')).toHaveAttribute('aria-pressed', 'true');
    await expect(rows(page)).toHaveCount(1);
  });

  test('All lists every state; an injected item links its task, which opens over the page', async ({
    page,
    tasksServer,
  }) => {
    await openInbox(page, tasksServer);
    await page.locator('.inbox-chip[data-state="all"]').click();
    await expect(rows(page)).toHaveCount(3);
    await page.locator('[data-item="IN-1"] .inbox-tasks a[data-task="T-004"]').click();
    await expect(page.locator('#detail-overlay')).toHaveClass(/open/);
    await expect(page).toHaveURL(/\/inbox\?(.*&)?state=all&task=T-004$/);
  });

  test('Edit changes an open item’s text in place', async ({ page, tasksServer }) => {
    await openInbox(page, tasksServer);
    await rows(page).first().locator('[data-action="edit"]').click();
    const text = page.getByRole('textbox', { name: "The item's text" });
    await text.fill('Look at how Linear triages');
    await page.getByRole('button', { name: 'Save' }).click();
    await expect(rows(page).first()).toContainText('Look at how Linear triages');
    expect(tasksServer.inboxChanges.at(-1)).toMatchObject({ method: 'PATCH', path: '/IN-3' });
  });

  test('in the app: through its commands, with no request to a server; the draft kept on its tab', async ({
    page,
    tasksServer,
  }) => {
    await tasksServer.asApp();
    tasksServer.use('standard');
    tasksServer.inbox = ITEMS.map((i) => ({ ...i }));
    await page.goto(`/projects/${tasksServer.projectId}/inbox`);
    await expect(rows(page)).toHaveCount(1);
    const box = page.locator('#inbox-add-text');
    await box.fill('From the app');
    await box.press('Enter');
    await expect(rows(page)).toHaveCount(2);
    expect(tasksServer.inboxChanges.at(-1)).toMatchObject({ method: 'add_inbox_item' });
    expect(tasksServer.httpRequests.filter((p) => p.includes('/inbox'))).toEqual([]);
    // Half written, another tab and back: still there
    await box.fill('Half written');
    await page.locator('#app-tab-new').click();
    await expect(page.locator('#new-tab')).toBeVisible();
    await page.locator('#app-titlebar [role="tab"]').first().click();
    await expect(page.locator('#inbox-add-text')).toHaveValue('Half written');
  });
});
