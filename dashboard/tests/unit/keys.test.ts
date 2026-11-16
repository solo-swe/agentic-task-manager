import { describe, expect, it } from 'vitest';
import {
  compareTasks,
  findTask,
  isProvisional,
  isUuid,
  keyOf,
  parseRef,
  PROVISIONAL_TIP,
  refView,
  shortId,
  taskLabel,
} from '../../src/lib/keys';
import type { Task } from '../../src/lib/types';

const UUID_A = '0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b';
const UUID_B = '0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5c';
const UUID_C = '0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5d';

function task(fields: Partial<Task> & { id: string }): Task {
  return { name: fields.id, type: 'feature', status: 'todo', phase: 1, description: '', ...fields } as Task;
}

const a = task({ id: UUID_A, key: 'ATM-2', number: 2, key_final: true, name: 'Two' });
const b = task({ id: UUID_B, key: 'ATM-10', number: 10, key_final: false, name: 'Ten', aliases: ['ATM-7'] });
const c = task({ id: UUID_C, key: 'ATM-3', number: 3, key_final: true, name: 'Three' });
const META = { key: 'ATM', key_history: ['T'] };
const maps = (...tasks: Task[]) => [new Map(tasks.map((t) => [t.id, t]))];

describe('taskLabel and isProvisional', () => {
  it('shows the key when there is one, else the ID', () => {
    expect(taskLabel(a)).toBe('ATM-2');
    expect(taskLabel(task({ id: 'T-001' }))).toBe('T-001');
    expect(taskLabel({ id: 'T-1', key: '' })).toBe('T-1');
  });

  it('calls a key provisional until it is final; an ID never', () => {
    expect(isProvisional(a)).toBe(false);
    expect(isProvisional(b)).toBe(true);
    expect(isProvisional({ key: 'ATM-4' })).toBe(true);
    expect(isProvisional({ key_final: false })).toBe(false);
  });

  it('says why a provisional key may change', () => {
    expect(PROVISIONAL_TIP).toBe('Provisional until merged into main: the number may change if another branch took it');
  });

  it('spreads a key only when there is one', () => {
    expect(keyOf(a)).toEqual({ key: 'ATM-2' });
    expect(keyOf(task({ id: 'T-1' }))).toEqual({});
  });
});

describe('compareTasks', () => {
  it('orders by number with task keys, not by UUID', () => {
    expect([b, c, a].sort(compareTasks).map(taskLabel)).toEqual(['ATM-2', 'ATM-3', 'ATM-10']);
  });

  it('orders IDs naturally before task keys', () => {
    const ids = ['T-10', 'T-2', 'T-1'].map((id) => task({ id }));
    expect(ids.sort(compareTasks).map((t) => t.id)).toEqual(['T-1', 'T-2', 'T-10']);
  });
});

describe('findTask', () => {
  const tasks = maps(a, b, c);

  it('finds a task by its UUID, in any case', () => {
    expect(findTask(UUID_A, tasks, META)).toBe(a);
    expect(findTask(UUID_A.toUpperCase(), tasks, META)).toBe(a);
  });

  it('finds a task by its key, in any case', () => {
    expect(findTask('ATM-10', tasks, META)).toBe(b);
    expect(findTask('atm-3', tasks, META)).toBe(c);
    expect(findTask(' ATM-2 ', tasks, META)).toBe(a);
  });

  it('finds a task by a key with an earlier prefix', () => {
    expect(findTask('T-3', tasks, META)).toBe(c);
    expect(findTask('t-10', tasks, META)).toBe(b);
  });

  it('finds a task by its alias, and a current key wins over it', () => {
    expect(findTask('ATM-7', tasks, META)).toBe(b);
    const seven = task({ id: '0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5e', key: 'ATM-7', number: 7 });
    expect(findTask('ATM-7', maps(a, b, seven), META)).toBe(seven);
  });

  it('finds nothing for another prefix, an unknown number or nothing', () => {
    expect(findTask('LFB-2', tasks, META)).toBeUndefined();
    expect(findTask('ATM-99', tasks, META)).toBeUndefined();
    expect(findTask('', tasks, META)).toBeUndefined();
  });

  it('looks in the archive after the working tasks', () => {
    const old = task({ id: '0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a60', key: 'ATM-1', number: 1, key_final: true });
    expect(findTask('ATM-1', [new Map([[a.id, a]]), new Map([[old.id, old]])], META)).toBe(old);
  });

  it('matches an ID exactly before task keys', () => {
    const legacy = maps(task({ id: 'T-001' }), task({ id: 'T-002' }));
    expect(findTask('T-001', legacy, undefined)?.id).toBe('T-001');
    expect(findTask('t-001', legacy, undefined)).toBeUndefined();
    expect(findTask('T-1', legacy, {})).toBeUndefined();
  });
});

describe('references', () => {
  it('reads another project’s reference, and this project’s as it is', () => {
    expect(parseRef(`3F2A9C1E:${UUID_A}`)).toEqual({ project: '3f2a9c1e', id: UUID_A });
    expect(parseRef(UUID_A)).toEqual({ project: null, id: UUID_A });
    expect(parseRef('T-001')).toEqual({ project: null, id: 'T-001' });
  });

  it('shortens a UUID only', () => {
    expect(isUuid(UUID_A)).toBe(true);
    expect(shortId(UUID_A)).toBe('0190a1b2');
    expect(shortId('T-001')).toBe('T-001');
  });

  const own = (id: string) => [a, b, c].find((t) => t.id === id);

  it('shows this project’s task by its key, provisional or not', () => {
    expect(refView(UUID_A, own)).toEqual({ label: 'ATM-2', provisional: false, task: a });
    expect(refView(UUID_B, own)).toMatchObject({ label: 'ATM-10', provisional: true });
    expect(refView('T-009', own)).toEqual({ label: 'T-009', provisional: false });
    expect(refView('0190a1b2-0000-7000-8000-000000000000', own).label).toBe('0190a1b2');
  });

  it('shows another project’s task by its key once its tasks are loaded', () => {
    const lfb = task({ id: UUID_C, key: 'LFB-5', number: 5, key_final: false, name: 'Library' });
    const view = refView(`3f2a9c1e:${UUID_C}`, own, [{ id: '3f2a9c1e', name: 'Lib', tasks: [lfb] }]);
    expect(view).toEqual({
      label: 'LFB-5',
      provisional: true,
      task: lfb,
      project: '3f2a9c1e',
      tip: 'Library (a task of Lib)',
    });
  });

  it('shows another project’s task in short when it isn’t loaded, and says so', () => {
    expect(refView(`3f2a9c1e:${UUID_C}`, own, [{ id: '3f2a9c1e', name: 'Lib' }])).toEqual({
      label: 'Lib:0190a1b2',
      provisional: false,
      project: '3f2a9c1e',
      tip: "A task of another project (Lib) that isn't loaded here",
    });
    expect(refView(`3f2a9c1e:${UUID_C}`, own)).toMatchObject({
      label: '3f2a9c1e:0190a1b2',
      tip: "A task of another project (3f2a9c1e) that isn't loaded here",
    });
  });
});
