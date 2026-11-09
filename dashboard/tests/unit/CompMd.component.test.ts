import { render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import CompMd from '../../src/lib/documents/CompMd.svelte';
import type { CompmdModule } from '../../src/lib/compmd';
import { realCompmd } from './compmd.helpers';

// Nothing fails silently (ADR-004, docs/tauri-compmd-design.md §4.3): when CompMD can't load, or a component draws
// nothing, the text says so, and the console says why
const loadCompmd = vi.hoisted(() => vi.fn());
vi.mock('../../src/lib/compmd', async (actual) => ({ ...(await actual()), loadCompmd }));

type Listener = (event: { component: string; reason: string; message: string }) => void;

/** The real processor, with a sandbox whose view only records its listeners, so a test can raise its events */
async function compmdWithView() {
  const listeners: Record<string, Listener[]> = {};
  const view = {
    on(event: string, listener: Listener) {
      (listeners[event] ??= []).push(listener);
    },
    render() {},
    scheme() {},
    destroy() {},
    async *lines() {},
  };
  const compmd = await realCompmd();
  const createSandbox = vi.fn(() => ({ open: () => view }));
  loadCompmd.mockResolvedValue({ ...compmd, createSandbox } as unknown as CompmdModule);
  return {
    raise: (event: string, data: Parameters<Listener>[0]) => listeners[event]?.forEach((l) => l(data)),
    createSandbox,
  };
}

afterEach(() => {
  vi.restoreAllMocks();
  loadCompmd.mockReset();
});

describe('CompMd', () => {
  it('when CompMD can’t load, the text shows as written, with a note saying why', async () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    loadCompmd.mockRejectedValue(new Error('compmd.js: 404'));
    const { container } = render(CompMd, { text: '# A heading' });
    const note = await screen.findByRole('note');
    expect(note).toHaveTextContent('Shown as written: the formatting couldn’t load (compmd.js: 404).');
    expect(container.querySelector('.compmd')).toHaveAttribute('data-compmd', 'failed');
    expect(container.querySelector('.compmd-plain')).toHaveTextContent('# A heading');
    expect(error).toHaveBeenCalled();
  });

  it('in a browser, the sandbox is given its base as a full URL on the server’s origin (H4)', async () => {
    const { createSandbox } = await compmdWithView();
    render(CompMd, { text: 'text' });
    await vi.waitFor(() => expect(createSandbox).toHaveBeenCalled());
    const [options] = createSandbox.mock.calls[0] as unknown as [{ base: string; page: string }];
    expect(options.page).toBe('host');
    expect(options.base).toMatch(new RegExp(`^${location.origin}/`));
  });

  it('a component that draws nothing gets one note, and the console the likely cause', async () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    const { raise, createSandbox } = await compmdWithView();
    render(CompMd, { text: 'text' });
    await vi.waitFor(() => expect(createSandbox).toHaveBeenCalled());
    const failed = { component: 'mermaid', reason: 'failed', message: 'the frame never answered' };
    raise('error', failed);
    raise('error', failed);
    const notes = await screen.findAllByRole('note');
    expect(notes).toHaveLength(1);
    expect(notes[0]).toHaveTextContent('A mermaid here couldn’t be drawn; the console says why.');
    expect(error).toHaveBeenCalledWith(expect.stringContaining('refused by a policy'));
  });

  it('a component that shows its fallback only warns: the fallback says it on the page', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const { raise, createSandbox } = await compmdWithView();
    render(CompMd, { text: 'text' });
    await vi.waitFor(() => expect(createSandbox).toHaveBeenCalled());
    raise('error', { component: 'mermaid', reason: 'threw', message: 'Parse error' });
    await vi.waitFor(() => expect(warn).toHaveBeenCalledWith(expect.stringContaining('its fallback (threw)')));
    expect(screen.queryByRole('note')).toBeNull();
  });
});
