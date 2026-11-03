import { fireEvent, render } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import ProjectSelector from '../../src/components/ProjectSelector.svelte';
import type { ProjectEntry, ProjectSummary } from '../../src/lib/load';
import { dashboard } from '../../src/lib/state.svelte';
import { summary as fullSummary } from './summary.helpers';

const summary = (percent: number, review = 0, blocked = 0): ProjectSummary =>
  fullSummary({ total: 10, done: percent / 10, percent, needs_you: { review, blocked_on_user: blocked, together: 0 } });
const project = (id: string, name: string, s: ProjectSummary | null, missing = false): ProjectEntry => ({
  id,
  name,
  path: `/work/${id}`,
  missing,
  summary: s,
});

describe('ProjectSelector', () => {
  beforeEach(() => {
    dashboard.projects = [
      project('a', 'Alpha', summary(40, 1)),
      project('b', 'Beta', summary(90, 0, 2)),
      project('c', 'Gone', null, true),
    ];
    dashboard.route = { page: 'project', id: 'a' };
  });
  afterEach(() => history.replaceState(null, '', '/'));

  it('badges what needs you in the other projects only', () => {
    const { container } = render(ProjectSelector);
    expect(container.querySelector('#other-projects-need')).toHaveTextContent('2');
    expect(container.querySelector('#project-selector')).toHaveAttribute(
      'aria-label',
      'Switch project (2 tasks in other projects need you)',
    );
  });

  it('opens a list: Projects Overview, then each project with its progress; a gone one is listed but disabled', async () => {
    const { container } = render(ProjectSelector);
    await fireEvent.click(container.querySelector('#project-selector')!);
    const options = [...container.querySelectorAll('[role=option]')];
    expect(options.map((o) => o.textContent?.trim().split('\n')[0])).toEqual([
      'Projects Overview',
      'Alpha',
      'Beta',
      'Gone not found',
    ]);
    expect(container.querySelector('[data-id="a"]')).toHaveAttribute('aria-selected', 'true');
    expect(container.querySelector('[data-id="c"]')).toHaveAttribute('aria-disabled', 'true');
    expect(container.querySelector('[data-id="b"] .project-option-fill')).toHaveStyle('width: 90%');
    expect(container.querySelector('[data-id="a"] .project-option-dot')).not.toBeNull();
  });

  it('typing filters; Enter opens the active project in the URL', async () => {
    const { container } = render(ProjectSelector);
    await fireEvent.click(container.querySelector('#project-selector')!);
    const filter = container.querySelector('#project-filter') as HTMLInputElement;
    await fireEvent.input(filter, { target: { value: 'bet' } });
    expect([...container.querySelectorAll('[role=option]')].map((o) => o.getAttribute('data-id'))).toEqual(['b']);
    await fireEvent.keyDown(filter, { key: 'Enter' });
    expect(location.hash).toBe('#project=b');
    expect(container.querySelector('#project-menu')).toBeNull();
  });

  it('Projects Overview goes to the overview; typing its name finds it (T-274)', async () => {
    const { container } = render(ProjectSelector);
    await fireEvent.click(container.querySelector('#project-selector')!);
    await fireEvent.input(container.querySelector('#project-filter')!, { target: { value: 'overv' } });
    expect([...container.querySelectorAll('[role=option]')].map((o) => o.getAttribute('data-id'))).toEqual([
      'overview',
    ]);
    await fireEvent.click(container.querySelector('[data-id="overview"]')!);
    expect(location.hash).toBe('#overview');
  });
});
