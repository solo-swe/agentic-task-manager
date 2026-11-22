import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import TaskCard from '../../src/lib/tasks/TaskCard.svelte';
import { scrollToTask } from '../../src/lib/scroll';
import { dashboard } from '../../src/lib/state.svelte';
import type { Task } from '../../src/lib/types';
import standard from '../fixtures/standard.json';
import { loadIntoDashboard } from './components.helpers';
import { viewed, vocabulary, withKinds } from './views';

vi.mock('../../src/lib/scroll', () => ({ scrollToTask: vi.fn() }));
const navigate = vi.hoisted(() => vi.fn());
vi.mock('../../src/lib/navigation', async (actual) => ({ ...(await actual()), navigate }));

const task = (id: string) => dashboard.taskMap.get(id) as Task;

describe('TaskCard', () => {
  beforeEach(() => {
    loadIntoDashboard(standard);
    vi.mocked(scrollToTask).mockClear();
  });

  it('renders the legacy card structure', () => {
    const { container } = render(TaskCard, { task: task('T-004') });
    const card = container.querySelector('.task-card')!;
    expect(card).toHaveAttribute('id', 'task-T-004');
    expect(card).toHaveAttribute('data-id', 'T-004');
    expect(card).toHaveClass('status-done');
    expect(card).toHaveClass('kind-done');
    // No phase badge (T-323): the sprint, only when the list shows several and doesn't group by them
    expect([...card.querySelectorAll('.task-meta .badge')].map((b) => b.textContent)).toEqual([
      'feature',
      'Done',
      'P2',
    ]);
    expect([...card.querySelectorAll('.uc-badge, .req-badge')].map((b) => b.textContent)).toEqual([
      'UC-001',
      'FR-1.1',
      'FR-1.2',
    ]);
  });

  // ATM-619: a task in review shows its code review, a link that opens it and not the task
  it('in review with a code review: the one atm says it waits on, as a link to its page; a click opens it, not the task', async () => {
    dashboard.projectId = 'p1';
    const show = vi.spyOn(dashboard, 'showDetail');
    // atm says it waits on CR-3, its older review still open, not on its latest link (ATM-717)
    const reviewed = viewed(
      {
        ...task('T-004'),
        status: 'in_review',
        code_reviews: [
          { id: 'u1', ref: 'CR-3' },
          { id: 'u2', ref: 'CR-7' },
        ],
      },
      { kind: 'review', review: { id: 'u1', ref: 'CR-3', status: 'waiting' } },
    );
    const { container } = render(TaskCard, { task: reviewed });
    const links = container.querySelectorAll<HTMLAnchorElement>('.card-review .review-link');
    expect([...links].map((a) => [a.dataset.review, a.getAttribute('href')])).toEqual([
      ['CR-3', '/projects/p1/reviews/CR-3'],
    ]);
    // Its status, as atm says it
    expect(links[0].querySelector('.review-link-status')).toHaveTextContent('Waiting for review');
    await fireEvent.click(links[0]);
    expect(navigate).toHaveBeenCalledWith('/projects/p1/reviews/CR-3');
    expect(show).not.toHaveBeenCalled();
    dashboard.projectId = null;
  });

  it('no link without a code review, nor once the task is out of review', () => {
    dashboard.projectId = 'p1';
    const inReview = render(TaskCard, { task: { ...task('T-004'), status: 'in_review' } });
    expect(inReview.container.querySelector('.card-review')).toBeNull();
    const done = render(TaskCard, { task: { ...task('T-004'), code_reviews: [{ id: 'u2', ref: 'CR-7' }] } });
    expect(done.container.querySelector('.card-review')).toBeNull();
    dashboard.projectId = null;
  });

  it('dependency links show the target name and scroll to it', async () => {
    render(TaskCard, { task: task('T-004') });
    const link = screen.getByText('T-002');
    expect(link).toHaveAttribute('title', 'Write contributing guide');
    await fireEvent.click(link);
    expect(scrollToTask).toHaveBeenCalledWith('T-002');
  });

  it('clicking the name opens the detail panel', async () => {
    render(TaskCard, { task: task('T-004') });
    await fireEvent.click(screen.getByText('User login'));
    expect([dashboard.detailId, dashboard.detailOpen]).toEqual(['T-004', true]);
  });

  it('escapes task text', () => {
    const t = { ...task('T-001'), name: '<b>Bold</b>' };
    const { container } = render(TaskCard, { task: t });
    expect(container.querySelector('.task-name')!.textContent).toBe('<b>Bold</b>');
    expect(container.querySelector('.task-name b')).toBeNull();
  });

  it("labels and styles the status from the project's vocabulary, by kind", () => {
    const vocab = vocabulary({
      statuses: [
        { id: 'todo', label: 'Backlog', kind: 'todo' },
        { id: 'in_progress', label: 'Doing', kind: 'active' },
        { id: 'done', label: 'Shipped', kind: 'done' },
        { id: 'awaiting_user', label: 'Awaiting you', kind: 'review' },
      ],
    });
    loadIntoDashboard(standard, undefined, vocab);
    const [awaiting] = withKinds([{ ...task('T-001'), status: 'awaiting_user' }], vocab);
    const { container } = render(TaskCard, { task: awaiting });
    const card = container.querySelector('.task-card')!;
    expect(card).toHaveClass('kind-review');
    expect(card.querySelectorAll('.task-meta .badge')[1].textContent).toBe('Awaiting you');
  });

  it('names its sprint when asked (showSprint), the backlog muted (T-323)', () => {
    const { container } = render(TaskCard, { task: task('T-004'), showSprint: true });
    expect(container.querySelector('.task-card-sprint')).toHaveTextContent('Phase 1');
    const inBacklog = { ...task('T-004'), phase: undefined } as unknown as Task;
    const { container: backlog } = render(TaskCard, { task: inBacklog, showSprint: true });
    expect(backlog.querySelector('.task-card-sprint')).toHaveTextContent('Backlog');
  });

  it("shows the task's priority, or the default for a task without one", () => {
    const { container } = render(TaskCard, { task: { ...task('T-001'), priority: 'P0' } });
    const badge = container.querySelector('.priority-badge')!;
    expect(badge).toHaveTextContent('P0');
    expect(badge).toHaveAttribute('title', 'P0 Critical priority');
    // The badge's own style: jsdom 31 resolves var() in the computed style, and the test page defines no --red
    expect((badge as HTMLElement).style.color).toBe('var(--red)');
    const plain = render(TaskCard, { task: task('T-002') }).container.querySelector('.priority-badge')!;
    expect(plain).toHaveAttribute('title', 'P2 Medium priority (the default)');
  });

  it('shows how long a task in review has waited, with the review note on hover', () => {
    dashboard.loadedAt = Date.parse('2026-04-03T13:30:00Z');
    const fields = {
      ...task('T-005'),
      status: 'in_review',
      review_requested_at: '2026-04-03T11:00:00Z',
      notes: [{ at: '2026-04-03T11:00:00Z', kind: 'review', text: 'Check the editor toolbar' }],
    };
    const reviewed = viewed(fields, { kind: 'review', waiting: { on: 'reviewer', since: '2026-04-03T11:00:00Z' } });
    const { container } = render(TaskCard, { task: reviewed });
    const line = container.querySelector('.review-wait')!;
    expect(line).toHaveTextContent('Waiting for review · 2 h 30 m');
    expect(line).toHaveAttribute('title', 'Check the editor toolbar');
    expect(render(TaskCard, { task: task('T-005') }).container.querySelector('.review-wait')).toBeNull();
  });

  // ATM-711's card: approved, a task in review is back with its agent, and its card says so, not "Waiting for review"
  it('a task in review that atm says is back with its agent says so, its chip and its line agreeing', () => {
    dashboard.loadedAt = Date.parse('2026-04-03T13:30:00Z');
    const fields = { ...task('T-005'), status: 'in_review', review_state: 'approved' };
    const approved = viewed(fields, { kind: 'review', waiting: { on: 'agent', since: '2026-04-03T12:00:00Z' } });
    const line = render(TaskCard, { task: approved }).container.querySelector('.review-wait')!;
    expect(line).toHaveTextContent('Back with its agent · 1 h 30 m');
    expect(line).not.toHaveTextContent('Waiting for review');
  });

  it('a blocked task says who it waits on, for how long, and how many questions (listed on hover)', () => {
    dashboard.loadedAt = Date.parse('2026-04-03T13:00:00Z');
    const questions = ['REST or GraphQL?', 'Which DB?'];
    const fields = {
      ...task('T-008'),
      status: 'blocked',
      waiting_on: 'user',
      blocked_at: '2026-04-03T12:00:00Z',
      questions,
    };
    const blocked = viewed(fields, {
      kind: 'blocked',
      waiting: { on: 'user', since: '2026-04-03T12:00:00Z', questions },
    });
    const { container } = render(TaskCard, { task: blocked });
    const line = container.querySelector('.block-wait')!;
    expect(line).toHaveTextContent('Waiting on you · 1 h · 2 questions');
    expect(line).toHaveClass('on-user');
    expect(line).toHaveAttribute('title', 'REST or GraphQL?\nWhich DB?');
  });

  it("shows the task's labels as chips, in meta.labels' order and text", () => {
    loadIntoDashboard(standard, undefined, vocabulary({ labels: [{ id: 'ux', label: 'UX' }] }));
    const { container } = render(TaskCard, { task: { ...task('T-001'), labels: ['spike-2', 'ux'] } });
    expect([...container.querySelectorAll('.label-chip')].map((c) => c.textContent)).toEqual(['UX', 'spike-2']);
    expect(container.querySelector('.label-chip[data-label="ux"]')).toHaveAttribute('title', 'Label: ux');
  });

  it("shows the task's components by their labels, in the project's order (T-323)", () => {
    loadIntoDashboard(
      standard,
      undefined,
      vocabulary({
        components: [
          { id: 'web', label: 'Web app', paths: [] },
          { id: 'api', label: 'REST API', paths: [] },
        ],
      }),
    );
    const { container } = render(TaskCard, { task: { ...task('T-001'), components: ['api', 'web', 'gone'] } });
    expect([...container.querySelectorAll('.component-chip')].map((c) => c.textContent)).toEqual([
      'Web app',
      'REST API',
      'gone',
    ]);
    expect(container.querySelector('.component-chip[data-component="api"]')).toHaveAttribute('title', 'Component: api');
  });

  it('a task open in a closed sprint: its sprint badge is marked (T-323)', () => {
    loadIntoDashboard({
      meta: { sprints: [{ id: 'Old', state: 'closed' }] },
      tasks: [{ ...task('T-001'), phase: undefined, sprint: 'Old', status: 'todo' } as unknown as Task],
    });
    const { container } = render(TaskCard, { task: dashboard.taskMap.get('T-001')!, showSprint: true });
    const badge = container.querySelector('.task-card-sprint')!;
    expect(badge).toHaveClass('leftover');
    expect(badge.querySelector('.sprint-marker.closed')).not.toBeNull();
  });

  it('uses the row: the description’s start, the latest note, who and when (T-338)', () => {
    const t: Task = {
      ...task('T-001'),
      status: 'done',
      description: 'Set up **the** repo\n\nwith `npm`',
      created_by: 'Ada',
      started_by: 'claude:abc',
      completed_by: 'Ada',
      started_at: '2026-01-01T10:00:00Z',
      completed_at: '2026-01-01T10:30:00Z',
      notes: [{ at: '2026-01-01T10:30:00Z', by: 'claude:abc', kind: 'outcome', text: 'Ran `npm test`' }],
    };
    // Its work as atm counts it (ATM-721): the half hour from its start
    const work = {
      minutes: 30,
      live: false,
      spans: [{ from: '2026-01-01T10:00:00Z', to: '2026-01-01T10:30:00Z', by: 'claude:abc' }],
    };
    const { container } = render(TaskCard, { task: viewed(t, { kind: 'done', work }) });
    expect(container.querySelector('.task-excerpt')?.textContent).toContain('Set up the repo');
    expect(container.querySelector('.task-note')?.textContent).toMatch(/Outcome:\s*Ran npm test/);
    expect([...container.querySelectorAll('.task-fact-who')].map((e) => e.textContent)).toEqual([
      'Ada',
      'claude:abc',
      'Ada',
    ]);
    expect(container.querySelector('.task-fact[data-role="finished"]')).toHaveAttribute('title', 'Finished by Ada');
    expect(container.querySelector('.task-fact-time')?.textContent).toMatch(/^done .* ago · took 30 m$/);
  });

  it('has no excerpt, note or facts when the task holds none', () => {
    const bare: Task = { id: 'T-900', name: 'Bare', status: 'todo', type: 'feature' } as Task;
    const { container } = render(TaskCard, { task: bare });
    expect(container.querySelector('.task-excerpt')).toBeNull();
    expect(container.querySelector('.task-note')).toBeNull();
    expect(container.querySelector('.task-facts')).toHaveClass('empty');
    expect(container.querySelector('.task-fact')).toBeNull();
  });

  it('has no dependency label, label chips or tags when there are none', () => {
    const { container } = render(TaskCard, { task: task('T-001') });
    expect(container.querySelector('.label-chips')).toBeNull();
    expect(container.querySelector('.deps-label')).toBeNull();
    expect(container.querySelector('.task-tags')).toBeEmptyDOMElement();
  });
});
