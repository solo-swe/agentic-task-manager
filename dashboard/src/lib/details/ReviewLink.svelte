<script lang="ts">
  // A task's link to one of its code reviews (ATM-585; the cards and the list share it, ATM-575): its reference and
  // where it stands (ATM-621), coloured as the Reviews list colours it: from the task's own view, which atm serves
  // with each review's status (never a request per task), or what the page has learned since (the Reviews tab
  // listed it); *needs you* when the project's summary says so. A click opens the review's page: in the app in a tab
  // of its own, as the Reviews list opens one; in the browser in this tab, or another for ⌘-click, Ctrl-click or a
  // middle click, as any link.
  import { inApp } from '../api/http';
  import { reviewPageOf } from '../address';
  import { currentAddress, navigate } from '../navigation';
  import { knownStatus, knownUnreadable } from '../reviews/known.svelte';
  import { STATUS_LABELS, type ReviewStatus } from '../reviews/types';
  import { dashboard } from '../state.svelte';
  import { tabActions, wantsNewTab } from '../tab-actions';
  import Icon from '../ui/Icon.svelte';

  interface Props {
    /** The project the review is in */
    project: string;
    /** Its reference (CR-7) */
    reference: string;
    /** The one to look at: drawn larger */
    prominent?: boolean;
    /** Where it stands, as the task's view says (`derived.review_statuses`, ATM-621): a status, or `unreadable` */
    status?: string | null;
  }
  let { project, reference, prominent = false, status: given = null }: Props = $props();

  /** A review's status, as atm names them (an older or newer atm's other word shows nothing) */
  const isStatus = (s: string | null | undefined): s is ReviewStatus => !!s && Object.hasOwn(STATUS_LABELS, s);

  // What the page learned since the task's view came (fresher) first, else the view's own
  const status = $derived(knownStatus(project, reference) ?? (isStatus(given) ? given : null));
  // A review atm can't read (ATM-617) says so, wherever its link is
  const unreadable = $derived(
    knownUnreadable(project, reference) ?? (given === 'unreadable' ? "atm can't read this review" : null),
  );
  const waiting = $derived(
    dashboard.projects
      .find((p) => p.id === project)
      ?.summary?.needs_you_reviews?.some((r) => r.reference === reference) ?? false,
  );
  const href = $derived(reviewPageOf(currentAddress(), project, reference));
  const tabs = tabActions();

  function open(e: MouseEvent) {
    // The link's own, a new tab's too: never the card's or row's it is on (ATM-619), which would open the task
    e.stopPropagation();
    if (!inApp && wantsNewTab(e)) return;
    e.preventDefault();
    // In place, so the review's back button comes back here (ATM-686); ⌘-click a tab of its own
    if (inApp && wantsNewTab(e)) tabs.open(href);
    else navigate(href);
  }
</script>

<a
  class="review-link"
  class:prominent
  {href}
  data-review={reference}
  onclick={open}
  onauxclick={open}
  title="Open {reference}'s page"
  ><Icon name="message-square" size={prominent ? 14 : 12} /><span class="review-link-ref">{reference}</span
  >{#if unreadable}<span class="review-link-unreadable" title={unreadable}>can't be read</span>{:else if status}<span
      class="review-link-status"
      data-status={status}>{STATUS_LABELS[status]}</span
    >{/if}{#if waiting}<span class="review-link-needs">needs you</span>{/if}</a
>

<style>
  .review-link {
    align-items: center;
    color: var(--accent);
    display: inline-flex;
    font-size: var(--text-xs);
    gap: var(--space-2);
    text-decoration: none;
  }
  .review-link:hover .review-link-ref {
    text-decoration: underline;
  }
  .review-link.prominent {
    border: 1px solid color-mix(in srgb, var(--accent) 45%, transparent);
    border-radius: var(--radius-pill);
    font-size: var(--text-sm);
    font-weight: 600;
    padding: var(--space-1) var(--space-5);
  }
  .review-link-ref {
    font-family: 'SF Mono', Menlo, Consolas, monospace;
  }
  /* Coloured as the Reviews list colours a status (ATM-621): approved green, sent back red, waiting and live the
     accent, closed and draft muted */
  .review-link-status {
    color: var(--text-muted);
    font-weight: 400;
  }
  .review-link-status:is([data-status='waiting'], [data-status='live']) {
    color: var(--accent);
  }
  .review-link-status[data-status='approved'] {
    color: var(--green);
  }
  .review-link-status[data-status='changes_requested'] {
    color: var(--red);
  }
  .review-link-status::before {
    content: '· ';
  }
  .review-link-unreadable {
    color: var(--red);
    font-weight: 400;
  }
  .review-link-unreadable::before {
    content: '· ';
  }
  .review-link-needs {
    color: var(--purple);
    font-weight: 600;
  }
</style>
