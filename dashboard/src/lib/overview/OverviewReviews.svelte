<script lang="ts">
  // Every project's reviews in one list (ATM-575, Hakan: a fourth tab on the overview): the reviews list of a project's
  // Reviews tab (search, filters, in groups when unfiltered, what waits for you first), over the projects shown, each
  // row saying its project; a project picker narrows it to one. A row opens its review's page, in its project.
  import { page as router } from '$app/state';
  import { onDestroy, untrack } from 'svelte';
  import { addressOf, reviewPageOf, reviewProjectsOf, withReviewProjects } from '../address';
  import { inApp } from '../api/http';
  import type { ProjectEntry } from '../load';
  import { currentAddress, navigate } from '../navigation';
  import { loadReviews } from '../review-data';
  import { latest } from '../latest';
  import { keepFilter, keptFilter } from '../review-filter';
  import { mergeProjectReviews } from '../reviews/page';
  import ReviewList from '../reviews/ReviewList.svelte';
  import ReviewProjects from './ReviewProjects.svelte';
  import type { ListQuery, ReviewEntry } from '../reviews/types';
  import { tabActions } from '../tab-actions';

  let { projects }: { projects: ProjectEntry[] } = $props();

  /** The projects it can list: those there (a missing one has no reviews to read) */
  const present = $derived(projects.filter((p) => !p.missing));
  /** The projects chosen (ATM-757): the address's `?projects=`, so a reload, Back and a link keep them; none is every
   * project, and one the workspace doesn't list is left out */
  const chosen = $derived(
    reviewProjectsOf(
      addressOf(router.url),
      present.map((p) => p.id),
    ),
  );
  const listed = $derived(chosen.length ? present.filter((p) => chosen.includes(p.id)) : present);
  /** Other projects chosen: a step in the tab's history, as another filter is */
  const choose = (ids: string[]) => navigate(withReviewProjects(currentAddress(), ids));

  // The filter is kept as a project's is (ATM-604), under the overview's own name
  const KEPT = '__overview';
  let query = $state<ListQuery>(keptFilter(KEPT));
  function chooseQuery(next: ListQuery) {
    query = next;
    keepFilter(KEPT, next);
  }

  let entries = $state.raw<ReviewEntry[]>([]);
  let loading = $state(false);
  let problem = $state<string | null>(null);
  let shownText = '';
  // Only the latest ask's answer is shown (ATM-675)
  const loads = latest();

  /** Each project's reviews the query keeps, asked of each at once; a project that can't answer is left out, and the
   * list says which */
  async function load() {
    const ask = loads.ask();
    const from = listed;
    loading = !entries.length;
    const answers = await Promise.all(
      from.map((p) =>
        loadReviews(p.id, query).then(
          (list) => ({ project: { id: p.id, name: p.name }, entries: list.reviews }),
          () => null,
        ),
      ),
    );
    ask.done();
    if (!ask.current()) return;
    loading = false;
    const failed = from.filter((_, i) => !answers[i]).map((p) => p.name);
    problem = failed.length ? `Couldn't read the reviews of ${failed.join(', ')}.` : null;
    const merged = mergeProjectReviews(answers.filter((a) => a !== null));
    const text = JSON.stringify(merged);
    if (text !== shownText) {
      shownText = text;
      entries = merged;
    }
  }

  // Another query or project picked, or the projects listed changed: ask again
  const asking = $derived(JSON.stringify([query, listed.map((p) => p.id)]));
  $effect(() => {
    void asking;
    untrack(() => void load());
  });
  // The server pushes nothing about reviews: asked again every 10 s while the page is in view
  const timer = setInterval(() => {
    if (document.visibilityState === 'visible') void load();
  }, 10_000);
  onDestroy(() => clearInterval(timer));

  const pageOf = (entry: ReviewEntry | undefined, reference: string) =>
    reviewPageOf(currentAddress(), entry?.project?.id ?? '', reference);

  /** A row opened: its review's page, in its project, in place (its back button comes back here, ATM-686); with ⌘ or
   * Ctrl in a tab of its own, the app's or the browser's */
  function open(reference: string, newTab: boolean, entry?: ReviewEntry) {
    if (!entry?.project) return;
    const address = pageOf(entry, reference);
    if (newTab && inApp) tabActions().open(address);
    else if (newTab) window.open(address, '_blank', 'noopener');
    else navigate(address);
  }
</script>

<!-- The project filter, beside the search in the title bar (docs/sidebar-overhaul-design.md §2.5, §6) -->
{#snippet projectsFilter()}
  <ReviewProjects projects={present} {chosen} onchoose={choose} />
{/snippet}

<div class="overview-reviews" id="overview-reviews">
  <!-- A project that couldn't answer is named here, above the others' reviews (ReviewList's own problem shows instead of
       its rows) -->
  {#if problem}<p class="overview-reviews-problem" role="alert">{problem}</p>{/if}
  <ReviewList
    {entries}
    {query}
    {loading}
    href={(reference, entry) => pageOf(entry, reference)}
    onquery={chooseQuery}
    onopen={open}
    searchAside={present.length > 1 ? projectsFilter : undefined}
  />
</div>

<style>
  .overview-reviews {
    display: flex;
    flex-direction: column;
  }
  .overview-reviews-problem {
    margin: var(--space-6) var(--space-6) 0;
    color: var(--text-muted);
  }
</style>
