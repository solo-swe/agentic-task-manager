<script lang="ts">
  import { page as router } from '$app/state';
  import { onDestroy, untrack } from 'svelte';
  import { addressOf, reviewOf, withReview } from '../address';
  import { watchWorkingCopy } from '../api/changes';
  import { inApp } from '../api/http';
  import { setWindowTitle } from '../api/shell';
  import {
    addComment,
    addDraft,
    closeReview,
    editComment,
    openReview,
    pokeReview,
    removeDraft,
    replyTo,
    resolveComment,
    submitDecision,
    updateDraft,
    type ChangeResult,
  } from '../api/review-changes';
  import { currentAddress, navigate } from '../navigation';
  import {
    LIVE_MS,
    isViewed,
    loadReview,
    loadReviews,
    revisionDocuments,
    toggleViewed,
    viewedKeys,
  } from '../review-data';
  import { keepFilter, keptFilter } from '../review-filter';
  import type { TaskLinks } from '../compmd';
  import { learnReviews, learnUnreadable } from '../reviews/known.svelte';
  import ReviewList from '../reviews/ReviewList.svelte';
  import ReviewPage from '../reviews/ReviewPage.svelte';
  import { reviewTitle } from '../reviews/page';
  import {
    NO_QUERY,
    type ListQuery,
    type ReviewEntry,
    type ReviewPageData,
    type UnreadableReview,
    type ViewFile,
  } from '../reviews/types';
  import { reviewWindow } from '../reviews/window.svelte';
  import { withTask } from '../routes';
  import { tabActions } from '../tab-actions';
  import { withView } from '../tabs';
  import { dashboard } from '../state.svelte';

  // A project's reviews (docs/code-review-design.md §3.11, ATM-574): the reviews page, on the project's Reviews tab, or
  // a review's page when the address names one (/projects/<id>/reviews/CR-7?round=&against=&file=), which the layout
  // shows over the whole window, titled by the project's short name, the reference and the title. The review tool's
  // components take their data and handlers from here: the task manager's side (its project, its address, its client
  // layer).
  const project = $derived(dashboard.projectId);
  const at = $derived(reviewOf(addressOf(router.url)));

  // The list
  let query = $state<ListQuery>({ ...NO_QUERY });
  // Opened with the project's last filter, kept in this browser (ATM-604), and again on going to another project
  let queryOf: string | null = null;
  $effect.pre(() => {
    const id = project;
    untrack(() => {
      if (!id || id === queryOf) return;
      queryOf = id;
      query = keptFilter(id);
    });
  });
  function chooseQuery(next: ListQuery) {
    query = next;
    if (project) keepFilter(project, next);
  }
  let entries = $state.raw<ReviewEntry[]>([]);
  // The reviews atm can't read (ATM-617), listed after them
  let unreadable = $state.raw<UnreadableReview[]>([]);
  // The review shown
  let shown = $state.raw<ReviewPageData | null>(null);
  let loading = $state(false);
  let busy = $state(false);
  let problem = $state<string | null>(null);
  let viewed = $state.raw<string[]>(viewedKeys());
  // A reference two reviews share (CR-8, when two clones took the number: CR-8.1 and CR-8.2), to choose from
  let choices = $state.raw<ReviewEntry[]>([]);

  // What was last shown, as text: a refresh that changed nothing changes nothing on the page
  let shownText = '';
  let entriesText = '';

  let asked = 0;
  async function load() {
    const id = project;
    if (!id) return;
    const ask = ++asked;
    if (!at.review) loading = !entries.length;
    try {
      if (at.review) {
        const page = await loadReview(id, at.review, at.round, at.against);
        learnReviews(id, [page.review]);
        const text = JSON.stringify(page);
        if (ask === asked && text !== shownText) {
          shownText = text;
          [shown, choices] = [page, []];
        }
      } else {
        const listed = await loadReviews(id, query);
        learnReviews(
          id,
          listed.reviews.map((e) => e.review),
        );
        learnUnreadable(id, listed.unreadable ?? []);
        const text = JSON.stringify([listed.reviews, listed.unreadable]);
        if (ask === asked && text !== entriesText) {
          entriesText = text;
          [entries, unreadable] = [listed.reviews, listed.unreadable ?? []];
        }
      }
      if (ask === asked) problem = null;
    } catch (e) {
      if (ask !== asked) return;
      problem = (e as Error).message;
      const review = at.review;
      if (review && !shown) {
        const listed = await loadReviews(id, NO_QUERY).catch(() => null);
        if (ask === asked) choices = (listed?.reviews ?? []).filter((r) => r.review.reference.startsWith(`${review}.`));
      }
    } finally {
      if (ask === asked) loading = false;
    }
  }

  // Another project, review, round or query: ask again (another file in the address is the page's to go to: nothing
  // to ask)
  const asking = $derived(JSON.stringify([project, at.review, at.round, at.against, query]));
  $effect(() => {
    void asking;
    untrack(() => {
      if (!at.review) {
        shown = null;
        shownText = '';
      }
      void load();
    });
  });

  // The server pushes nothing about reviews yet: ask again now and then while the tab is in view, and on coming back.
  // A live review follows its working copy (ATM-576): in the app, asked again when the app says a file of the work
  // changed (ATM-607); in the browser, every LIVE_MS while it is shown, a 304 while nothing changed. The rest every 10 s
  const followingLive = $derived(!!at.review && shown?.review.status === 'live');
  $effect(() => {
    const id = project;
    if (!inApp || !followingLive || !id) return;
    return watchWorkingCopy(dashboard.workspace, id, () => void load());
  });
  let ticks = 0;
  const timer = setInterval(() => {
    if (document.visibilityState !== 'visible') return;
    ticks += 1;
    if ((followingLive && !inApp) || ticks % (10_000 / LIVE_MS) === 0) void load();
  }, LIVE_MS);
  onDestroy(() => clearInterval(timer));

  // The window's title: the project's short name, the review's reference and title
  const shortName = $derived(
    typeof dashboard.meta?.key === 'string' && dashboard.meta.key ? dashboard.meta.key : dashboard.projectName,
  );
  $effect(() => {
    if (!at.review) return;
    const title = reviewTitle(shortName, shown?.review.reference ?? at.review, shown?.review.title ?? '');
    reviewWindow.title = title;
    if (shown && project)
      reviewWindow.names[`${project}:${at.review}`] = reviewTitle(null, shown.review.reference, shown.review.title);
    if (inApp) return; // the app's title bar sets it
    document.title = title;
  });
  onDestroy(() => (reviewWindow.title = ''));
  $effect(() => {
    if (inApp && at.review && reviewWindow.title) void setWindowTitle(reviewWindow.title);
  });

  const go = (address: string, replace = false) => navigate(address, { replace });

  /** A review opened from the list: in the app, in a tab of its own; in the browser, in place (its page has a way
   * back), or with ⌘ or Ctrl in a browser tab of its own */
  function open(reference: string, newTab = false) {
    const address = withReview(currentAddress(), { review: reference });
    if (inApp) tabActions().open(address);
    else if (newTab) window.open(address, '_blank', 'noopener');
    else go(address);
  }

  /** A change made: the page as it is now; or why it was refused, shown and given back (a comment's box keeps it) */
  async function change(made: Promise<ChangeResult>): Promise<string | null> {
    busy = true;
    const result = await made;
    busy = false;
    if (!result.ok) {
      problem = result.message;
      return result.message;
    }
    problem = null;
    await load();
    return null;
  }

  /** A live review opened from the list (ATM-576), then shown; or why it was refused */
  async function startLive(live: { title: string; base: string; task: string }): Promise<string | null> {
    if (!project) return 'No project is open.';
    const result = await openReview(project, {
      title: live.title,
      start: 'live',
      change: live.base ? { base: live.base } : {},
      ...(live.task ? { tasks: [live.task] } : {}),
    });
    if (!result.ok) return result.message;
    const reference = result.body.reference;
    if (typeof reference === 'string') open(reference);
    return null;
  }

  const review = $derived(shown?.review.reference ?? at.review ?? '');
  const listAddress = $derived(withReview(currentAddress(), { review: null }));

  // A task named in a review (ATM-593): this project's opens its details over the review (the review's own address,
  // with `?task=`), closed back to the review as it was; another project's opens at its project's page. ⌘, Ctrl or a
  // middle click: a tab of its own, at its project's page with its details. A key naming no task the page knows
  // isn't a link
  const foreign = (key: string) => (dashboard.findTask(key) ? null : (dashboard.refOf(key).address ?? null));
  /** A task's own page: its project's list with its details, for a link's address and a tab of its own */
  function taskPage(key: string): string {
    return foreign(key) ?? withTask(withView(listAddress, 'list'), key);
  }
  const taskLinks: TaskLinks = {
    known: (key) => !!dashboard.findTask(key) || !!foreign(key),
    open(key, newTab) {
      const elsewhere = foreign(key);
      if (newTab || (elsewhere && inApp)) {
        const address = taskPage(key);
        if (inApp) tabActions().open(address);
        else window.open(address, '_blank', 'noopener');
      } else if (elsewhere) go(elsewhere);
      else go(withTask(currentAddress(), key));
    },
  };
