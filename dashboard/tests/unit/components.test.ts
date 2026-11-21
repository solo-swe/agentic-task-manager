import { describe, expect, it } from 'vitest';
import { componentProgress } from '../../src/lib/component-progress';
import { componentLabel, componentOrder, taskComponents, type Component } from '../../src/lib/components';
import type { Task } from '../../src/lib/types';
import { withKinds } from './views';

/** A task as atm serves it: with its kind */
const task = (id: string, fields: Partial<Task>): Task =>
  withKinds([{ id, name: id, type: 'feature', description: '', status: 'todo', ...fields } as Task])[0];

// The project's components as atm serves them with its vocabulary (ATM-718: atm-lib reads meta.components)
const components: Component[] = [
  { id: 'dashboard', label: 'Dashboard', paths: ['dashboard/**'], review: 'async' },
  { id: 'api', label: 'REST API', paths: ['server.py'] },
  { id: 'cli', label: 'cli', paths: [] },
];

describe('components (T-323)', () => {
  it('labels by ID; an unknown ID shows as itself', () => {
    expect(componentLabel(components, 'api')).toBe('REST API');
    expect(componentLabel(components, 'gone')).toBe('gone');
  });

  it("a task's components in the project's order, unknown ones after, alphabetically", () => {
    expect(taskComponents(task('T-1', { components: ['zeta', 'cli', 'alpha', 'dashboard'] }), components)).toEqual([
      'dashboard',
      'cli',
      'alpha',
      'zeta',
    ]);
    expect(taskComponents(task('T-2', {}), components)).toEqual([]);
    expect(['cli', 'api', 'dashboard'].sort(componentOrder(components))).toEqual(['dashboard', 'api', 'cli']);
  });
});

describe('componentProgress (T-323)', () => {
  it("per component: done of the not-skipped, open, in review, blocked; several count in each; 'No component' last", () => {
    const rows = componentProgress(
      [
        task('T-1', { components: ['cli', 'dashboard'], status: 'done' }),
        task('T-2', { components: ['dashboard'], status: 'in_review' }),
        task('T-3', { components: ['dashboard'], status: 'blocked' }),
        task('T-4', { components: ['dashboard'], status: 'skipped' }),
        task('T-5', { components: ['dashboard'], status: 'in_progress' }),
        task('T-6', {}),
      ],
      components,
    );
    expect(rows).toEqual([
      { id: 'dashboard', label: 'Dashboard', total: 4, done: 1, skipped: 1, open: 1, inReview: 1, blocked: 1 },
      { id: 'cli', label: 'cli', total: 1, done: 1, skipped: 0, open: 0, inReview: 0, blocked: 0 },
      { id: '', label: 'No component', total: 1, done: 0, skipped: 0, open: 1, inReview: 0, blocked: 0 },
    ]);
  });
});
