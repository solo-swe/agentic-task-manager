import { fireEvent, render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';
import ReviewProjects from '../../src/lib/overview/ReviewProjects.svelte';
import { settled } from './components.helpers';

// All reviews' project filter (lib/overview/ReviewProjects.svelte, ATM-757): All projects, or one or more of them;
// the choice is given and each change reported (the page keeps it in its address)

const projects = [
  { id: 'atm', name: 'Agentic Task Manager' },
  { id: 'compmd', name: 'CompMD' },
  { id: 'blog', name: 'Blog' },
];

function show(chosen: string[] = []) {
  const onchoose = vi.fn();
  const result = render(ReviewProjects, { projects, chosen, onchoose });
  return { ...result, onchoose, button: result.container.querySelector('#review-projects') as HTMLElement };
}

/** Open the menu as the keyboard does (jsdom's pointer events carry no pointer type, which Bits UI's trigger reads) */
async function openMenu(button: HTMLElement) {
  button.focus();
  await fireEvent.keyDown(button, { key: 'Enter' });
  await settled();
}

describe('ReviewProjects', () => {
  it('says what is chosen: all projects, one by its name, or how many', () => {
    expect(show().button.textContent).toContain('All projects');
    expect(show(['compmd']).button.textContent).toContain('CompMD');
    const { button } = show(['atm', 'blog']);
    expect(button.textContent).toContain('2 projects');
    expect(button.getAttribute('data-chosen')).toBe('atm,blog');
  });

  it('lists All projects, then each project, ticked when chosen', async () => {
    const { button } = show(['compmd']);
    await openMenu(button);
    const items = [...document.querySelectorAll<HTMLElement>('#review-projects-menu [data-project]')];
    expect(items.map((i) => i.dataset.project)).toEqual(['', 'atm', 'compmd', 'blog']);
    expect(items.map((i) => i.getAttribute('aria-checked'))).toEqual([null, 'false', 'true', 'false']);
  });

  it('a project picked is added to the choice, in the projects’ order; picked again, taken out', async () => {
    const one = show(['blog']);
    await openMenu(one.button);
    await fireEvent.click(screen.getByRole('menuitemcheckbox', { name: 'Agentic Task Manager' }));
    expect(one.onchoose).toHaveBeenLastCalledWith(['atm', 'blog']);
    one.unmount();
    const two = show(['atm', 'blog']);
    await openMenu(two.button);
    await fireEvent.click(screen.getByRole('menuitemcheckbox', { name: 'Blog' }));
    expect(two.onchoose).toHaveBeenLastCalledWith(['atm']);
  });

  it('All projects clears the choice', async () => {
    const { button, onchoose } = show(['atm']);
    await openMenu(button);
    await fireEvent.click(screen.getByRole('menuitem', { name: 'All projects' }));
    expect(onchoose).toHaveBeenLastCalledWith([]);
  });
});