</script>

<svelte:window onfocus={() => void load()} />

<div class="reviews-tab" class:reviewing={!!at.review} id="reviews-tab">
  {#if at.review}
    {#if shown && project}
      {@const page = shown}
      <ReviewPage
        {page}
        project={shortName}
        file={at.file}
        side={(revision) => revisionDocuments(project, revision)}
        isViewed={(file: ViewFile) => isViewed(viewed, page.review.id, page.view.round, page.view.against, file)}
        taskHref={taskPage}
        tasks={taskLinks}
        backHref={listAddress}
        onback={() => go(listAddress)}
        {busy}
        {problem}
        docStyle={dashboard.docStyle === 'auto' ? null : dashboard.docStyle}
        onfile={(path) => go(withReview(currentAddress(), { ...at, file: path }), true)}
        onround={(round, against) => go(withReview(currentAddress(), { ...at, round, against }))}
        onviewed={(file) => (viewed = toggleViewed(page.review.id, page.view.round, page.view.against, file))}
        oncomment={({ path, picked, body }) =>
          change(
            addComment(project, review, {
              path,
              lines: picked?.lines ?? null,
              side: picked?.side ?? 'new',
              quote: picked?.quote ?? null,
              round: path ? page.view.round : null,
              body,
            }),
          )}
        onreply={(to, body) => change(replyTo(project, review, to, body))}
        onresolve={(comment, resolved) => change(resolveComment(project, review, comment, resolved))}
        onedit={(comment, body) => change(editComment(project, review, comment, body))}
        onsubmit={(decision, body) => change(submitDecision(project, review, body ? { decision, body } : { decision }))}
        onclose={(reason) => change(closeReview(project, review, reason))}
        onpoke={(body) => change(pokeReview(project, review, body))}
        ondrafts={{
          comment: (path, picked, body) =>
            change(
              addDraft(project, review, {
                path,
                lines: picked?.lines ?? null,
                side: picked?.side ?? 'new',
                quote: picked?.quote ?? null,
                round: path ? page.view.round : null,
                body,
              }),
            ),
          reply: (to, body) => change(addDraft(project, review, { reply_to: to, body })),
          edit: (id, body) => change(updateDraft(project, review, id, body)),
          remove: (id) => change(removeDraft(project, review, id)),
        }}
        ondocument={(ref) => dashboard.openDocument(ref)}
      />
    {:else if problem}
      <p class="reviews-problem" role="alert">{problem}</p>
      {#if choices.length}
        <ul class="reviews-choices" aria-label="The reviews {at.review} names">
          {#each choices as choice (choice.review.id)}
            <li>
              <a
                href={withReview(currentAddress(), { review: choice.review.reference })}
                onclick={(e) => {
                  e.preventDefault();
                  go(withReview(currentAddress(), { review: choice.review.reference }));
                }}>{choice.review.reference}</a
              >
              {choice.review.title}
            </li>
          {/each}
        </ul>
      {/if}
      <button class="reviews-back" onclick={() => go(listAddress)}>All reviews</button>
    {:else}
      <p class="reviews-loading">Loading {at.review}…</p>
    {/if}
  {:else}
    <ReviewList
      {entries}
      {unreadable}
      {query}
      {loading}
      {problem}
      href={(reference) => withReview(currentAddress(), { review: reference })}
      onquery={chooseQuery}
      onopen={(reference, newTab) => open(reference, newTab)}
      onpoke={project ? (entry) => change(pokeReview(project, entry.review.reference)) : undefined}
      onlive={startLive}
      taskHref={taskPage}
      {taskLinks}
    />
  {/if}
</div>

<style>
  .reviews-tab {
    min-height: 100%;
  }
  .reviews-tab.reviewing {
    height: 100%;
  }
  .reviews-problem,
  .reviews-loading {
    padding: var(--space-6);
    color: var(--text-muted);
  }
  .reviews-back {
    margin-left: var(--space-6);
  }
  .reviews-choices {
    margin: 0 var(--space-6) var(--space-6);
    padding-left: var(--space-8);
  }
</style>
