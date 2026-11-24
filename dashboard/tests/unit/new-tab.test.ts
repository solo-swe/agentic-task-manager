import { describe, expect, it } from 'vitest';
import { filtered, keyed, scopesOf, type Chooser } from '../../src/lib/new-tab';

// The new tab's chooser (lib/new-tab.ts, ATM-755): the scopes it lists, and what each key does

const projects = [
  { id: 'atm', name: 'Agentic Task Manager' },
  { id: 'compmd', name: 'CompMD' },
  { id: 'lfb', name: 'Learn Forever Blog' },
];
const start: Chooser = { scope: null, item: 0, column: 0, filter: '' };
const after = (state: Chooser, ...keys: string[]) =>
  keys.reduce((s, key) => {
    const done = keyed(s, projects, key);
    if (!done || !('state' in done)) throw new Error(`${key}: ${JSON.stringify(done)}`);
    return done.state;
  }, state);

describe('the scopes listed', () => {
  it('All, then every project; a filter keeps those it names (name or ID, any case), All only when it matches', () => {
    expect(scopesOf(projects, '')).toEqual([null, 'atm', 'compmd', 'lfb']);
    expect(scopesOf(projects, 'COMP')).toEqual(['compmd']);
    expect(scopesOf(projects, 'lfb')).toEqual(['lfb']);
    expect(scopesOf(projects, 'task')).toEqual(['atm']);
    expect(scopesOf(projects, 'all')).toEqual([null]);
    expect(scopesOf(projects, 'zzz')).toEqual([]);
  });
});

describe('the keys', () => {
  it('↑↓ move in the scopes, at the ends staying; the item follows within the new scope’s', () => {
    expect(after(start, 'ArrowDown').scope).toBe('atm');
    expect(after(start, 'ArrowDown', 'ArrowDown', 'ArrowDown', 'ArrowDown').scope).toBe('lfb');
    expect(after(start, 'ArrowUp').scope).toBe(null);
    // On a project's sixth item, back to All (four): its last
    expect(after({ ...start, scope: 'atm', item: 5 }, 'ArrowUp')).toMatchObject({ scope: null, item: 3 });
  });

  it('→ goes to the items, where ↑↓ move within them; ← back to the scopes', () => {
    const items = after({ ...start, scope: 'atm' }, 'ArrowRight');
    expect(items.column).toBe(1);
    expect(after(items, 'ArrowDown', 'ArrowDown').item).toBe(2);
    expect(after(items, 'ArrowUp').item).toBe(0);
    expect(after(items, ...Array(9).fill('ArrowDown')).item).toBe(5);
    expect(after(items, 'ArrowLeft').column).toBe(0);
  });

  it('Enter opens the chosen item of the chosen scope', () => {
    expect(keyed({ ...start, scope: 'atm', item: 2 }, projects, 'Enter')).toEqual({ open: 'tasks' });
    expect(keyed({ ...start, item: 2 }, projects, 'Enter')).toEqual({ open: 'reviews' });
    // Nothing to open while the filter lists nothing
    expect(keyed({ ...start, scope: 'atm', filter: 'zzz' }, projects, 'Enter')).toBeNull();
  });

  it('typing filters the projects and chooses the first kept; Backspace and Escape undo it', () => {
    const typed = after(start, 'c', 'o');
    expect(typed).toEqual({ scope: 'compmd', item: 0, column: 0, filter: 'co' });
    expect(after(typed, 'Backspace')).toMatchObject({ filter: 'c', scope: 'compmd' });
    expect(after(typed, 'Escape')).toMatchObject({ filter: '', scope: 'compmd' });
    // Typed from the items' column: back to the scopes
    expect(after({ ...start, column: 1 }, 'l').column).toBe(0);
  });

  it('a scope still listed keeps its item; other keys do nothing', () => {
    expect(filtered({ ...start, scope: 'atm', item: 4 }, projects, 'a')).toMatchObject({ scope: 'atm', item: 4 });
    for (const key of [' ', 'Tab', 'Shift', 'F5']) expect(keyed(start, projects, key), key).toBeNull();
    expect(keyed(start, projects, 'Backspace')).toBeNull();
  });
});
