import { describe, expect, it } from 'vitest';
import { BACKLOG } from '../../src/lib/filters';
import { groupList } from '../../src/lib/list-groups';
import { sprintModel } from '../../src/lib/sprints';
import type { Task, TasksFile } from '../../src/lib/types';
import { resolveVocabulary } from '../../src/lib/vocab';
import sprintsFile from '../fixtures/sprints.json';

// fixtures/sprints.json: Foundations closed (T-001), Ada Lovelace active (T-002–T-004), Alan Turing planned (none),
// T-005 in the backlog
const file = sprintsFile as TasksFile;
const tasks = file.tasks as Task[];
const vocab = resolveVocabulary(file.meta);
const model = sprintModel(file.meta, tasks, vocab);
const shape = (groups: ReturnType<typeof groupList>) => groups.map((g) => [g.key, g.tasks.map((t) => t.id)]);

describe('groupList (T-323)', () => {
  it('by sprint: the active one, the planned ones in order, the backlog, then the closed ones, most recent first', () => {
    const more = sprintModel(
      {
        sprints: [
          { id: 'Old', state: 'closed' },
          { id: 'Newer', state: 'closed' },
          { id: 'Now', state: 'active' },
          { id: 'Next', state: 'planned' },
          { id: 'Later', state: 'planned' },
        ],
      },
      [],
      vocab,
    );
    const at = (sprint: string | undefined, id: string) => ({ id, sprint, status: 'todo', type: 'feature' }) as Task;
    const list = [at('Old', 'T-1'), at('Later', 'T-2'), at(undefined, 'T-3'), at('Newer', 'T-4'), at('Next', 'T-5')];
    list.push(at('Now', 'T-6'));
    expect(shape(groupList(list, 'sprint', more, vocab))).toEqual([
      ['Now', ['T-6']],
      ['Next', ['T-5']],
      ['Later', ['T-2']],
      [BACKLOG, ['T-3']],
      ['Newer', ['T-4']],
      ['Old', ['T-1']],
    ]);
  });

  it('by sprint: only sprints with tasks shown; each group has its sprint, the backlog none', () => {
    const groups = groupList(tasks, 'sprint', model, vocab);
    expect(shape(groups)).toEqual([
      ['Ada Lovelace', ['T-002', 'T-003', 'T-004']],
      [BACKLOG, ['T-005']],
      ['Foundations', ['T-001']],
    ]);
    expect(groups[0]).toMatchObject({ label: 'Ada Lovelace', sprint: { state: 'active' } });
    expect(groups[1]).toMatchObject({ label: 'Backlog' });
    expect(groups[1].sprint).toBeUndefined();
  });

  it("by label: meta.labels' text and order; a task with several is in each; those with none last", () => {
    const labelled = [{ ...tasks[0], labels: ['ux', 'api'] }, { ...tasks[1], labels: ['api'] }, tasks[2]];
    const v = resolveVocabulary({ labels: [{ id: 'ux', label: 'UX' }] });
    const groups = groupList(labelled, 'label', model, v);
    expect(shape(groups)).toEqual([
      ['ux', ['T-001']],
      ['api', ['T-001', 'T-002']],
      ['', ['T-003']],
    ]);
    expect(groups.map((g) => g.label)).toEqual(['UX', 'api', 'No label']);
  });

  it('by component: alphabetically, those with none last; none: one group', () => {
    const parts = [{ ...tasks[0], components: ['ui', 'api'] }, tasks[1]];
    expect(shape(groupList(parts, 'component', model, vocab))).toEqual([
      ['api', ['T-001']],
      ['ui', ['T-001']],
      ['', ['T-002']],
    ]);
    expect(groupList(parts, 'component', model, vocab).at(-1)?.label).toBe('No component');
    expect(shape(groupList(parts, 'none', model, vocab))).toEqual([['all', ['T-001', 'T-002']]]);
  });
});
