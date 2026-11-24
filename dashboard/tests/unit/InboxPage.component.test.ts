import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import InboxPage from '../../src/lib/inbox/InboxPage.svelte';
import type { InboxListing } from '../../src/lib/api/inbox';
import { dashboard } from '../../src/lib/state.svelte';
import standard from '../fixtures/standard.json';
import { loadIntoDashboard, settled } from './components.helpers';

// The Inbox page (lib/inbox/InboxPage.svelte, ATM-762): the client (api/inbox.ts) is a stub here, as are the address
// and the navigation

const where = vi.hoisted(() => ({ address: '/projects/fixture/inbox' }));
const navigate = vi.hoisted(() => vi.fn());
vi.mock('../../src/lib/navigation', async (actual) => ({
  ...(await actual()),
  navigate,
  currentAddress: () => where.address,
}));
const api = vi.hoisted(() => ({
  loadInbox: vi.fn(),
  addItem: vi.fn(),
  editItem: vi.fn(),
  discardItem: vi.fn(),
  restoreItem: vi.fn(),
}));
vi.mock('../../src/lib/api/inbox', () => api);

const listing = (items: InboxListing['items']): InboxListing => ({ items, created: 2, discarded: 1, injected: 1 });
const ITEMS: InboxListing['items'] = [
  { id: 'IN-4', text: 'Scroll to the moved card\nAnd keep it in view.', state: 'created', created_by: 'Hakan' },
  {
    id: 'IN-3',
    text: 'Look at how others triage',
    state: 'created',
    created_by: 'claude:ab12',
    from_project: 'LFB',
  },
  { id: 'IN-2', text: 'a dark sidebar?', state: 'discarded', reason: 'The themes set it.' },
  { id: 'IN-1', text: 'Remember the diff position', state: 'injected', tasks: ['T-001', 'LFB-5'] },
];
const ok = { ok: true as const, body: {} };

