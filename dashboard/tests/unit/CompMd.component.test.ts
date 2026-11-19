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
  // ATM-586: a code review's summary and comments are the project's text: given onopen, their relative links open the
  // document viewer, as a task's do; without it (Help, What's new) they stay text
  it('given onopen, a relative link opens its document; without, it stays text', async () => {
    loadCompmd.mockImplementation(realCompmd);
    const onopen = vi.fn();
    const text = 'See [the design](docs/design.md#routes) and [lines](src/a.rs#L3-L4).';
    const { container, unmount } = render(CompMd, { text, onopen });
    await vi.waitFor(() => expect(container.querySelectorAll('a')).toHaveLength(2));
    container.querySelectorAll('a').forEach((a) => a.click());
    expect(onopen.mock.calls).toEqual([
      [{ path: 'docs/design.md', heading: 'routes' }],
      [{ path: 'src/a.rs', lines: [3, 4] }],
    ]);
    unmount();
    const plain = render(CompMd, { text });
    await vi.waitFor(() => expect(plain.container.querySelector('.compmd')).toHaveAttribute('data-compmd', 'ready'));
    expect(plain.container.querySelector('a')).toBeNull();
    expect(plain.container).toHaveTextContent('See the design and lines.');
  });

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

    // ATM-625: a run cut in two by a comment kept its old drawing (all its old lines) until its next turn, so the box
    // under its first half was far below, and focusing it scrolled the pane
    it('drawn, then given a new text: the new text as written at once, the old drawing hidden, until its turn', async () => {
      const idle: (() => void)[] = [];
      vi.stubGlobal('requestIdleCallback', (act: () => void) => idle.push(act));
      const { raise } = await compmdWithView();
      const { container, rerender } = render(CompMd, { text: 'one\ntwo\nthree', plain: 'one\ntwo\nthree' });
      await vi.waitFor(() => expect(idle).toHaveLength(1));
      idle.shift()!();
      raise('rendered', {} as never);
      const outer = () => container.querySelector('.compmd')!;
      const page = () => container.querySelector('.compmd-page:not(.compmd-waiting)') as HTMLElement;
      const waiting = () => container.querySelector('.compmd-waiting');
      await vi.waitFor(() => expect(outer()).toHaveAttribute('data-compmd', 'ready'));
      page().textContent = 'one two three'; // what CompMD drew
      await rerender({ text: 'one', plain: 'one' });
      expect(outer()).toHaveAttribute('data-compmd', 'loading');
      expect(outer()).toHaveClass('compmd-stale');
      expect(page().hidden).toBe(true);
      expect(waiting()).toHaveTextContent(/^one$/);
      // Its turn: drawn, and the plain text gives way
      await vi.waitFor(() => expect(idle).toHaveLength(1));
      idle.shift()!();
      raise('rendered', {} as never);
      await vi.waitFor(() => expect(outer()).toHaveAttribute('data-compmd', 'ready'));
      expect(outer()).not.toHaveClass('compmd-stale');
      expect(page().hidden).toBe(false);
      expect(waiting()).toBeNull();
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

    // ATM-620: a view made after its text was gone was never destroyed, and held its card in memory
    it('gone after its turn was given, before the page was idle, it never renders, and the next one goes', async () => {
      const idle: (() => void)[] = [];
      vi.stubGlobal('requestIdleCallback', (act: () => void) => idle.push(act));
      const { createSandbox } = await compmdWithView();
      const first = render(CompMd, { text: 'one', plain: 'one' });
      await vi.waitFor(() => expect(idle).toHaveLength(1));
      expect(turnsWaiting().running).toBe(true);
      first.unmount();
      idle.shift()!();
      expect(createSandbox).not.toHaveBeenCalled();
      expect(turnsWaiting()).toEqual({ waiting: 0, running: false });
    });
  });

  it('gone before CompMD has loaded, it never renders', async () => {
    const { createSandbox } = await compmdWithView();
    let loaded: (m: CompmdModule) => void = () => {};
    const module = (await loadCompmd()) as CompmdModule;
    loadCompmd.mockReturnValue(new Promise<CompmdModule>((resolve) => (loaded = resolve)));
    const shown = render(CompMd, { text: '# Gone' });
    shown.unmount();
    loaded(module);
    await new Promise((r) => setTimeout(r, 0));
    expect(createSandbox).not.toHaveBeenCalled();
  });
});
