import { fireEvent, render, screen } from '@testing-library/svelte';
import { flushSync } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import HelpDialog from '../../src/lib/dialogs/HelpDialog.svelte';
import { slug } from '../../src/lib/compmd';
import { dashboard } from '../../src/lib/state.svelte';
import standard from '../fixtures/standard.json';
import { compmdSettled } from './compmd.helpers';
import { loadIntoDashboard, press, settled } from './components.helpers';

// Help renders docs/help.md with the real CompMD, as the served module has it
const loadCompmd = vi.hoisted(() => vi.fn());
vi.mock('../../src/lib/compmd', async (actual) => {
  const { realCompmd } = await import('./compmd.helpers');
  loadCompmd.mockImplementation(realCompmd);
  return { ...(await actual()), loadCompmd };
});

const overlay = () => document.querySelector('#help-overlay')!;
const heading = (dialog: HTMLElement, text: string) =>
  [...dialog.querySelectorAll<HTMLElement>('h1, h2, h3, h4, h5, h6')].find((h) => h.textContent === text);

async function openHelp() {
  dashboard.openDialog('help');
  flushSync();
  await compmdSettled();
  return screen.getByRole('dialog', { name: 'Help' });
}

describe('HelpDialog', () => {
  beforeEach(() => {
    loadIntoDashboard(standard);
    dashboard.closeDialog();
  });

  it('is closed until opened, and loads CompMD only then; then shows docs/help.md', async () => {
    loadCompmd.mockClear();
    render(HelpDialog);
    expect(overlay()).not.toHaveClass('open');
    expect(loadCompmd).not.toHaveBeenCalled();
    const dialog = await openHelp();
    expect(overlay()).toHaveClass('open');
    expect(dialog.querySelector('.compmd')).toHaveAttribute('data-compmd', 'ready');
    expect(heading(dialog, 'Statuses and kinds')?.tagName).toBe('H2');
    expect(dialog).toHaveTextContent('/atm-loop-tasks');
    expect(location.search).toBe('?help');
    // Headings have no IDs: an element with the ID "help" would make the browser jump to it on "#help"
    expect(document.getElementById('help')).toBeNull();
  });

  it('closes with the close button, Escape and the backdrop, returning focus', async () => {
    const opener = document.body.appendChild(document.createElement('button'));
    opener.focus();
    render(HelpDialog);
    for (const close of [
      () => fireEvent.click(screen.getByRole('button', { name: 'Close' })),
      () => fireEvent.keyDown(document, { key: 'Escape' }),
      () => press(overlay()),
    ]) {
      opener.focus();
      dashboard.openDialog('help');
      flushSync();
      expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Close' }));
      await settled();
      await close();
      flushSync();
      expect(dashboard.dialog).toBeNull();
      expect(document.activeElement).toBe(opener);
    }
    opener.remove();
  });

  it('links within the page scroll to the heading and leave the address alone', async () => {
    render(HelpDialog);
    const dialog = await openHelp();
    const target = heading(dialog, 'Priorities and labels')!;
    let scrolled = false;
    target.scrollIntoView = () => (scrolled = true);
    await fireEvent.click(screen.getByRole('link', { name: 'priorities and labels' }));
    expect(scrolled).toBe(true);
    expect(location.search).toBe('?help');
  });
});

describe('docs/help.md', () => {
  it('covers the topics D-1.3 asks for; links within the page go to headings that exist, off it to the repository, CompMD and Mermaid', async () => {
    render(HelpDialog);
    const dialog = await openHelp();
    const slugs = [...dialog.querySelectorAll('h1, h2, h3, h4, h5, h6')].map((h) => slug(h.textContent ?? ''));
    for (const id of [
      'views',
      'filters-and-saved-views',
      'statuses-and-kinds',
      'waiting-on-you',
      'priorities-and-labels',
      'the-task-details',
      'workflows-and-stages',
      'the-archive',
      'the-skills',
      'running-agents-in-parallel',
    ])
      expect(slugs).toContain(id);

    const links = [...dialog.querySelectorAll('.compmd a')];
    expect(links.length).toBeGreaterThan(0);
    for (const a of links) {
      const href = a.getAttribute('href') ?? '';
      if (href.startsWith('#')) expect(slugs).toContain(href.slice(1));
      else {
        expect(href).toMatch(
          /^https:\/\/(github\.com\/solo-swe\/agentic-task-manager|compmd\.hakanserce\.com\/$|mermaid\.js\.org\/$)/,
        );
        expect(a).toHaveAttribute('target', '_blank');
        expect(a).toHaveAttribute('rel', 'noopener noreferrer');
      }
    }
  });
});
