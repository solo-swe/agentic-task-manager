import { describe, expect, it } from 'vitest';
import { dialogOf, withDialog } from '../../src/lib/dialogs';

describe('dialogOf', () => {
  it('reads the dialog flag', () => {
    expect(dialogOf('/?about')).toBe('about');
    expect(dialogOf('/projects/p1/list?about')).toBe('about');
  });

  it('is null without one', () => {
    expect(dialogOf('/')).toBeNull();
    expect(dialogOf('/?view=about')).toBeNull();
  });
});

describe('withDialog', () => {
  it('opens a dialog, keeping the other parameters', () => {
    expect(withDialog('/projects/p1/kanban', 'about')).toBe('/projects/p1/kanban?about');
    expect(withDialog('/', 'about')).toBe('/?about');
  });

  it('closes it', () => {
    expect(withDialog('/projects/p1/kanban?sprint=all&about', null)).toBe('/projects/p1/kanban?sprint=all');
  });
});
