import { describe, expect, it } from 'vitest';
import { ACTIVE, BACKLOG } from '../../src/lib/filters';
import { parseSprintHash, sprintHashValue, withSprintHash } from '../../src/lib/sprint-hash';
import { sprintModel } from '../../src/lib/sprints';
import type { Task, TasksFile } from '../../src/lib/types';
import { resolveVocabulary } from '../../src/lib/vocab';
import sprintsFile from '../fixtures/sprints.json';

const file = sprintsFile as TasksFile;
const model = sprintModel(file.meta, file.tasks as Task[], resolveVocabulary(file.meta));

describe('the Sprint filter in the URL (T-323)', () => {
  it('reads active, backlog, all and names (any case); unknown names are dropped; none without the parameter', () => {
    expect(parseSprintHash('#view=kanban', model)).toBeNull();
    expect(parseSprintHash('#sprint=active', model)).toEqual([ACTIVE]);
    expect(parseSprintHash('#sprint=all', model)).toEqual([]);
    expect(parseSprintHash('#sprint=foundations,Backlog', model)).toEqual(['Foundations', BACKLOG]);
    expect(parseSprintHash('#sprint=Ada%20Lovelace', model)).toEqual(['Ada Lovelace']);
    expect(parseSprintHash('#sprint=Renamed', model)).toEqual([]);
  });

  it('writes the selection, commas readable, keeping the other parameters; null removes it', () => {
    expect(sprintHashValue([])).toBe('all');
    expect(sprintHashValue([ACTIVE, BACKLOG, 'Foundations'])).toBe('active,backlog,Foundations');
    expect(withSprintHash('#view=list', ['Ada Lovelace', BACKLOG])).toBe('#view=list&sprint=Ada%20Lovelace,backlog');
    expect(withSprintHash('#view=list&sprint=active', null)).toBe('#view=list');
  });
});
