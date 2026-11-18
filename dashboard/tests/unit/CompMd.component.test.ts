import { render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import CompMd from '../../src/lib/documents/CompMd.svelte';
import type { CompmdModule } from '../../src/lib/compmd';
import { resetTurns, turnsWaiting } from '../../src/lib/render-turns';
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

  it('in a browser, the sandbox is given the bundle’s origin, where atm serve serves it (ADR-006)', async () => {
    const { createSandbox } = await compmdWithView();
    render(CompMd, { text: 'text' });
    await vi.waitFor(() => expect(createSandbox).toHaveBeenCalled());
    const [options] = createSandbox.mock.calls[0] as unknown as [{ origin: string; page: string }];
    expect(options.page).toBe('host');
    expect(options.origin).toMatch(new RegExp(`^${location.origin}/assets/compmd/[^/]+/compmd/$`));
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

  describe('with `plain` (ATM-587: a review’s runs of code)', () => {
    afterEach(() => {
      resetTurns();
      vi.unstubAllGlobals();
    });

    it('shows the code as written, then waits its turn among such renders; drawn, CompMD’s page is all there is', async () => {
      const idle: (() => void)[] = [];
      vi.stubGlobal('requestIdleCallback', (act: () => void) => idle.push(act));
      const { raise, createSandbox } = await compmdWithView();
      const { container } = render(CompMd, { text: '```ts\nlet a = 1;\n```', plain: 'let a = 1;' });
      const waiting = () => container.querySelector('.compmd-waiting');
      expect(waiting()).toHaveTextContent('let a = 1;');
      // Its turn when the page is idle: only then made and rendered
      await vi.waitFor(() => expect(idle).toHaveLength(1));
      expect(createSandbox).not.toHaveBeenCalled();
      idle.shift()!();
      expect(createSandbox).toHaveBeenCalled();
      expect(turnsWaiting().running).toBe(true);
      raise('rendered', {} as never);
      await vi.waitFor(() => expect(waiting()).toBeNull());
      await vi.waitFor(() => expect(turnsWaiting().running).toBe(false));
      expect(container.querySelector('.compmd')).toHaveAttribute('data-compmd', 'ready');
    });

    it('gone before its turn, it never renders', async () => {
      const idle: (() => void)[] = [];
      vi.stubGlobal('requestIdleCallback', (act: () => void) => idle.push(act));
      const { createSandbox } = await compmdWithView();
      const first = render(CompMd, { text: 'one', plain: 'one' });
      const second = render(CompMd, { text: 'two', plain: 'two' });
      await vi.waitFor(() => expect(turnsWaiting().waiting + idle.length).toBe(2));
      second.unmount();
      expect(turnsWaiting().waiting).toBe(0);
      idle.shift()!();
      expect(createSandbox).toHaveBeenCalledTimes(1);
      first.unmount();
    });
  });
});
