import { describe, expect, it } from 'vitest';
import { ACTIVE, BACKLOG } from '../../src/lib/filters';
import { sprintParamOf, sprintParamValues, withSprintParam } from '../../src/lib/sprint-param';
import { sprintModel } from '../../src/lib/sprints';
import type { Task, TasksFile } from '../../src/lib/types';
import { resolveVocabulary } from '../../src/lib/vocab';
import sprintsFile from '../fixtures/sprints.json';

const file = sprintsFile as TasksFile;
const model = sprintModel(file.meta, file.tasks as Task[], resolveVocabulary(file.meta));

describe('the Sprint filter in the address (T-323)', () => {
  it('reads active, backlog, all and names (any case); unknown names are dropped; none without the parameter', () => {
    expect(sprintParamOf('/projects/p1/kanban', model)).toBeNull();
    expect(sprintParamOf('/projects/p1?sprint=active', model)).toEqual([ACTIVE]);
    expect(sprintParamOf('/projects/p1?sprint=all', model)).toEqual([]);
    expect(sprintParamOf('/projects/p1?sprint=foundations&sprint=Backlog', model)).toEqual(['Foundations', BACKLOG]);
    expect(sprintParamOf('/projects/p1?sprint=Ada%20Lovelace', model)).toEqual(['Ada Lovelace']);
    expect(sprintParamOf('/projects/p1?sprint=Renamed', model)).toEqual([]);
  });

  it('reads back a sprint whose name holds a comma, alone or with others, as it writes it (T-494)', () => {
    const withComma = sprintModel(
      { ...file.meta, sprints: [{ id: 'Library, gallery and blog' }, { id: 'Library' }, { id: 'Next' }] },
      [],
      resolveVocabulary(file.meta),
    );
    for (const selection of [
      ['Library, gallery and blog'],
      ['Library, gallery and blog', BACKLOG],
      [ACTIVE, 'Library', 'Library, gallery and blog', 'Next'],
    ]) {
      const address = withSprintParam('/projects/p1/kanban', selection);
      expect(sprintParamOf(address, withComma), address).toEqual(selection);
    }
    // One value is one sprint: a comma never splits it
    expect(sprintParamOf('/projects/p1?sprint=Library,Next', withComma)).toEqual([]);
  });

  it('writes one parameter per value, keeping the other parameters; null removes them', () => {
    expect(sprintParamValues([])).toEqual(['all']);
    expect(sprintParamValues([ACTIVE, BACKLOG, 'Foundations'])).toEqual(['active', 'backlog', 'Foundations']);
    expect(withSprintParam('/projects/p1/list', ['Ada Lovelace', BACKLOG])).toBe(
      '/projects/p1/list?sprint=Ada%20Lovelace&sprint=backlog',
    );
    expect(withSprintParam('/projects/p1/list?sprint=active&sprint=backlog&about', ['Next'])).toBe(
      '/projects/p1/list?about&sprint=Next',
    );
    expect(withSprintParam('/projects/p1/list?sprint=active', null)).toBe('/projects/p1/list');
  });
});
