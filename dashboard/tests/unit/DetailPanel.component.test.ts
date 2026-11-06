import { fireEvent, render } from '@testing-library/svelte';
import { flushSync } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import DetailPanel from '../../src/lib/details/DetailPanel.svelte';
import { compmdSettled } from './compmd.helpers';
import { dashboard } from '../../src/lib/state.svelte';
import type { TasksFile } from '../../src/lib/types';
import archiveFile from '../fixtures/archive.json';
import sprints from '../fixtures/sprints.json';
import standard from '../fixtures/standard.json';
import { loadIntoDashboard } from './components.helpers';

// Task text renders with the real CompMD (T-284), as the served module has it
vi.mock('../../src/lib/compmd', async (actual) => ({
  ...(await actual()),
  loadCompmd: (await import('./compmd.helpers')).realCompmd,
}));

// The archive's meta (archived_at) isn't part of tasks.json's
const archive = archiveFile as unknown as TasksFile;

function open(fields: Record<string, unknown>, id = 'T-005') {
  const tasks = standard.tasks.map((t) => (t.id === id ? { ...t, ...fields } : t));
  loadIntoDashboard({ ...standard, tasks });
  const result = render(DetailPanel);
  dashboard.showDetail(id);
  flushSync();
  return result.container;
}

describe('DetailPanel', () => {
  beforeEach(() => loadIntoDashboard(standard));

  // The section rail (T-280): one entry per group the task has, in the order of the groups below it
  const railGroups = (container: HTMLElement) =>
    [...container.querySelectorAll<HTMLElement>('#detail-rail .detail-rail-item')].map((b) => b.dataset.group);
  const bodyGroups = (container: HTMLElement) =>
    [...container.querySelectorAll<HTMLElement>('#detail-body [data-group]')].map((g) => g.dataset.group);

  it('shows its sprint and release, and the sprints it was carried through, flagged from twice (SR-7.6, SR-4.5)', () => {
    loadIntoDashboard(sprints as TasksFile);
    const { container } = render(DetailPanel);
    dashboard.showDetail('T-003');
    flushSync();
    expect(container.querySelector('#detail-sprint')).toHaveTextContent('Ada Lovelace');
    expect(container.querySelector('#detail-release')).toHaveTextContent('release 1.0');
    const items = [...container.querySelectorAll('#detail-sprint-history li strong')].map((s) => s.textContent);
    expect(items).toEqual(['Foundations', 'Ada Lovelace']);
    // Carried once: not flagged yet
    expect(container.querySelector('.detail-carried')).toBeNull();

    dashboard.showDetail('T-005');
    flushSync();
    expect(container.querySelector('#detail-sprint')).toHaveTextContent('Backlog');
    expect(container.querySelector('#detail-release')).toBeNull();
    expect(container.querySelector('#detail-sprint-history')).toBeNull();
  });

  it('shows the review mode and state, the sprint review, with you and components (SR-7.6)', () => {
    const container = open({
      status: 'in_review',
      review: 'blocker',
      review_state: 'changes_requested',
      with: 'user',
      components: ['ui', 'api'],
      sprint_review: 'pending',
      // The review mode is atm's to say (T-387)
      derived: {
        kind: 'review',
        ready: false,
        waiting_for: [],
        priority: 'P2',
        archived: false,
        effective_review: 'blocker',
      },
    });
    expect(container.querySelector('#detail-review-mode')).toHaveTextContent('review: blocker');
    expect(container.querySelector('.task-meta [data-review-state]')).toHaveTextContent('Changes requested');
    expect(container.querySelector('#detail-sprint-review')).toHaveTextContent('waits for the sprint review');
    expect(container.querySelector('#detail-with-user')).toHaveTextContent('with you');
    expect([...container.querySelectorAll('.task-meta .component-chip')].map((c) => c.textContent)).toEqual([
      'review: blocker',
      // Components the project doesn't define, alphabetically (T-323)
      'api',
      'ui',
    ]);
    const plain = open({});
    expect(plain.querySelector('#detail-review-mode')).toBeNull();
    expect(plain.querySelector('#detail-with-user')).toBeNull();
  });

  it('shows how it ended when not done, what it duplicates or is duplicated by, and related tasks both ways (SR-7.6)', () => {
    const fields: Record<string, Record<string, unknown>> = {
      'T-001': { related: ['T-002'] },
      'T-003': { status: 'skipped', resolution: 'duplicate', duplicate_of: 'T-002' },
      'T-004': { related: ['T-002', 'T-099'] },
    };
    loadIntoDashboard({ ...standard, tasks: standard.tasks.map((t) => ({ ...t, ...fields[t.id] })) });
    const { container } = render(DetailPanel);
    const ids = (selector: string) =>
      [...container.querySelectorAll(`${selector} .detail-dep-item strong`)].map((s) => s.textContent);

    dashboard.showDetail('T-002');
    flushSync();
    expect(ids('#detail-related')).toEqual(['T-001', 'T-004']);
    expect(ids('#detail-duplicated-by')).toEqual(['T-003']);
    expect(container.querySelector('#detail-resolution')).toBeNull(); // done: the default
    expect(railGroups(container)).toContain('dependencies');

    dashboard.showDetail('T-003');
    flushSync();
    expect(container.querySelector('#detail-resolution')).toHaveTextContent('duplicate');
    expect(ids('#detail-duplicate-of')).toEqual(['T-002']);

    // A related task that doesn't exist is shown, not linked; a link opens the other task
    dashboard.showDetail('T-004');
    flushSync();
    expect(container.querySelector('#detail-related [data-archived="T-099"]')).toHaveTextContent('not found');
    container.querySelector<HTMLElement>('#detail-related .detail-dep-item:not(.archived)')!.click();
    flushSync();
    expect(dashboard.detailId).toBe('T-002');
  });

  it('a skipped task shows won’t do, its default resolution', () => {
    const container = open({ status: 'skipped' });
    expect(container.querySelector('#detail-resolution')).toHaveTextContent("won't do");
  });

  it('a task carried twice or more is flagged', () => {
    const data = sprints as TasksFile;
    const tasks = (data.tasks ?? []).map((t) =>
      t.id === 'T-003'
        ? {
            ...t,
            sprint_history: [
              { sprint: 'Old', from: '2026-03-01T09:00:00Z', to: '2026-03-20T09:00:00Z' },
              { sprint: 'Foundations', from: '2026-04-01T09:00:00Z', to: '2026-04-05T18:00:00Z' },
              { sprint: 'Ada Lovelace', from: '2026-04-05T18:00:00Z' },
            ],
          }
        : t,
    );
    loadIntoDashboard({ ...data, tasks });
    const { container } = render(DetailPanel);
    dashboard.showDetail('T-003');
    flushSync();
    expect(container.querySelector('.detail-carried')).toHaveTextContent('carried 2 times');
  });

  it('has a rail entry per group, in the order of the groups, Notes only with notes', () => {
    const plain = open({ notes: [] });
    expect(railGroups(plain)).not.toContain('notes');
    expect(railGroups(plain)).toEqual(bodyGroups(plain));
    expect(railGroups(plain)[0]).toBe('info');
    expect(railGroups(plain).at(-1)).toBe('ref');

    const noted = open({ notes: [{ at: '2026-04-01T08:00:00Z', kind: 'outcome', text: 'Done' }] });
    expect(railGroups(noted)).toContain('notes');
    expect(railGroups(noted)).toEqual(bodyGroups(noted));
    const labels = [...noted.querySelectorAll('#detail-rail .detail-rail-label')].map((l) => l.textContent);
    expect(labels).toContain('Notes');
  });

  it('lists the notes newest first, with kind, author and time; a legacy last_failure among them', async () => {
    const container = open({
      notes: [
        { at: '2026-04-01T08:00:00Z', by: 'claude:aaaa', kind: 'handoff', text: 'Toolbar done; links left' },
        { at: '2026-04-02T08:00:00Z', by: 'Hakan', kind: 'rejection', text: 'Use the icon set' },
      ],
      last_failure: { at: '2026-04-01T12:00:00Z', reason: 'lint failed' },
    });
    await compmdSettled();
    const items = [...container.querySelectorAll('#detail-notes .detail-note')];
    expect(items.map((n) => (n as HTMLElement).dataset.kind)).toEqual(['rejection', 'failure', 'handoff']);
    expect(items[0].querySelector('.note-kind')).toHaveTextContent('Rejected');
    expect(items[0].querySelector('.note-meta')).toHaveTextContent(/^Hakan · /);
    expect(items[0].querySelector('.note-text')).toHaveTextContent('Use the icon set');
    expect(items[1].querySelector('.note-text')).toHaveTextContent('lint failed');
  });

  it('shows the commit, short, with the full SHA on hover; without a repository address a click says how to set one', async () => {
    const container = open({ commit: 'e4969ae0123456789abcdef' });
    expect(container.querySelector('#detail-commit code')).toHaveTextContent(/^e4969ae$/);
    const ref = container.querySelector('#detail-commit .repo-ref')!;
    expect(ref).toHaveAttribute('title', 'e4969ae0123456789abcdef');
    expect(ref.tagName).toBe('BUTTON');
    await fireEvent.click(ref);
    expect(container.querySelector('#detail-commit .repo-hint')).toHaveTextContent('atm repository set <url>');
  });

  it('links the commit to GitHub when meta.repository is a GitHub repository', () => {
    const tasks = standard.tasks.map((t) => (t.id === 'T-005' ? { ...t, commit: 'e4969ae0123' } : t));
    loadIntoDashboard({ ...standard, meta: { ...standard.meta, repository: 'https://github.com/acme/demo' }, tasks });
    const { container } = render(DetailPanel);
    dashboard.showDetail('T-005');
    flushSync();
    const link = container.querySelector('#detail-commit a.repo-ref')!;
    expect(link).toHaveAttribute('href', 'https://github.com/acme/demo/commit/e4969ae0123');
    expect(link).toHaveAttribute('target', '_blank');
    // Nothing says the commit was pushed: the link may not work yet
    expect(container.querySelector('#detail-commit-note')).toHaveTextContent(
      'The link works once the commit is pushed',
    );
  });

  it('no push warning when the task is in a PR, or without a link', () => {
    const tasks = standard.tasks.map((t) =>
      t.id === 'T-005' ? { ...t, commit: 'e4969ae0123', pr: { number: 3 } } : t,
    );
    loadIntoDashboard({ ...standard, meta: { ...standard.meta, repository: 'https://github.com/acme/demo' }, tasks });
    const { container } = render(DetailPanel);
    dashboard.showDetail('T-005');
    flushSync();
    expect(container.querySelector('#detail-commit a.repo-ref')).not.toBeNull();
    expect(container.querySelector('#detail-commit-note')).toBeNull();
    // Without a repository address there is no link to warn about
    expect(open({ commit: 'e4969ae0123' }).querySelector('#detail-commit-note')).toBeNull();
  });

  it('a blocked task lists its questions and how to answer them', async () => {
    const container = open({
      status: 'blocked',
      waiting_on: 'user',
      questions: ['REST or GraphQL?', 'Which DB?'],
    });
    await compmdSettled();
    const section = container.querySelector('#detail-blocked')!;
    expect(section).toHaveTextContent('Waiting on you · 2 questions');
    expect([...section.querySelectorAll('.block-questions li')].map((li) => li.textContent?.trim())).toEqual([
      'REST or GraphQL?',
      'Which DB?',
    ]);
    expect(section).toHaveTextContent('atm unblock T-005 --answer');
  });

  it('says who created, started and completed it, and who tried before', () => {
    const container = open({
      created_by: 'Hakan',
      started_by: 'claude:cccc',
      completed_by: 'Hakan',
      notes: [{ at: '2026-04-01T08:00:00Z', by: 'claude:aaaa', kind: 'handoff', text: 'half done' }],
    });
    const who = container.querySelector('#detail-actors')!;
    expect(who).toHaveTextContent('Created by Hakan · started by claude:cccc · completed by Hakan');
    expect(who).toHaveTextContent('Earlier attempts: claude:aaaa');
  });

  it('a dependency on an archived task is a stub', () => {
    const tasks = standard.tasks.map((t) => (t.id === 'T-001' ? { ...t, dependencies: ['T-090'] } : t));
    loadIntoDashboard({ ...standard, meta: { ...standard.meta, archived: 3 }, tasks }, archive);
    const { container } = render(DetailPanel);
    dashboard.showDetail('T-001');
    flushSync();
    expect(container.querySelector('.detail-dep-item[data-archived="T-090"]')).toHaveTextContent(
      /T-090\s*· archived, done/,
    );
  });

  it('opens an archived task, marked as archived', () => {
    loadIntoDashboard(standard, archive);
    const { container } = render(DetailPanel);
    dashboard.showDetail('T-091');
    flushSync();
    expect(container.querySelector('#detail-archived')).toHaveTextContent('archived');
  });

  it('renders the description with CompMD: GFM, HTML outside the allowlist kept as text, links in a new tab', async () => {
    const container = open({
      description:
        '## Goal\n\nLog **every** sign-in.\n\n- the IP\n- the time\n\n```\natm show T-005\n```\n\n' +
        '| a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n\n' +
        'See [the spec](https://example.com/spec). <i>not italic</i> <script>bad()</script>',
    });
    await compmdSettled();
    const desc = container.querySelector('#detail-description')!;
    expect(desc.querySelector('h2')).toHaveTextContent('Goal');
    expect(desc.querySelector('strong')).toHaveTextContent('every');
    expect([...desc.querySelectorAll('ul:not(:has(.compmd-task)) > li')].map((li) => li.textContent)).toEqual([
      'the IP',
      'the time',
    ]);
    expect(desc.querySelector('pre code')).toHaveTextContent('atm show T-005');
    expect(desc.querySelectorAll('table td')).toHaveLength(2);
    expect(desc.querySelector('.compmd-task input[type=checkbox]')).toBeChecked();
    expect(desc.querySelector('a')).toHaveAttribute('target', '_blank');
    expect(desc.querySelector('i')).toBeNull();
    expect(desc.querySelector('script')).toBeNull();
    expect(desc).toHaveTextContent('<i>not italic</i>');
  });

  it('lists use cases and requirements one a line; one written as a link is a link, a bare ID text (T-403)', async () => {
    const container = open({
      use_cases: [],
      requirements: ['[the phrase that defines it](docs/x.md#:~:text=SR%2D2.1)', 'SR-9'],
    });
    await compmdSettled();
    const entries = (label: string) => {
      const section = [...container.querySelectorAll('.detail-section')].find(
        (s) => s.querySelector('.label')?.textContent === label,
      )!;
      return [...section.querySelectorAll('.detail-refs > .detail-ref')];
    };
    expect(entries('Requirements').map((e) => e.textContent?.trim())).toEqual(['the phrase that defines it', 'SR-9']);
    expect(entries('Requirements')[0].querySelector('a')).toHaveTextContent('the phrase that defines it');
    expect(entries('Requirements')[1].querySelector('a')).toBeNull();
    expect(entries('Use Cases')).toEqual([]);
    expect(container.querySelector('.req-badge, .uc-badge')).toBeNull();
  });

  it('the task’s own data is md.data.task (T-284)', async () => {
    // Its derived values too, which come with the v1 route's tasks (lib/compmd's tests); these come from a fixture
    const container = open({ description: 'This is {{md.data.task.id}}, {{md.data.task.status}}.', status: 'todo' });
    await compmdSettled();
    expect(container.querySelector('#detail-description')).toHaveTextContent('This is T-005, todo.');
  });

  it('a # link scrolls to the heading in the same text; a link atm wouldn’t follow isn’t one', async () => {
    const container = open({ description: '[jump](#setup) [bad](javascript:x)\n\n## Setup\n\nThere.' });
    await compmdSettled();
    const desc = container.querySelector('#detail-description')!;
    const heading = desc.querySelector('h2')!;
    const scrolled = vi.fn();
    heading.scrollIntoView = scrolled;
    await fireEvent.click(desc.querySelector('a[href="#setup"]')!);
    expect(scrolled).toHaveBeenCalled();
    expect(desc.querySelector('a[href^="javascript"]')).toBeNull();
    expect(desc).toHaveTextContent('jump bad');
  });

  it('renders notes, the review note and questions with CompMD, with HTML kept as text (T-219)', async () => {
    const container = open({
      status: 'in_review',
      notes: [
        { at: '2026-04-01T08:00:00Z', by: 'Hakan', kind: 'done', text: 'Ran `npm test`; see [CI](https://ci.example)' },
        { at: '2026-04-02T08:00:00Z', by: 'claude:aaaa', kind: 'review', text: 'Check **the toolbar** <i>x</i>' },
      ],
    });
    await compmdSettled();
    const notes = [...container.querySelectorAll('#detail-notes .note-text')];
    expect(notes.map((n) => n.textContent!.trim())).toEqual(['Check the toolbar <i>x</i>', 'Ran npm test; see CI']);
    expect(notes[0].querySelector('strong')).toHaveTextContent('the toolbar');
    expect(notes[0].querySelector('i')).toBeNull();
    expect(notes[1].querySelector('code')).toHaveTextContent('npm test');
    expect(notes[1].querySelector('a')).toHaveAttribute('href', 'https://ci.example');
    expect(notes[1].querySelector('a')).toHaveAttribute('target', '_blank');
    expect(container.querySelector('#detail-review .review-note strong')).toHaveTextContent('the toolbar');

    const blocked = open({ status: 'blocked', waiting_on: 'user', questions: ['Keep `--force`? [why](javascript:x)'] });
    await compmdSettled();
    const question = blocked.querySelector('.block-questions li')!;
    expect(question.querySelector('code')).toHaveTextContent('--force');
    expect(question.querySelector('a')).toBeNull(); // only links atm would follow
    expect(question).toHaveTextContent('Keep --force? why');
  });

  it('a plain description is one paragraph, as before', async () => {
    const container = open({ description: 'Add the editor.' });
    await compmdSettled();
    const paragraphs = container.querySelectorAll('#detail-description .compmd-page p');
    expect([...paragraphs].map((p) => p.textContent)).toEqual(['Add the editor.']);
  });

  it('has no notes, commit or blocked section when the task has none', () => {
    const container = open({});
    expect(container.querySelector('#detail-actors')).toBeNull();
    expect(container.querySelector('#detail-notes')).toBeNull();
    expect(container.querySelector('#detail-commit')).toBeNull();
    expect(container.querySelector('#detail-blocked')).toBeNull();
  });
});