describe('InboxPage', () => {
  beforeEach(() => {
    loadIntoDashboard(standard);
    dashboard.projectId = 'fixture';
    where.address = '/projects/fixture/inbox';
    dashboard.address = where.address;
    for (const f of Object.values(api)) f.mockReset();
    api.loadInbox.mockResolvedValue({ unchanged: false, listing: listing(ITEMS), validators: {} });
    for (const f of [api.addItem, api.editItem, api.discardItem, api.restoreItem]) f.mockResolvedValue(ok);
    navigate.mockClear();
  });
  afterEach(() => {
    dashboard.projectId = null;
  });

  const row = (c: HTMLElement, id: string) => c.querySelector<HTMLElement>(`[data-item="${id}"]`)!;

  it('lists the items by their first line, with the counts; each state its own actions', async () => {
    const { container } = render(InboxPage);
    await waitFor(() => expect(container.querySelectorAll('.inbox-item')).toHaveLength(4));
    expect(api.loadInbox).toHaveBeenCalledWith('fixture', 'created', false, undefined, undefined);
    expect(row(container, 'IN-4').querySelector('.inbox-title')).toHaveTextContent('Scroll to the moved card');
    expect([...container.querySelectorAll('.inbox-chip')].map((c) => c.textContent)).toEqual([
      'Open2',
      'Discarded1',
      'Injected1',
      'All4',
    ]);
    const actions = (id: string) =>
      [...row(container, id).querySelectorAll<HTMLElement>('[data-action]')].map((a) => a.dataset.action);
    expect(actions('IN-4')).toEqual(['edit', 'discard']);
    expect(actions('IN-2')).toEqual(['restore']);
    expect(actions('IN-1')).toEqual([]);
    expect(row(container, 'IN-2')).toHaveTextContent('discarded: The themes set it.');
    // Left by another project's agent (ATM-772): from that project; the project's own say nothing of it
    expect(row(container, 'IN-3').querySelector('.inbox-from')).toHaveTextContent('from LFB');
    expect(row(container, 'IN-3').querySelector('.inbox-from')).toHaveAttribute(
      'title',
      'Left here by claude:ab12 working in LFB',
    );
    expect(row(container, 'IN-4').querySelector('.inbox-from')).toBeNull();
    // Injected: its tasks, this project's a link to its details here, another's as text
    const tasks = row(container, 'IN-1').querySelector('.inbox-tasks')!;
    expect(tasks.querySelector('a[data-task="T-001"]')).toHaveAttribute('href', '/projects/fixture/inbox?task=T-001');
    expect(tasks).toHaveTextContent('LFB-5');
    expect(tasks.querySelector('a[data-task="LFB-5"]')).toBeNull();
  });

  it('the rest of an item’s text on a click on its title', async () => {
    const { container } = render(InboxPage);
    await waitFor(() => expect(row(container, 'IN-4')).toBeTruthy());
    const title = row(container, 'IN-4').querySelector<HTMLButtonElement>('button.inbox-title')!;
    await fireEvent.click(title);
    expect(title).toHaveAttribute('aria-expanded', 'true');
    expect(row(container, 'IN-4').querySelector('.inbox-text')).toBeTruthy();
    // One line: no more to show
    expect(row(container, 'IN-3').querySelector('button.inbox-title')).toBeNull();
  });

  it('the quick-add box is focused; Enter adds and empties it, keeping the focus; ⇧Enter is a new line', async () => {
    const { container } = render(InboxPage);
    const box = container.querySelector<HTMLTextAreaElement>('#inbox-add-text')!;
    expect(document.activeElement).toBe(box);
    await fireEvent.input(box, { target: { value: 'two lines' } });
    await fireEvent.keyDown(box, { key: 'Enter', shiftKey: true });
    expect(api.addItem).not.toHaveBeenCalled();
    await fireEvent.keyDown(box, { key: 'Enter' });
    await waitFor(() => expect(api.addItem).toHaveBeenCalledWith('fixture', 'two lines'));
    await waitFor(() => expect(box.value).toBe(''));
    expect(document.activeElement).toBe(box);
    // Asked again after it
    expect(api.loadInbox.mock.calls.length).toBeGreaterThan(1);
  });

  it('a refused add says why and keeps the text', async () => {
    api.addItem.mockResolvedValue({ ok: false, message: 'an item of at most 20,000 characters' });
    const { container } = render(InboxPage);
    const box = container.querySelector<HTMLTextAreaElement>('#inbox-add-text')!;
    await fireEvent.input(box, { target: { value: 'x' } });
    await fireEvent.click(container.querySelector('#inbox-add')!);
    await waitFor(() => expect(container.querySelector('#inbox-problem')).toHaveTextContent('20,000'));
    expect(box.value).toBe('x');
  });

  it('the filter and archived are the address’s: a choice is a step there; the address’s is asked', async () => {
    where.address = '/projects/fixture/inbox?state=discarded&archived=1';
    dashboard.address = where.address;
    const { container } = render(InboxPage);
    await waitFor(() => expect(api.loadInbox).toHaveBeenCalledWith('fixture', 'discarded', true, undefined, undefined));
    expect(container.querySelector('[data-state="discarded"]')).toHaveAttribute('aria-pressed', 'true');
    expect(container.querySelector<HTMLInputElement>('#inbox-archived')!.checked).toBe(true);
    await fireEvent.click(container.querySelector('.inbox-chip[data-state="all"]')!);
    expect(navigate).toHaveBeenLastCalledWith('/projects/fixture/inbox?state=all&archived=1');
    await fireEvent.click(container.querySelector('#inbox-archived')!);
    expect(navigate).toHaveBeenLastCalledWith('/projects/fixture/inbox?state=discarded');
  });

  it('Discard asks why, and needs a reason; Restore brings one back', async () => {
    const { container } = render(InboxPage);
    await waitFor(() => expect(row(container, 'IN-3')).toBeTruthy());
    await fireEvent.click(row(container, 'IN-3').querySelector('[data-action="discard"]')!);
    await settled();
    const confirm = document.querySelector<HTMLButtonElement>('#inbox-discard-confirm')!;
    expect(confirm.disabled).toBe(true);
    const reason = document.querySelector<HTMLTextAreaElement>('#inbox-discard-reason')!;
    await fireEvent.input(reason, { target: { value: '  Done already. ' } });
    await fireEvent.click(confirm);
    await waitFor(() => expect(api.discardItem).toHaveBeenCalledWith('fixture', 'IN-3', 'Done already.'));
    await fireEvent.click(row(container, 'IN-2').querySelector('[data-action="restore"]')!);
    await waitFor(() => expect(api.restoreItem).toHaveBeenCalledWith('fixture', 'IN-2'));
  });

  it('Edit changes an open item’s text in place', async () => {
    const { container } = render(InboxPage);
    await waitFor(() => expect(row(container, 'IN-3')).toBeTruthy());
    await fireEvent.click(row(container, 'IN-3').querySelector('[data-action="edit"]')!);
    const text = screen.getByRole('textbox', { name: "The item's text" }) as HTMLTextAreaElement;
    expect(text.value).toBe('Look at how others triage');
    await fireEvent.input(text, { target: { value: 'Look at how Linear triages' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(api.editItem).toHaveBeenCalledWith('fixture', 'IN-3', 'Look at how Linear triages'));
  });

  it('an empty inbox says how it is used; one that can’t be read, why', async () => {
    api.loadInbox.mockResolvedValue({ unchanged: false, listing: listing([]), validators: {} });
    const one = render(InboxPage);
    await waitFor(() => expect(one.container.querySelector('#inbox-empty')).toHaveTextContent('/atm-triage-inbox'));
    one.unmount();
    api.loadInbox.mockRejectedValue(new Error('HTTP 500'));
    const two = render(InboxPage);
    await waitFor(() => expect(two.container.querySelector('#inbox-problem')).toHaveTextContent('HTTP 500'));
  });
});
