import { fireEvent, render } from '@testing-library/svelte';
import { beforeEach, describe, expect, it } from 'vitest';
import LabelChips from '../../src/lib/ui/LabelChips.svelte';
import { dashboard } from '../../src/lib/state.svelte';
import type { Task } from '../../src/lib/types';
import standard from '../fixtures/standard.json';
import { loadIntoDashboard } from './components.helpers';

// In the order they show: labels without a place in meta.labels sort by name
const LABELS = ['a-very-long-label-name-indeed', 'alpha', 'delta', 'epsilon', 'gamma'];
const task = () => ({ ...(dashboard.taskMap.get('T-001') as Task), labels: LABELS });
const texts = (root: ParentNode, selector: string) => [...root.querySelectorAll(selector)].map((c) => c.textContent);

describe('LabelChips (T-256)', () => {
  beforeEach(() => loadIntoDashboard(standard));

  it('shows every label in full by default', () => {
    const { container } = render(LabelChips, { task: task() });
    expect(texts(container, '.label-chips > .label-chip')).toEqual(LABELS);
    expect(container.querySelector('.label-more')).toBeNull();
  });

  it('compact: the first two, then +N; its popover lists them all on hover, and a click pins it', async () => {
    const { container } = render(LabelChips, { task: task(), compact: true });
    expect(container.querySelector('.label-chips')).toHaveClass('compact');
    expect(texts(container, '.label-chips > span.label-chip')).toEqual(LABELS.slice(0, 2));
    const more = container.querySelector('.label-more')!;
    expect(more).toHaveTextContent('+3');
    expect(container.querySelector('.label-popover')).toBeNull();

    await fireEvent.mouseEnter(more);
    expect(texts(container, '.label-popover .label-chip')).toEqual(LABELS);
    await fireEvent.mouseLeave(more);
    expect(container.querySelector('.label-popover')).toBeNull();

    await fireEvent.click(more);
    expect(more).toHaveAttribute('aria-expanded', 'true');
    await fireEvent.keyDown(window, { key: 'Escape' });
    expect(container.querySelector('.label-popover')).toBeNull();
  });

  it('compact with two labels or fewer: no +N', () => {
    const { container } = render(LabelChips, { task: { ...task(), labels: ['gamma', 'alpha'] }, compact: true });
    expect(texts(container, '.label-chips > .label-chip')).toEqual(['alpha', 'gamma']);
    expect(container.querySelector('.label-more')).toBeNull();
  });
});
