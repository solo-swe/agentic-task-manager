import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import TaskCard from '../../src/components/TaskCard.svelte';
import { scrollToTask } from '../../src/lib/scroll';
import { dashboard } from '../../src/lib/state.svelte';
import type { Task } from '../../src/lib/types';
import standard from '../fixtures/standard.json';
import { loadIntoDashboard } from './components.helpers';

vi.mock('../../src/lib/scroll', () => ({ scrollToTask: vi.fn() }));

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
    loadIntoDashboard({
      ...standard,
      meta: {
        statuses: [
          { id: 'todo', label: 'Backlog', kind: 'todo' },
          { id: 'in_progress', label: 'Doing', kind: 'active' },
          { id: 'done', label: 'Shipped', kind: 'done' },
          { id: 'awaiting_user', label: 'Awaiting you', kind: 'review' },
        ],
      },
    });
    const { container } = render(TaskCard, { task: { ...task('T-001'), status: 'awaiting_user' } });
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
    expect(badge).toHaveStyle({ color: 'var(--red)' });
    const plain = render(TaskCard, { task: task('T-002') }).container.querySelector('.priority-badge')!;
    expect(plain).toHaveAttribute('title', 'P2 Medium priority (the default)');
  });

  it('shows how long a task in review has waited, with the review note on hover', () => {
    dashboard.loadedAt = Date.parse('2026-04-03T13:30:00Z');
    const reviewed = {
      ...task('T-005'),
      status: 'in_review',
      review_requested_at: '2026-04-03T11:00:00Z',
      notes: [{ at: '2026-04-03T11:00:00Z', kind: 'review', text: 'Check the editor toolbar' }],
    };
    const { container } = render(TaskCard, { task: reviewed });
    const line = container.querySelector('.review-wait')!;
    expect(line).toHaveTextContent('Waiting for review · 2 h 30 m');
    expect(line).toHaveAttribute('title', 'Check the editor toolbar');
    expect(render(TaskCard, { task: task('T-005') }).container.querySelector('.review-wait')).toBeNull();
  });

  it('a blocked task says who it waits on, for how long, and how many questions (listed on hover)', () => {
    dashboard.loadedAt = Date.parse('2026-04-03T13:00:00Z');
    const blocked = {
      ...task('T-008'),
      status: 'blocked',
      waiting_on: 'user',
      blocked_at: '2026-04-03T12:00:00Z',
      questions: ['REST or GraphQL?', 'Which DB?'],
    };
    const { container } = render(TaskCard, { task: blocked });
    const line = container.querySelector('.block-wait')!;
    expect(line).toHaveTextContent('Waiting on you · 1 h · 2 questions');
    expect(line).toHaveClass('on-user');
    expect(line).toHaveAttribute('title', 'REST or GraphQL?\nWhich DB?');
  });

  it("shows the task's labels as chips, in meta.labels' order and text", () => {
    loadIntoDashboard({ ...standard, meta: { labels: [{ id: 'ux', label: 'UX' }] } });
    const { container } = render(TaskCard, { task: { ...task('T-001'), labels: ['spike-2', 'ux'] } });
    expect([...container.querySelectorAll('.label-chip')].map((c) => c.textContent)).toEqual(['UX', 'spike-2']);
    expect(container.querySelector('.label-chip[data-label="ux"]')).toHaveAttribute('title', 'Label: ux');
  });

  it('has no dependency label, label chips or tags when there are none', () => {
    const { container } = render(TaskCard, { task: task('T-001') });
    expect(container.querySelector('.label-chips')).toBeNull();
    expect(container.querySelector('.deps-label')).toBeNull();
    expect(container.querySelector('.task-tags')).toBeEmptyDOMElement();
  });
});
