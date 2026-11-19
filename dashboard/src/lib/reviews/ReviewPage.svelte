<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from 'svelte';
  import CompMd from '../documents/CompMd.svelte';
  import type { DocumentRef } from '../documents';
  import type { DocumentSource } from '../documents/source';
  import { formatWhen } from '../format';
  import { readStored, writeStored } from '../prefs';
  import Icon from '../ui/Icon.svelte';
  import ChainBox from './ChainBox.svelte';
  import ComposerActions from './ComposerActions.svelte';
  import { draftedComments, setDraftActions, withDrafts, type DraftActions } from './drafts';
  import { focusOnShow } from './focus';
  import { setCommentEdits, setDocumentLinks, setTaskLinks } from './links';
  import type { TaskLinks } from '../compmd';
  import ReviewFile, { holdNearCards, whenIdle } from './ReviewFile.svelte';
  import {
    chainsOf,
    driftedOf,
    fileTree,
    filesIn,
    filterChains,
    isOpen,
    isResolved,
    lineCounts,
    openedTo,
    reviewChains,
    shownRows,
    treeFolders,
    treeOrder,
    treeWidth,
    TREE_WIDTH,
    type ChainFilter,
  } from './page';
  import { STATUS_LABELS, type PlacedChain, type Picked, type ReviewPageData, type ViewFile } from './types';

  // A review's page (docs/code-review-design.md §3.11, ATM-574), a page of its own: a top bar (the project's short
  // name, the reference and title, its status, *Submit review*, which is *Approve* once no comment is open) and three
  // tabs. *Overview*: who and what at the top (author, reviewers, branch, tasks, rounds), the description, and the
  // conversation on the review beside it. *Diffs*: the round picker (a round's whole change, or a round against an
  // earlier one), the files as a tree beside the list of their diffs (a splitter between them, its width kept), each
  // file a card that folds once viewed, the previous and next change and comment, the layout (split, unified) and view
  // (source, rendered) for all. *Comments*: every chain, all, open or resolved. Keys on the Diffs tab: n / p the next
  // and previous change, c / C the next and previous comment, ] / [ the next and previous file, v the file at the top
  // viewed. Everything it does is the page's: the handlers. Which round and file are shown is the address's.
  let {
    page,
    project = null,
    file = null,
    side,
    isViewed,
    taskHref,
    tasks,
    backHref,
    busy = false,
    problem = null,
    docStyle = null,
    onback,
    onfile,
    onround,
    onviewed,
    oncomment,
    onreply,
    onresolve,
    onsubmit,
    onclose,
    onpoke,
    ondrafts,
    onedit,
    ondocument,
  }: {
    page: ReviewPageData;
    /** The project's short name (its key), before the reference */
    project?: string | null;
    /** The file in view (its path): the Diffs tab opens at it */
    file?: string | null;
    /** A side's files: the commit or tree a round's side is read from */
    side: (revision: string) => DocumentSource;
    isViewed: (file: ViewFile) => boolean;
    /** A task's address, for its link */
    taskHref?: (key: string) => string;
    /** What a task named in the review opens (ATM-593): the Tasks fact's keys, and the keys written in the description
     * and the comments that name a task (`known`); `open`, its details over the review, or `newTab` a tab of its own */
    tasks?: TaskLinks;
    /** The reviews page's address, for the link back */
    backHref?: string;
    busy?: boolean;
    problem?: string | null;
    /** The document style picked in Settings, for a Markdown file's rendered view */
    docStyle?: string | null;
    onback?: () => void;
    onfile: (path: string) => void;
    /** `round` null: the latest (a live review's working copy now) */
    onround: (round: number | null, against: number | null) => void;
    onviewed: (file: ViewFile) => void;
    /** A comment made; why it wasn't saved, if it wasn't (its box keeps it) */
    oncomment: (comment: { path: string | null; picked: Picked | null; body: string }) => Promise<string | null> | void;
    onreply: (to: string, body: string) => void;
    onresolve: (comment: string, resolved: boolean) => void;
    onsubmit: (decision: 'approve' | 'request_changes' | 'comment', body: string) => void;
    /** The review closed, ended without approval, the summary its reason (ATM-576: how a live review ends unapproved) */
    onclose?: (reason: string) => void;
    /** A poke sent, with its message (ATM-611); why it wasn't, if it wasn't (its box keeps it) */
    onpoke?: (body: string) => Promise<string | null> | void;
    /** What writing, changing and removing a draft does (ATM-612): every box that writes a comment or a reply then
     * offers *Add to my review*; without it, they send at once */
    ondrafts?: Omit<DraftActions, 'pending'>;
    /** One's own comment's text changed (ATM-638); why it wasn't, if it wasn't (its box keeps it) */
    onedit?: (comment: string, body: string) => Promise<string | null>;
    /** A link in the summary or a comment to a document of the project that isn't a file of the round's: the task
     * manager opens its document viewer (ATM-586) */
    ondocument?: (ref: DocumentRef) => void;
  } = $props();

  const review = $derived(page.review);
  const view = $derived(page.view);
  // The user's drafts among the chains (ATM-612): placed where the server placed them, marked, counted open (each would
  // be an open comment once sent, which an approval can't be made with)
  const allChains = $derived(withDrafts(page.chains, page.drafts, page.you, view.round));
  const pendingDrafts = $derived(page.drafts?.length ?? 0);
  // Editing one's own comments (ATM-638): none on an ended review, which takes no change (no one is "you" there)
  if (untrack(() => onedit))
    setCommentEdits({
      get you() {
        return ended ? '' : page.you;
      },
      edit: (comment, body) => onedit?.(comment, body) ?? Promise.resolve('Comments can’t be changed here.'),
    });
  // Set once, for the boxes under the page: each calls the handlers as they are then (a round shown since, say)
  const refusedHere = 'Drafts can’t be kept here.';
  if (untrack(() => ondrafts))
    setDraftActions({
      get pending() {
        return pendingDrafts;
      },
      comment: (path, picked, body) => ondrafts?.comment(path, picked, body) ?? Promise.resolve(refusedHere),
      reply: (to, body) => ondrafts?.reply(to, body) ?? Promise.resolve(refusedHere),
      edit: (id, body) => ondrafts?.edit(id, body) ?? Promise.resolve(refusedHere),
      remove: (id) => ondrafts?.remove(id) ?? Promise.resolve(refusedHere),
    });
  // In the tree's order (ATM-596): the diffs, moving between files, the round's select and the Viewed count follow it
  const files = $derived(treeOrder(view.files));
  const tree = $derived(fileTree(files));
  const ended = $derived(review.status === 'approved' || review.status === 'closed');
  /** A live review: its working copy followed as it is made (ATM-576) */
  const live = $derived(review.status === 'live');
  /** Sent back to its author: more words are *Publish comments*, never a second send-back (ATM-597, CR-2) */
  const sentBack = $derived(review.status === 'changes_requested');
  const rounds = $derived(page.rounds.map((r) => r.number));
  const sideOf = (revision: string | null) => (revision ? side(revision) : null);
  const statusLetter = { added: 'A', modified: 'M', deleted: 'D', renamed: 'R' } as const;
  const totals = $derived(
    files.reduce(
      (sum, f) => {
        const c = lineCounts(f.patch);
        return { added: sum.added + c.added, removed: sum.removed + c.removed };
      },
      { added: 0, removed: 0 },
    ),
  );
  const when = (at: string) => {
    const ts = Date.parse(at);
    return Number.isNaN(ts) ? at : formatWhen(ts, Date.now());
  };

  // The tab: Diffs when the address names a file, else Overview
  let tab = $state<'overview' | 'diffs' | 'comments'>('overview');
  onMount(() => {
    if (file) tab = 'diffs';
  });

  // The layout and view of every diff, and the tree's width: kept in this browser
  const LAYOUT_KEY = 'atm-review-layout';
  const stored = readStored(LAYOUT_KEY);
  let mode = $state<'split' | 'unified'>(stored.mode === 'unified' ? 'unified' : 'split');
  let rendered = $state<'source' | 'rendered'>(stored.view === 'rendered' ? 'rendered' : 'source');
  let width = $state(typeof stored.tree === 'number' ? stored.tree : TREE_WIDTH.default);
  /** Long lines wrap (ATM-588): in Unified, until Split can keep its sides level with a wrapped line */
  let wrap = $state(stored.wrap === true);
  $effect(() => writeStored(LAYOUT_KEY, { mode, view: rendered, tree: width, wrap }));
  const WRAP_IN_SPLIT = 'Wrap works in Unified for now: Split keeps its two sides level line by line';
  const hasMarkdown = $derived(files.some((f) => /\.(md|markdown|mdx)$/i.test(f.path)));

  // A file folds once viewed; its fold button opens or folds it again whatever its tick
  let folds = $state<Record<string, boolean>>({});
  const folded = (f: ViewFile) => folds[f.path] ?? isViewed(f);
  function toggleViewed(f: ViewFile) {
    const rest = { ...folds };
    delete rest[f.path];
    folds = rest;
    onviewed(f);
  }
  const viewedCount = $derived(files.filter((f) => isViewed(f)).length);

  // The tree's folders fold (ATM-595): what is folded is kept per review for the session (a reload keeps it, nothing
  // else does), and going to a file opens the folders on its way, so the file shown is always in the tree
  const TREE_KEY = 'atm-review-tree:';
  let collapsed = $state.raw<ReadonlySet<string>>(new Set());
  let collapsedFor = '';
  $effect.pre(() => {
    const id = review.id;
    if (id === collapsedFor) return;
    collapsedFor = id;
    let kept: unknown = null;
    try {
      kept = JSON.parse(sessionStorage.getItem(TREE_KEY + id) ?? 'null');
    } catch {
      // no storage (a private window), or not ours: nothing folded
    }
    collapsed = new Set(Array.isArray(kept) ? kept.filter((p): p is string => typeof p === 'string') : []);
  });
  function fold(next: ReadonlySet<string>) {
    collapsed = next;
    try {
      sessionStorage.setItem(TREE_KEY + review.id, JSON.stringify([...next]));
    } catch {
      // kept for the page only
    }
  }
  function toggleFolder(path: string, open = collapsed.has(path)) {
    const rest = [...collapsed].filter((p) => p !== path);
    fold(new Set(open ? rest : [...rest, path]));
  }
  function folderKeys(e: KeyboardEvent, path: string) {
    if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
    e.preventDefault();
    toggleFolder(path, e.key === 'ArrowRight');
  }
  const shownTree = $derived(shownRows(tree, collapsed));
  $effect(() => {
    const path = atFile;
    if (!path) return;
    untrack(() => {
      const next = openedTo(collapsed, path);
      if (next !== collapsed) fold(next);
    });
  });
  /** A folded folder's sum: its files, those with comments, those not viewed yet */
  function folderSum(path: string) {
    const inIt = filesIn(tree, path);
    const commented = inIt.filter((f) => chainsOf(allChains, f).length).length;
    const unviewed = inIt.filter((f) => !isViewed(f)).length;
    return [
      `${inIt.length} ${inIt.length === 1 ? 'file' : 'files'}`,
      ...(commented ? [`${commented} with comments`] : []),
      ...(unviewed && unviewed < inIt.length ? [`${unviewed} not viewed`] : []),
      ...(unviewed && unviewed === inIt.length && viewedCount ? ['none viewed'] : []),
    ].join(' · ');
  }

  // The list of diffs, and the file at its top (the tree marks it; ] [ and v act on it)
  let list = $state<HTMLElement | null>(null);
  let atFile = $state<string | null>(null);
  const cards = () => [...(list?.querySelectorAll<HTMLElement>('.review-file-card') ?? [])];
  /** The file gone to (the tree, ] and [): it stays the one marked while its card is in view, even where the list
   * can't scroll it to the top (the last files) */
  let chosen: string | null = null;
  function findTop() {
    const box = list?.getBoundingClientRect();
    if (!box) return;
    const kept = chosen
      ? cards()
          .find((c) => c.dataset.file === chosen)
          ?.getBoundingClientRect()
      : undefined;
    if (kept && kept.top >= box.top - 4 && kept.top < box.bottom) {
      atFile = chosen;
      return;
    }
    chosen = null;
    const card = cards().find((c) => c.getBoundingClientRect().bottom > box.top + 48);
    atFile = card?.dataset.file ?? null;
  }
  let scrolling = 0;
  function onscroll() {
    if (scrolling) return;
    scrolling = requestAnimationFrame(() => {
      scrolling = 0;
      findTop();
    });
  }

  let writing: (() => void) | null = null;
  onDestroy(() => writing?.());
  /** A link in the summary or a comment to a document of the project (ATM-586): a file the round changed opens its diff
   * here, the review being about it; anything else, or a document's heading or text, the document viewer */
  function openLink(ref: DocumentRef) {
    if (!ref.heading && !ref.text && files.some((f) => f.path === ref.path)) void showFile(ref.path);
    else ondocument?.(ref);
  }
  setDocumentLinks(openLink);
  // The keys in its texts: links to their tasks (ATM-593). Read when a text is drawn, so the latest handlers apply
  setTaskLinks({
    known: (key) => tasks?.known(key) ?? false,
    open: (key, newTab) => tasks?.open(key, newTab),
  });
  /** A task's key clicked (the Tasks fact): its details over the review, or with ⌘, Ctrl or a middle click a tab of
   * its own; without `tasks`, the link's own address */
  function openTask(e: MouseEvent, key: string) {
    if (!tasks) return;
    e.preventDefault();
    tasks.open(key, e.metaKey || e.ctrlKey || e.button === 1);
  }

  /** The file just gone to: its card is made at once, the others after it */
  let wanted = $state<string | null>(null);
  const cardOf = (path: string) =>
    list?.querySelector<HTMLElement>(`.review-file-card[data-file="${CSS.escape(path)}"]`) ?? null;
  async function showFile(path: string, address = true) {
    tab = 'diffs';
    holdNearCards();
    wanted = path;
    await tick();
    cardOf(path)?.scrollIntoView({ block: 'start' });
    atFile = chosen = pinned = path;
    // The address follows once the page is idle: the file is drawn first (a navigation re-renders the layout)
    writing?.();
    if (address) writing = whenIdle(() => onfile(path));
  }

  // The file gone to stays at the top while the cards around it are made (each as tall as its diff, not its
  // estimate), until the user scrolls, clicks or presses a key in the list, or another jump
  let pinned: string | null = null;
  $effect(() => {
    const el = list;
    void files;
    if (!el) return;
    const unpin = () => (pinned = null);
    const settle = new ResizeObserver(() => {
      if (pinned) cardOf(pinned)?.scrollIntoView({ block: 'start' });
    });
    for (const card of el.querySelectorAll('.review-file-card')) settle.observe(card);
    const events = ['wheel', 'touchstart', 'mousedown', 'keydown'] as const;
    for (const name of events) el.addEventListener(name, unpin, { passive: true });
    return () => {
      settle.disconnect();
      for (const name of events) el.removeEventListener(name, unpin);
    };
  });
  // The address's file, once the list is there
  let opened = false;
  $effect(() => {
    if (opened || !list || !file || !files.some((f) => f.path === file)) return;
    opened = true;
    void showFile(file, false);
  });

  function stepFile(by: number) {
    const i = files.findIndex((f) => f.path === (atFile ?? files[0]?.path));
    const next = files[i + by];
    if (next) void showFile(next.path);
  }

  // The previous and next change, or comment, across the files: the next one below (or above) the last one gone to,
  // or the top of the list; a file not made yet is made first, and its first (or last) one gone to
  const TARGETS = {
    change: { marks: '[data-change]', pending: '.review-file-card[data-pending]', inside: '[data-change]' },
    comment: {
      marks: '.review-file-card .chain',
      pending: '.review-file-card[data-pending][data-comments]',
      inside: '.chain',
    },
  } as const;
  let at: Element | null = null;
  /** Wait for a card's diff to show what is looked for: its sides load first */
  async function marksOf(card: HTMLElement, selector: string) {
    for (let i = 0; i < 120; i++) {
      const marks = card.querySelectorAll(selector);
      if (marks.length) return [...marks];
      await new Promise((r) => requestAnimationFrame(r));
    }
    return [];
  }
  async function jump(back: boolean, kind: keyof typeof TARGETS = 'change') {
    tab = 'diffs';
    holdNearCards();
    pinned = null;
    await tick();
    if (!list) return;
    const target = TARGETS[kind];
    const from = at?.isConnected ? at.getBoundingClientRect().top : list.getBoundingClientRect().top + (back ? 0 : -1);
    // In page order (a rendered split's two sides interleave)
    const marks = [...list.querySelectorAll<HTMLElement>(`${target.marks}, ${target.pending}`)]
      .map((el) => ({ el, top: el.getBoundingClientRect().top }))
      .sort((a, b) => a.top - b.top);
    const above = marks.filter((m) => m.top < from - 2);
    let to: Element | undefined = back ? above.at(-1)?.el : marks.find((m) => m.top > from + 2)?.el;
    if (!to) return;
    if (to.matches('.review-file-card')) {
      wanted = (to as HTMLElement).dataset.file ?? null;
      to.scrollIntoView({ block: 'start' });
      const found = await marksOf(to as HTMLElement, target.inside);
      to = back ? found.at(-1) : found[0];
      if (!to) return;
    }
    for (const old of list.querySelectorAll('.review-current-change')) old.classList.remove('review-current-change');
    to.classList.add('review-current-change');
    to.scrollIntoView({ block: 'center' });
    at = to;
    chosen = null;
    findTop();
  }

  function onkeydown(e: KeyboardEvent) {
    if (tab !== 'diffs' || e.metaKey || e.ctrlKey || e.altKey) return;
    const target = e.target instanceof Element ? e.target : null;
    if (target?.closest('input, textarea, select, [contenteditable="true"]')) return;
    const top = files.find((f) => f.path === (atFile ?? files[0]?.path));
    const act = {
      n: () => jump(false),
      p: () => jump(true),
      c: () => jump(false, 'comment'),
      C: () => jump(true, 'comment'),
      ']': () => stepFile(1),
      '[': () => stepFile(-1),
      v: () => top && toggleViewed(top),
    }[e.key];
    if (!act) return;
    e.preventDefault();
    void act();
  }

  // The splitter: dragged, or moved with the arrow keys
  let split = $state<HTMLElement>();
  /** The width the tree and the diffs share (the window's, before the page is laid out) */
  const pageWidth = () => split?.clientWidth || window.innerWidth;
  function drag(e: PointerEvent) {
    const bar = e.currentTarget as HTMLElement;
    bar.setPointerCapture(e.pointerId);
    const left = split?.getBoundingClientRect().left ?? 0;
    const move = (m: PointerEvent) => (width = treeWidth(m.clientX - left, pageWidth()));
    const up = () => {
      bar.removeEventListener('pointermove', move);
      bar.removeEventListener('pointerup', up);
    };
    bar.addEventListener('pointermove', move);
    bar.addEventListener('pointerup', up);
  }
  function nudge(e: KeyboardEvent) {
    const by = { ArrowLeft: -16, ArrowRight: 16 }[e.key];
    if (!by) return;
    e.preventDefault();
    width = treeWidth(width + by, pageWidth());
  }

  // A chain on the review as a whole: kept, with what was written, until the page says it is saved
  let composing = $state(false);
  let draft = $state('');
  let refused = $state<string | null>(null);
  /** Sent at once, or kept as a draft (`asDraft`, ATM-612) */
  async function saveComment(asDraft = false) {
    const body = draft.trim();
    if (!body) return;
    refused = null;
    const problem =
      asDraft && ondrafts
        ? await ondrafts.comment(null, null, body)
        : await oncomment({ path: null, picked: null, body });
    if (typeof problem === 'string') {
      refused = problem;
      return;
    }
    composing = false;
    draft = '';
  }

  // Poke (ATM-611): a nudge with an optional message; the box closes once it is sent, and keeps it with why if not
  let poking = $state(false);
  let pokeText = $state('');
  let pokeRefused = $state<string | null>(null);
  async function poke() {
    pokeRefused = (await onpoke?.(pokeText.trim())) ?? null;
    if (!pokeRefused) {
      poking = false;
      pokeText = '';
    }
  }

  // Submit: a summary and a decision
  let submitting = $state(false);
  let summary = $state('');
  function submit(decision: 'approve' | 'request_changes' | 'comment') {
    if (decision === 'approve' && approveBlocked) return;
    onsubmit(decision, summary.trim());
    submitting = false;
  }
  // A decision made (the review's `updated` moves): the summary box starts empty; a refresh that changed nothing else
  // keeps what is being written
  let updatedAt = untrack(() => review.updated);
  $effect(() => {
    if (review.updated === updatedAt) return;
    updatedAt = review.updated;
    summary = '';
  });

  // The Comments tab
  let filter = $state<ChainFilter>('all');
  const chainCounts = $derived({
    all: allChains.length,
    open: allChains.filter(isOpen).length,
    resolved: allChains.filter((c) => isResolved(c)).length,
  });
  /** Nothing left open: the decision at hand is to approve */
  const settled = $derived(chainCounts.open === 0);
  /** Why Approve can't be pressed now, as the review's rules would refuse it (ATM-601); null when it can. A review
   * sent back can be approved too (ATM-609) */
  const approveBlocked = $derived.by(() => {
    const drafted = draftedComments(page.drafts);
    const open = chainCounts.open - drafted;
    if (open > 0) return `${open} ${open === 1 ? 'comment is' : 'comments are'} still open: resolve them first`;
    if (drafted > 0)
      return `Your ${drafted === 1 ? 'draft comment' : `${drafted} draft comments`} would be open once sent: send ${drafted === 1 ? 'it' : 'them'} with Comment or Request changes, or remove ${drafted === 1 ? 'it' : 'them'}`;
    return null;
  });
  /** What a decision sends with it (ATM-612): the user's drafts */
  const pendingLabel = $derived(
    pendingDrafts ? `${pendingDrafts} ${pendingDrafts === 1 ? 'draft' : 'drafts'} will be sent with it` : null,
  );
  const listed = $derived(filterChains(allChains, filter));
  /** A chain as the Comments tab shows it: where it was made (a drifted one at its own lines) */
  const asMade = (c: PlacedChain) => ({
    ...c,
    anchor: { ...c.anchor, lines: c.placed ?? c.drifted?.lines ?? c.anchor.lines },
  });

  const tabs = $derived([
    { id: 'overview', label: 'Overview', icon: 'info', count: null, open: 0 },
    { id: 'diffs', label: 'Diffs', icon: 'file-diff', count: files.length, open: 0 },
    // The open ones beside the total (ATM-614): *Comments 7 (2 open)*
    { id: 'comments', label: 'Comments', icon: 'message-square', count: allChains.length, open: chainCounts.open },
  ] as const);
