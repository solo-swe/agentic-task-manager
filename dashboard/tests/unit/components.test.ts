import { describe, expect, it } from 'vitest';
import { componentProgress } from '../../src/lib/component-progress';
import { componentLabel, componentOrder, projectComponents, taskComponents } from '../../src/lib/components';
import type { Task, TasksFile } from '../../src/lib/types';
import { DEFAULT_VOCABULARY } from '../../src/lib/vocab';

const task = (id: string, fields: Partial<Task>): Task =>
  ({ id, name: id, type: 'feature', description: '', status: 'todo', ...fields }) as Task;

const meta = {
  components: [
    { id: 'dashboard', label: 'Dashboard', paths: ['dashboard/**'], review: 'async' },
    { id: 'api', label: 'REST API', paths: ['server.py', 7] },
    { id: 'cli' },
    { label: 'no id' },
    'not an object',
  ],
} as unknown as TasksFile['meta'];
const components = projectComponents(meta);

describe('projectComponents (T-323)', () => {
  it('read leniently: entries without an ID left out, the label defaults to the ID, paths kept if strings', () => {
    expect(components).toEqual([
      { id: 'dashboard', label: 'Dashboard', paths: ['dashboard/**'], review: 'async' },
      { id: 'api', label: 'REST API', paths: ['server.py'] },
      { id: 'cli', label: 'cli', paths: [] },
    ]);
    expect(projectComponents(undefined)).toEqual([]);
    expect(projectComponents({ components: 'x' } as unknown as TasksFile['meta'])).toEqual([]);
  });

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
      DEFAULT_VOCABULARY,
    );
    expect(rows).toEqual([
      { id: 'dashboard', label: 'Dashboard', total: 4, done: 1, skipped: 1, open: 1, inReview: 1, blocked: 1 },
      { id: 'cli', label: 'cli', total: 1, done: 1, skipped: 0, open: 0, inReview: 0, blocked: 0 },
      { id: '', label: 'No component', total: 1, done: 0, skipped: 0, open: 1, inReview: 0, blocked: 0 },
    ]);
  });
});
