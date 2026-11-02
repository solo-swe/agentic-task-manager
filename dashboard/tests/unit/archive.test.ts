import { describe, expect, it } from 'vitest';
import { archiveNotice } from '../../src/lib/archive';

// Whether to archive is atm's rule (atm-cli's archive_hint, tested with atm-cli); the dashboard words its hint (T-372)
const hint = (bytes: number, archivable: number) => ({ bytes, finished: archivable, archivable, auto: true, text: '' });

describe('archiveNotice', () => {
  it('words atm’s hint: the file’s size, and how many tasks `atm archive` would move', () => {
    expect(archiveNotice(hint(2048, 4))).toBe(
      'tasks.json is 2 KB: `atm archive` would move 4 finished tasks to tasks-archive.json',
    );
    expect(archiveNotice(hint(3 * 1_048_576, 1))).toBe(
      'tasks.json is 3.0 MB: `atm archive` would move 1 finished task to tasks-archive.json',
    );
  });

  it('says nothing without a hint, or with nothing to move', () => {
    expect(archiveNotice(null)).toBeNull();
    expect(archiveNotice(undefined)).toBeNull();
    expect(archiveNotice(hint(2048, 0))).toBeNull();
  });
});
