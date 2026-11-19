import { render } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import ReviewLink from '../../src/lib/details/ReviewLink.svelte';
import { learnReviews } from '../../src/lib/reviews/known.svelte';

// ATM-621: a task's link to its code review says where the review stands, from the task's own view (atm serves each
// review's status with the task), coloured as the Reviews list colours it; what the page learned since comes first
describe('ReviewLink', () => {
  const link = (props: Record<string, unknown>) =>
    render(ReviewLink, { props: { project: 'p', reference: 'CR-7', ...props } }).container;
  const status = (c: HTMLElement) => c.querySelector('.review-link-status');

  it('shows each status with its label, prominent or not, its colour by data-status', () => {
    for (const [given, label] of [
      ['draft', 'Draft'],
      ['waiting', 'Waiting for review'],
      ['live', 'Live'],
      ['changes_requested', 'Changes requested'],
      ['approved', 'Approved'],
      ['closed', 'Closed'],
    ] as const) {
      for (const prominent of [true, false]) {
        const s = status(link({ reference: `CR-${given}`, status: given, prominent }));
        expect(s?.textContent, `${given} ${prominent}`).toBe(label);
        expect(s?.getAttribute('data-status')).toBe(given);
      }
    }
  });

  it('without a status (an older atm), or with a word atm doesn’t have, the badge as before: its reference only', () => {
    for (const given of [undefined, null, 'archived']) {
      const c = link({ reference: 'CR-8', status: given, prominent: true });
      expect(c.querySelector('.review-link-ref')?.textContent).toBe('CR-8');
      expect(status(c)).toBeNull();
    }
  });

  it('a review atm can’t read says so instead', () => {
    const c = link({ reference: 'CR-9', status: 'unreadable' });
    expect(c.querySelector('.review-link-unreadable')?.textContent).toBe("can't be read");
    expect(status(c)).toBeNull();
  });

  it('what the page learned since the task’s view came wins: an approval seen on the Reviews tab', () => {
    learnReviews('p', [{ reference: 'CR-10', status: 'approved' }]);
    expect(status(link({ reference: 'CR-10', status: 'waiting' }))?.textContent).toBe('Approved');
  });
});