</script>

<svelte:window {onkeydown} />

<article class="review-page" id="review-page" data-review={review.reference} data-tab={tab} aria-busy={busy}>
  <header class="review-top">
    <div class="review-top-row">
      {#if backHref || onback}
        <a
          class="review-back"
          href={backHref ?? '#'}
          title="All reviews"
          onclick={(e) => {
            if (!onback) return;
            e.preventDefault();
            onback();
          }}><Icon name="arrow-left" size={15} /><span>Reviews</span></a
        >
        <span class="review-divider" aria-hidden="true"></span>
      {/if}
      <h1 class="review-title">
        {#if project}<span class="review-project">{project}</span>{/if}
        <span class="review-ref">{review.reference}</span>
        <span class="review-name">{review.title}</span>
      </h1>
      <span class="review-status status-{review.status}" data-status={review.status}
        >{STATUS_LABELS[review.status]}</span
      >
      {#if page.needs_you}<span class="review-needs">Needs you</span>{/if}
      {#if live && view.now}<span
          class="review-following"
          id="review-following"
          title="The change as it is being made; a comment keeps what you saw as a checkpoint"
          >Following the working copy</span
        >{/if}
      {#if !ended && onpoke}
        <!-- A poke (ATM-611): a nudge its agent hears and answers, saying where the review stands -->
        <div class="review-submit-anchor">
          <button
            class="review-button"
            id="review-poke"
            aria-expanded={poking}
            title="Nudge its agent: it answers, saying where the review stands"
            onclick={() => (poking = !poking)}><Icon name="bell" size={15} /> Poke</button
          >
          {#if poking}
            <form
              class="review-submit review-poke"
              aria-label="Poke"
              onsubmit={(e) => {
                e.preventDefault();
                void poke();
              }}
            >
              <textarea
                id="review-poke-body"
                bind:value={pokeText}
                rows="2"
                placeholder="A message (optional)"
                aria-label="Poke message"
                {@attach focusOnShow}></textarea>
              {#if pokeRefused}<p class="review-poke-problem" role="alert">{pokeRefused}</p>{/if}
              <div class="review-actions">
                <button type="button" class="review-button" onclick={() => (poking = false)}>Cancel</button>
                <button type="submit" class="review-button primary" disabled={busy}>Poke</button>
              </div>
            </form>
          {/if}
        </div>
      {/if}
      {#if !ended}
        <div class="review-submit-anchor">
          {#if settled && !approveBlocked}
            <div class="review-split-button">
              <button
                class="review-button primary approve"
                id="review-approve"
                disabled={busy}
                title="Every comment is resolved"
                onclick={() => submit('approve')}><Icon name="check" size={15} /> Approve</button
              >
              <button
                class="review-button primary approve more"
                aria-label="Other decisions"
                aria-expanded={submitting}
                onclick={() => (submitting = !submitting)}><Icon name="chevron-down" size={15} /></button
              >
            </div>
          {:else}
            <button
              class="review-button primary review-submit-toggle"
              aria-expanded={submitting}
              onclick={() => (submitting = !submitting)}
              >{sentBack && pendingDrafts ? 'Publish comments' : 'Submit review'}{#if pendingDrafts}<span
                  class="review-pending"
                  id="review-pending"
                  title="Your drafts, sent with your review">{pendingDrafts}</span
                >{/if}
              <Icon name="chevron-down" size={15} /></button
            >
          {/if}
          {#if submitting}
            <section class="review-submit" aria-label="Submit">
              <h2 class="review-submit-title">Finish your review</h2>
              {#if pendingLabel}<p class="review-pending-note" id="review-pending-note">
                  <Icon name="pencil" size={13} />
                  {pendingLabel}
                </p>{/if}
              <textarea
                id="review-summary-body"
                bind:value={summary}
                rows="4"
                placeholder={pendingDrafts
                  ? 'A summary of your review (optional)'
                  : 'A summary of your review (needed to comment)'}
                aria-label="Summary"
                {@attach focusOnShow}></textarea>
              <div class="review-actions">
                <button
                  class="review-button"
                  data-decision="comment"
                  disabled={busy || (!summary.trim() && !pendingDrafts)}
                  onclick={() => submit('comment')}>{sentBack ? 'Publish comments' : 'Comment'}</button
                >
                {#if live}
                  <!-- A live review isn't sent back: its author ends it with a round; a reviewer approves or closes it -->
                  {#if onclose}
                    <button
                      class="review-button danger"
                      data-decision="close"
                      disabled={busy || !summary.trim()}
                      title="End it without approval; the summary says why"
                      onclick={() => {
                        onclose(summary.trim());
                        submitting = false;
                      }}>Close review</button
                    >
                  {/if}
                {:else if !sentBack}
                  <!-- Sent back already, it isn't sent back again (CR-2): more words are published as comments -->
                  <button
                    class="review-button danger"
                    data-decision="request_changes"
                    disabled={busy}
                    onclick={() => submit('request_changes')}>Request changes</button
                  >
                {/if}
                <!-- Disabled, saying why, while the rules would refuse it (ATM-601): the reason on the wrapper, as a
                     disabled button shows no tooltip in every engine, and as its description for screen readers -->
                <span class="review-approve-wrap" title={approveBlocked ?? undefined}>
                  <button
                    class="review-button primary"
                    data-decision="approve"
                    disabled={busy || !!approveBlocked}
                    aria-describedby={approveBlocked ? 'review-approve-why' : undefined}
                    onclick={() => submit('approve')}><Icon name="check" size={14} /> Approve</button
                  >
                </span>
              </div>
              {#if approveBlocked}
                <p class="review-approve-why" id="review-approve-why">
                  <Icon name="info" size={13} />
                  {approveBlocked}
                </p>
              {/if}
            </section>
          {/if}
        </div>
      {/if}
    </div>
    <div class="review-top-row review-bar">
      <div class="review-tabs" role="tablist" aria-label="Review">
        {#each tabs as t (t.id)}
          <button
            role="tab"
            id="review-tab-{t.id}"
            aria-selected={tab === t.id}
            onclick={() => (tab = t.id)}
            aria-label={t.open ? `${t.label}: ${t.count}, ${t.open} open` : undefined}
            ><Icon name={t.icon} size={15} /><span class="review-tab-label">{t.label}</span>{#if t.count !== null}<span
                class="review-count">{t.count}</span
              >{/if}{#if t.open}<span class="review-count-open">({t.open} open)</span>{/if}</button
          >
        {/each}
      </div>
      {#if tab === 'diffs'}
        <div class="review-tools">
          <label class="review-select">
            <span>Round</span>
            <select
              id="review-round"
              value={view.now ? 'now' : String(view.round)}
              onchange={(e) => {
                const value = e.currentTarget.value;
                onround(value === 'now' ? null : Number(value), null);
              }}
            >
              {#if live}<option value="now">Now, as it is being made</option>{/if}
              {#each rounds as n (n)}<option value={String(n)}>{n} of {rounds.length}</option>{/each}
            </select>
          </label>
          <select
            class="review-select-bare"
            id="review-against"
            aria-label="Showing"
            value={view.against ?? ''}
            onchange={(e) => onround(view.round, e.currentTarget.value ? Number(e.currentTarget.value) : null)}
          >
            <option value="">Whole change</option>
            {#each rounds.filter((n) => n < view.round) as n (n)}<option value={n}>Since round {n}</option>{/each}
          </select>
          <span class="review-progress" title="{viewedCount} of {files.length} files viewed">
            <span class="review-meter"
              ><span style:width="{files.length ? (viewedCount / files.length) * 100 : 0}%"></span></span
            >
            <span class="review-viewed-count">{viewedCount} of {files.length} viewed</span>
          </span>
          <span class="review-tool-divider" aria-hidden="true"></span>
          <div class="review-icons" role="group" aria-label="Changes">
            <Icon name="file-diff" size={14} />
            <button
              class="review-icon-button"
              id="review-prev-change"
              aria-label="Previous change"
              title="Previous change (p)"
              onclick={() => jump(true)}><Icon name="chevron-up" size={16} /></button
            >
            <button
              class="review-icon-button"
              id="review-next-change"
              aria-label="Next change"
              title="Next change (n)"
              onclick={() => jump(false)}><Icon name="chevron-down" size={16} /></button
            >
          </div>
          <div class="review-icons" role="group" aria-label="Comments">
            <Icon name="message-square" size={14} />
            <button
              class="review-icon-button"
              id="review-prev-comment"
              aria-label="Previous comment"
              title="Previous comment (Shift C)"
              onclick={() => jump(true, 'comment')}><Icon name="chevron-up" size={16} /></button
            >
            <button
              class="review-icon-button"
              id="review-next-comment"
              aria-label="Next comment"
              title="Next comment (c)"
              onclick={() => jump(false, 'comment')}><Icon name="chevron-down" size={16} /></button
            >
          </div>
          <span class="review-tool-divider" aria-hidden="true"></span>
          <div class="review-segmented" role="group" aria-label="Layout">
            <button aria-pressed={mode === 'split'} aria-label="Split" title="Split" onclick={() => (mode = 'split')}
              ><Icon name="columns-2" size={15} /></button
            >
            <button
              aria-pressed={mode === 'unified'}
              aria-label="Unified"
              title="Unified"
              onclick={() => (mode = 'unified')}><Icon name="rows-2" size={15} /></button
            >
          </div>
          <div class="review-segmented" role="group" aria-label="Lines">
            <span class="review-wrap-hint" title={mode === 'split' ? WRAP_IN_SPLIT : undefined}>
              <button
                id="review-wrap"
                aria-pressed={wrap && mode === 'unified'}
                aria-label="Wrap"
                title={mode === 'split' ? WRAP_IN_SPLIT : 'Wrap long lines'}
                disabled={mode === 'split'}
                onclick={() => (wrap = !wrap)}><Icon name="wrap-text" size={15} /></button
              >
            </span>
          </div>
          {#if hasMarkdown}
            <div class="review-segmented" role="group" aria-label="View">
              <button
                aria-pressed={rendered === 'source'}
                aria-label="Source"
                title="Source"
                onclick={() => (rendered = 'source')}><Icon name="code" size={15} /></button
              >
              <button
                aria-pressed={rendered === 'rendered'}
                aria-label="Rendered"
                title="Rendered"
                onclick={() => (rendered = 'rendered')}><Icon name="eye" size={15} /></button
              >
            </div>
          {/if}
        </div>
      {/if}
    </div>
  </header>

  {#if problem}<p class="review-problem" role="alert">{problem}</p>{/if}

  {#if tab === 'overview'}
    <div class="review-overview" role="tabpanel" aria-labelledby="review-tab-overview">
      <dl class="review-facts">
        <div class="review-fact">
          <dt><Icon name="user" size={13} /> Author</dt>
          <dd>{review.author ?? 'nobody yet'}</dd>
        </div>
        <div class="review-fact">
          <dt><Icon name="circle-check" size={13} /> Decided by</dt>
          <dd>{review.reviewers.length ? review.reviewers.join(', ') : 'anyone but its author'}</dd>
        </div>
        {#if page.branch}
          <div class="review-fact">
            <dt><Icon name="git-branch" size={13} /> Branch</dt>
            <dd class="review-branch">{page.branch}</dd>
          </div>
        {/if}
        {#if review.tasks.length}
          <div class="review-fact">
            <dt><Icon name="list-todo" size={13} /> Tasks</dt>
            <dd>
              {#each review.tasks as task (task)}
                {#if taskHref}<a
                    class="review-task"
                    href={taskHref(task)}
                    data-task={task}
                    onclick={(e) => openTask(e, task)}
                    onauxclick={(e) => e.button === 1 && openTask(e, task)}>{task}</a
                  >{:else}<span class="review-task">{task}</span>{/if}
              {/each}
            </dd>
          </div>
        {/if}
        <div class="review-fact">
          <dt><Icon name="file-diff" size={13} /> Changes</dt>
          <dd>
            {files.length}
            {files.length === 1 ? 'file' : 'files'}
            <span class="added">+{totals.added}</span>
            <span class="removed">−{totals.removed}</span>
          </dd>
        </div>
        <div class="review-fact">
          <dt><Icon name="message-square" size={13} /> Comments</dt>
          <dd>{chainCounts.open} open · {chainCounts.resolved} resolved</dd>
        </div>
      </dl>

      <div class="review-overview-body">
        <div class="review-main">
          <section class="review-card review-summary" aria-label="Description">
            <h2 class="review-card-title">Description</h2>
            {#if page.summary.trim()}<CompMd text={page.summary} onopen={openLink} {tasks} />{:else}<p
                class="review-empty"
              >
                No description.
              </p>{/if}
          </section>
          <section class="review-card" aria-label="Rounds">
            <h2 class="review-card-title">Rounds</h2>
            <ol class="review-round-list">
              {#each [...page.rounds].reverse() as r (r.number)}
                <li>
                  <button
                    class="review-link"
                    onclick={() => {
                      onround(r.number, null);
                      tab = 'diffs';
                    }}>Round {r.number}</button
                  >
                  <span class="review-round-facts"
                    >{r.files.length}
                    {r.files.length === 1 ? 'file' : 'files'}{r.submodule ? ` in the submodule ${r.submodule}` : ''} · {r.by}{r.at
                      ? ` · ${when(r.at)}`
                      : ''}</span
                  >
                  {#if r.number === view.round}<span class="review-round-shown">shown</span>{/if}
                </li>
              {/each}
            </ol>
          </section>
        </div>

        <aside class="review-conversation" aria-label="Comments on the review">
          <h2 class="review-card-title">Conversation</h2>
          {#each reviewChains(allChains) as chain (chain.id)}<ChainBox {chain} {onreply} {onresolve} />{/each}
          {#if !reviewChains(allChains).length && !composing}
            <p class="review-empty">No comments on the review as a whole yet.</p>
          {/if}
          {#if composing}
            <form
              class="review-composer"
              onsubmit={(e) => {
                e.preventDefault();
                void saveComment(!!ondrafts && pendingDrafts > 0);
              }}
            >
              <textarea
                id="review-comment-body"
                bind:value={draft}
                rows="4"
                aria-label="Comment"
                placeholder="Leave a comment on the review…"
                {@attach focusOnShow}></textarea>
              {#if refused}<p class="review-refused" role="alert">Not saved: {refused}</p>{/if}
              <div class="review-actions">
                <ComposerActions
                  buttonClass="review-button"
                  ready={!!draft.trim() && !busy}
                  drafting={!!ondrafts && pendingDrafts > 0}
                  now={() => void saveComment(false)}
                  oncancel={() => (composing = false)}
                  ondraft={ondrafts ? () => void saveComment(true) : undefined}
                />
              </div>
            </form>
          {:else}
            <button class="review-button review-add-comment" onclick={() => (composing = true)}
              ><Icon name="message-square" size={14} /> Comment on the review</button
            >
          {/if}
        </aside>
      </div>
    </div>
  {:else if tab === 'diffs'}
    <div
      class="review-split"
      role="tabpanel"
      aria-labelledby="review-tab-diffs"
      bind:this={split}
      style:--tree="{width}px"
    >
      <nav class="review-files" aria-label="Files">
        <div class="review-files-head">
          <span>Files</span>
          <span class="review-files-total"
            ><span class="added">+{totals.added}</span> <span class="removed">−{totals.removed}</span></span
          >
          {#if treeFolders(tree).length}
            <span class="review-files-folds">
              <button
                class="review-files-fold"
                id="review-collapse-all"
                title="Collapse all folders"
                aria-label="Collapse all folders"
                onclick={() => fold(new Set(treeFolders(tree)))}><Icon name="chevrons-down-up" size={13} /></button
              >
              <button
                class="review-files-fold"
                id="review-expand-all"
                title="Expand all folders"
                aria-label="Expand all folders"
                onclick={() => fold(new Set())}><Icon name="chevrons-up-down" size={13} /></button
              >
            </span>
          {/if}
        </div>
        {#if !files.length}<p class="review-empty">No files changed here.</p>{/if}
        <ul role="tree" aria-label="Files changed">
          {#each shownTree as row (row.kind === 'folder' ? `d:${row.path}` : `f:${row.file.path}`)}
            {#if row.kind === 'folder'}
              {@const shut = collapsed.has(row.path)}
              <li
                class="review-folder"
                role="treeitem"
                aria-expanded={!shut}
                aria-selected="false"
                aria-level={row.depth + 1}
                data-folder={row.path}
                style:--depth={row.depth}
              >
                <button
                  class="review-folder-toggle"
                  aria-label="{shut ? 'Expand' : 'Collapse'} {row.path}"
                  onclick={() => toggleFolder(row.path)}
                  onkeydown={(e) => folderKeys(e, row.path)}
                  ><Icon name={shut ? 'chevron-right' : 'chevron-down'} size={13} /><Icon
                    name="folder"
                    size={13}
                  />{row.name}</button
                >
                {#if shut}<span class="review-folder-sum">{folderSum(row.path)}</span>{/if}
              </li>
            {:else}
              {@const counts = lineCounts(row.file.patch)}
              <li
                class="review-file"
                role="treeitem"
                aria-selected={row.file.path === atFile}
                aria-level={row.depth + 1}
                class:current={row.file.path === atFile}
                class:viewed={isViewed(row.file)}
                style:--depth={row.depth}
              >
                <input
                  type="checkbox"
                  aria-label="Viewed {row.file.path}"
                  checked={isViewed(row.file)}
                  onchange={() => toggleViewed(row.file)}
                />
                <button class="review-file-name" data-file={row.file.path} onclick={() => showFile(row.file.path)}>
                  <span class="review-file-status status-{row.file.status}">{statusLetter[row.file.status]}</span>
                  {row.name}
                </button>
                <span class="review-file-counts">
                  {#if row.file.binary}bin{:else}<span class="added">+{counts.added}</span>
                    <span class="removed">−{counts.removed}</span>{/if}
                </span>
              </li>
            {/if}
          {/each}
        </ul>
      </nav>
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (a splitter is
           dragged, and moved with the arrow keys) -->
      <div
        class="review-splitter"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize the files"
        aria-valuenow={width}
        aria-valuemin={TREE_WIDTH.least}
        tabindex="0"
        onpointerdown={drag}
        onkeydown={nudge}
      ></div>
      <div class="review-diffs" bind:this={list} {onscroll}>
        {#each files as f (f.path)}
          <ReviewFile
            file={f}
            chains={chainsOf(allChains, f)}
            drifted={driftedOf(allChains, f)}
            before={sideOf(f.before)}
            after={sideOf(f.after)}
            viewed={isViewed(f)}
            folded={folded(f)}
            {mode}
            view={rendered}
            wrap={wrap && mode === 'unified'}
            {busy}
            {docStyle}
            root={list}
            wanted={f.path === wanted}
            onfold={() => (folds = { ...folds, [f.path]: !folded(f) })}
            onviewed={() => toggleViewed(f)}
            oncomment={(picked, body) => oncomment({ path: f.path, picked, body })}
            {onreply}
            {onresolve}
            onround={(round) => onround(round, null)}
          />
        {/each}
      </div>
    </div>
  {:else}
    <div class="review-comments" role="tabpanel" aria-labelledby="review-tab-comments">
      <div class="review-segmented review-filter" role="group" aria-label="Show">
        {#each [['all', 'All'], ['open', 'Open'], ['resolved', 'Resolved']] as const as [id, label] (id)}
          <button aria-pressed={filter === id} data-filter={id} onclick={() => (filter = id)}
            >{label} <span class="review-count">{chainCounts[id]}</span></button
          >
        {/each}
      </div>
      {#if !listed.length}
        <p class="review-empty">
          {filter === 'all' ? 'No comments yet.' : filter === 'open' ? 'Nothing open.' : 'Nothing resolved yet.'}
        </p>
      {/if}
      {#each listed as chain (chain.id)}
        <ChainBox
          chain={asMade(chain)}
          detailed
          {onreply}
          {onresolve}
          onwhere={chain.anchor.path ? () => showFile(chain.anchor.path!) : () => (tab = 'overview')}
        />
      {/each}
    </div>
  {/if}
</article>

<style>
  .review-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--bg);
    color: var(--text);
    font-size: var(--text-md);
  }
  /* The top bar: the title and the decision, then the tabs and the Diffs tab's tools */
  /* In the page's background, as a project's header: the app's selected tab, in it too, joins it (ATM-608) */
  .review-top {
    flex: none;
    border-bottom: 1px solid var(--border);
    background: var(--bg);
  }
  .review-top-row {
    display: flex;
    align-items: center;
    gap: var(--space-5);
    padding: var(--space-6) var(--space-8) 0;
  }
  .review-bar {
    flex-wrap: wrap;
    padding-top: var(--space-4);
    padding-bottom: 0;
  }
  .review-back {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-4);
    border-radius: var(--radius-md);
    color: var(--text-muted);
    font-size: var(--text-sm);
    text-decoration: none;
    white-space: nowrap;
  }
  .review-back:hover {
    color: var(--text);
    background: var(--surface2);
  }
  .review-divider {
    width: 1px;
    height: var(--space-8);
    background: var(--border);
  }
  .review-title {
    display: flex;
    align-items: baseline;
    gap: var(--space-4);
    min-width: 0;
    margin: 0;
    font-size: var(--text-2xl);
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .review-project,
  .review-ref {
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    font-size: var(--text-lg);
    font-weight: 500;
    color: var(--text-muted);
    white-space: nowrap;
  }
  .review-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .review-status,
  .review-needs {
    padding: var(--space-1) var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    font-size: var(--text-xs);
    font-weight: 600;
    white-space: nowrap;
    color: var(--text-muted);
  }
  .review-status.status-approved {
    color: var(--green);
    border-color: color-mix(in srgb, var(--green) 50%, transparent);
    background: color-mix(in srgb, var(--green) 10%, transparent);
  }
  .review-status.status-changes_requested {
    color: var(--red);
    border-color: color-mix(in srgb, var(--red) 50%, transparent);
    background: color-mix(in srgb, var(--red) 10%, transparent);
  }
  .review-status.status-waiting,
  .review-status.status-live {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 50%, transparent);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .review-needs {
    color: var(--purple);
    border-color: color-mix(in srgb, var(--purple) 50%, transparent);
    background: color-mix(in srgb, var(--purple) 10%, transparent);
  }
  .review-following {
    font-size: var(--text-xs);
    color: var(--text-muted);
    white-space: nowrap;
  }
  /* Buttons, as the dashboard's dialogs draw theirs; a comment box's are ComposerActions' (ATM-612), which these reach */
  .review-page :global(.review-button) {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-6);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface2);
    color: var(--text);
    font: inherit;
    font-size: var(--text-md);
    font-weight: 500;
    white-space: nowrap;
    cursor: pointer;
  }
  .review-page :global(.review-button:hover:not(:disabled)) {
    border-color: color-mix(in srgb, var(--text-muted) 60%, var(--border));
  }
  .review-page :global(.review-button.primary) {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .review-button.primary.approve {
    background: var(--green);
    border-color: var(--green);
  }
  .review-button.danger {
    color: var(--red);
  }
  .review-page :global(.review-button:disabled) {
    opacity: 0.5;
    cursor: default;
  }
  /* The drafts a decision sends (ATM-612): counted on the button, said in its box */
  .review-pending {
    min-width: var(--space-8);
    padding: 0 var(--space-2);
    border-radius: var(--radius-pill);
    background: color-mix(in srgb, var(--on-accent) 25%, transparent);
    font-size: var(--text-xs);
    font-weight: 700;
  }
  .review-pending-note {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0;
    color: var(--text-muted);
    font-size: var(--text-sm);
  }
  .review-submit-anchor {
    position: relative;
    margin-left: auto;
  }
  .review-split-button {
    display: flex;
  }
  .review-split-button .review-button:first-child {
    border-radius: var(--radius-md) 0 0 var(--radius-md);
  }
  .review-split-button .more {
    padding-left: var(--space-3);
    padding-right: var(--space-3);
    border-left: 1px solid color-mix(in srgb, var(--on-accent) 25%, transparent);
    border-radius: 0 var(--radius-md) var(--radius-md) 0;
  }
  .review-submit {
    position: absolute;
    right: 0;
    top: calc(100% + var(--space-3));
    z-index: 10;
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    width: min(28rem, 90vw);
    padding: var(--space-7);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    box-shadow: 0 12px 36px color-mix(in srgb, var(--overlay) 45%, transparent);
  }
  .review-submit-title,
  .review-card-title {
    margin: 0;
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-muted);
  }
  /* The tabs */
  .review-tabs {
    display: flex;
    gap: var(--space-2);
  }
  .review-tabs button {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-6) var(--space-5);
    border: none;
    border-bottom: 2px solid transparent;
    background: none;
    color: var(--text-muted);
    font: inherit;
    font-size: var(--text-md);
    font-weight: 500;
    cursor: pointer;
  }
  .review-tabs button:hover {
    color: var(--text);
  }
  .review-tabs button[aria-selected='true'] {
    color: var(--text);
    font-weight: 600;
    border-bottom-color: var(--accent);
  }
  .review-count {
    min-width: var(--space-8);
    padding: 0 var(--space-3);
    border-radius: var(--radius-pill);
    background: var(--surface2);
    color: var(--text-muted);
    font-size: var(--text-xs);
    font-weight: 600;
    text-align: center;
  }
  /* The open comments beside the total, quieter (ATM-614) */
  .review-count-open {
    color: var(--text-muted);
    font-size: var(--text-xs);
  }
  /* The Diffs tab's tools */
  .review-tools {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-5);
    margin-left: auto;
    padding-bottom: var(--space-4);
    font-size: var(--text-sm);
    color: var(--text-muted);
  }
  .review-select {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .review-tools select {
    height: 28px;
    padding: 0 var(--space-4);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg);
    color: var(--text);
    font: inherit;
    font-size: var(--text-sm);
  }
  .review-progress {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }
  .review-meter {
    width: 64px;
    height: var(--space-3);
    border-radius: var(--radius-pill);
    background: var(--surface2);
    overflow: hidden;
  }
  .review-meter span {
    display: block;
    height: 100%;
    background: var(--green);
  }
  .review-tool-divider {
    width: 1px;
    height: var(--space-8);
    background: var(--border);
  }
  .review-icons {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: 0 var(--space-1) 0 var(--space-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg);
    color: var(--text-muted);
  }
  .review-icon-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: none;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--text-muted);
    cursor: pointer;
  }
  .review-icon-button:hover {
    color: var(--text);
    background: var(--surface2);
  }
  .review-segmented {
    display: flex;
    padding: var(--space-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg);
  }
  .review-segmented button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    min-width: 28px;
    height: 24px;
    padding: 0 var(--space-3);
    border: none;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--text-muted);
    font: inherit;
    font-size: var(--text-sm);
    cursor: pointer;
  }
  .review-segmented button:hover {
    color: var(--text);
  }
  .review-segmented button[aria-pressed='true'] {
    background: var(--surface2);
    color: var(--text);
    box-shadow: 0 1px 2px color-mix(in srgb, var(--overlay) 25%, transparent);
  }
  .review-problem {
    margin: var(--space-5) var(--space-8) 0;
    padding: var(--space-4) var(--space-6);
    border: 1px solid color-mix(in srgb, var(--red) 40%, transparent);
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--red) 10%, transparent);
    color: var(--red);
  }
  /* Overview: the facts across the top, the description and rounds, the conversation beside them */
  .review-overview,
  .review-comments {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: var(--space-8);
  }
  .review-facts {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
    margin: 0 0 var(--space-8);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
    overflow: hidden;
  }
  .review-fact {
    padding: var(--space-5) var(--space-7);
  }
  .review-fact + .review-fact {
    border-left: 1px solid var(--border);
  }
  .review-fact dt {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-bottom: var(--space-2);
    color: var(--text-muted);
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.03em;
    text-transform: uppercase;
  }
  .review-fact dd {
    margin: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--space-4);
    font-size: var(--text-lg);
  }
  .review-branch {
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    font-size: var(--text-md) !important;
  }
  .review-task {
    padding: 0 var(--space-4);
    border-radius: var(--radius-pill);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent);
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    font-size: var(--text-sm);
    text-decoration: none;
  }
  .review-overview-body {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(300px, 420px);
    gap: var(--space-8);
    align-items: start;
  }
  .review-main {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    min-width: 0;
  }
  .review-card {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    padding: var(--space-7) var(--space-8);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
  }
  .review-summary :global(.compmd) {
    font-size: var(--text-lg);
    line-height: 1.6;
  }
  .review-round-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .review-round-list li {
    display: flex;
    align-items: baseline;
    gap: var(--space-5);
  }
  .review-round-facts {
    color: var(--text-muted);
  }
  .review-round-shown {
    padding: 0 var(--space-4);
    border-radius: var(--radius-pill);
    background: var(--surface2);
    color: var(--text-muted);
    font-size: var(--text-xs);
  }
  .review-conversation {
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    min-width: 0;
  }
  .review-conversation :global(.chain),
  .review-comments :global(.chain) {
    margin-left: 0;
    margin-right: 0;
  }
  .review-link {
    padding: 0;
    border: none;
    background: none;
    color: var(--accent);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  .review-link:hover {
    text-decoration: underline;
  }
  .review-add-comment {
    align-self: flex-start;
  }
  .review-composer {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .review-refused {
    margin: 0;
    color: var(--red);
    font-size: var(--text-sm);
  }
  /* Diffs: the tree, the splitter, the list */
  .review-split {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: var(--tree) 5px minmax(0, 1fr);
  }
  .review-files {
    overflow: auto;
    padding: 0 0 var(--space-6);
    font-size: var(--text-sm);
    background: var(--surface);
  }
  .review-files-head {
    position: sticky;
    top: 0;
    display: flex;
    justify-content: space-between;
    padding: var(--space-5) var(--space-6);
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-muted);
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .review-files-total {
    letter-spacing: 0;
    text-transform: none;
  }
  .review-files ul {
    list-style: none;
    margin: var(--space-3) 0 0;
    padding: 0;
  }
  .review-folder,
  .review-file {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin: 0 var(--space-3);
    padding: var(--space-2) var(--space-4) var(--space-2) calc(var(--space-4) + var(--depth) * var(--space-7));
    border-radius: var(--radius-md);
  }
  .review-folder {
    color: var(--text-muted);
    flex-wrap: wrap;
  }
  .review-folder-toggle {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .review-folder-toggle:hover {
    color: var(--text);
  }
  .review-folder-sum {
    font-size: var(--text-xs);
  }
  .review-files-folds {
    display: inline-flex;
    gap: var(--space-1);
    margin-left: var(--space-4);
  }
  .review-files-fold {
    display: inline-flex;
    padding: var(--space-1);
    border: none;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--text-muted);
    cursor: pointer;
  }
  .review-files-fold:hover {
    background: var(--surface2);
    color: var(--text);
  }
  .review-file:hover {
    background: var(--surface2);
  }
  .review-file.current {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .review-file.viewed .review-file-name {
    color: var(--text-muted);
  }
  .review-file input {
    margin: 0;
    accent-color: var(--green);
  }
  .review-file-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 0;
    border: none;
    background: none;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .review-file-status {
    display: inline-block;
    width: 1.2em;
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    font-weight: 700;
    color: var(--yellow);
  }
  .review-file-status.status-added {
    color: var(--green);
  }
  .review-file-status.status-deleted {
    color: var(--red);
  }
  .review-file-status.status-renamed {
    color: var(--accent);
  }
  .review-file-counts {
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    font-size: var(--text-2xs);
    color: var(--text-muted);
    white-space: nowrap;
  }
  .added {
    color: var(--green);
  }
  .removed {
    color: var(--red);
  }
  .review-splitter {
    cursor: col-resize;
    border-left: 1px solid var(--border);
    background: transparent;
    touch-action: none;
    transition: background 0.12s ease;
  }
  .review-splitter:hover,
  .review-splitter:focus-visible {
    background: color-mix(in srgb, var(--accent) 35%, transparent);
    outline: none;
  }
  .review-diffs {
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-7);
    padding: var(--space-7) var(--space-8) 50vh;
    background: var(--bg);
  }
  .review-diffs :global(.review-current-change) {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  /* Comments */
  .review-comments {
    max-width: 960px;
    width: 100%;
    margin: 0 auto;
    box-sizing: border-box;
  }
  .review-filter {
    align-self: flex-start;
    display: inline-flex;
    margin-bottom: var(--space-7);
  }
  .review-filter button {
    height: 26px;
    padding: 0 var(--space-5);
  }
  textarea {
    width: 100%;
    box-sizing: border-box;
    padding: var(--space-4) var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg);
    color: var(--text);
    font: inherit;
    resize: vertical;
  }
  textarea:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 20%, transparent);
  }
  .review-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-4);
  }
  .review-approve-wrap {
    display: inline-flex;
  }
  .review-approve-wrap:has(button:disabled) {
    cursor: not-allowed;
  }
  .review-approve-why {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-2);
    margin: var(--space-3) 0 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .review-empty {
    margin: 0;
    color: var(--text-muted);
  }
  @media (max-width: 960px) {
    .review-overview-body {
      grid-template-columns: 1fr;
    }
    .review-conversation {
      position: static;
    }
  }
  @media (max-width: 720px) {
    .review-split {
      grid-template-columns: 1fr;
      grid-template-rows: auto 1fr;
    }
    .review-splitter {
      display: none;
    }
    .review-files {
      max-height: 30vh;
    }
  }
</style>
