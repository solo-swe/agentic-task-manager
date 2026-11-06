import { describe, expect, it } from 'vitest';
import { ACTIVE, BACKLOG } from '../../src/lib/filters';
import { sprintParamOf, sprintParamValue, withSprintParam } from '../../src/lib/sprint-param';
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
    expect(sprintParamOf('/projects/p1?sprint=foundations,Backlog', model)).toEqual(['Foundations', BACKLOG]);
    expect(sprintParamOf('/projects/p1?sprint=Ada%20Lovelace', model)).toEqual(['Ada Lovelace']);
    expect(sprintParamOf('/projects/p1?sprint=Renamed', model)).toEqual([]);
  });

  it('writes the selection, commas readable, keeping the other parameters; null removes it', () => {
    expect(sprintParamValue([])).toBe('all');
    expect(sprintParamValue([ACTIVE, BACKLOG, 'Foundations'])).toBe('active,backlog,Foundations');
    expect(withSprintParam('/projects/p1/list', ['Ada Lovelace', BACKLOG])).toBe(
      '/projects/p1/list?sprint=Ada%20Lovelace,backlog',
    );
    expect(withSprintParam('/projects/p1/list?sprint=active', null)).toBe('/projects/p1/list');
  });
});
