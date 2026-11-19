import { afterEach, describe, expect, it, vi } from 'vitest';
import { followScheme, resetRuntime, sandboxOf } from '../../src/lib/compmd-runtime';
import type { CompmdModule } from '../../src/lib/compmd';

// ATM-646: what every rendered text shares, made once for the document
describe('CompMD for the whole document', () => {
  afterEach(() => resetRuntime());

  const module = () => {
    const sandbox = { open: vi.fn() };
    return { compmd: { createSandbox: vi.fn(() => sandbox) } as unknown as CompmdModule, sandbox };
  };
  const options = () => ({ frames: 'f/', modules: () => undefined, page: 'host' as const });

  it('one sandbox for every text, made the first time; a new module (tests, a reload) makes its own', () => {
    const a = module();
    expect(sandboxOf(a.compmd, options)).toBe(a.sandbox);
    expect(sandboxOf(a.compmd, options)).toBe(a.sandbox);
    expect(a.compmd.createSandbox).toHaveBeenCalledTimes(1);
    const b = module();
    expect(sandboxOf(b.compmd, options)).toBe(b.sandbox);
    expect(b.compmd.createSandbox).toHaveBeenCalledTimes(1);
  });

  it("one observer on <html> tells every view the dashboard's theme changed; it goes with the last view", async () => {
    const changed = () => new Promise((r) => setTimeout(r, 0));
    const one = { scheme: vi.fn() };
    const two = { scheme: vi.fn() };
    const stopOne = followScheme(one);
    const stopTwo = followScheme(two);
    document.documentElement.setAttribute('data-mode', 'dark');
    await changed();
    expect([one.scheme.mock.calls.length, two.scheme.mock.calls.length]).toEqual([1, 1]);
    // Other attributes aren't the theme
    document.documentElement.setAttribute('lang', 'en');
    await changed();
    expect(one.scheme).toHaveBeenCalledTimes(1);
    // A view gone isn't told; the last one gone, nothing is
    stopOne();
    document.documentElement.setAttribute('data-theme', 'paper');
    await changed();
    expect([one.scheme.mock.calls.length, two.scheme.mock.calls.length]).toEqual([1, 2]);
    stopTwo();
    document.documentElement.setAttribute('data-mode', 'light');
    await changed();
    expect(two.scheme).toHaveBeenCalledTimes(2);
  });
});
