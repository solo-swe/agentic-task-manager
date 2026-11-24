<script lang="ts">
  // The Documents item's page (docs/documents-design.md §4, ATM-766): the project's documents, its Markdown files where
  // git has them, as a tree by folder (the first view) or by last change (Recent). Each row is a document's title, its
  // file name and how long ago it changed, *new* when git doesn't track it yet. The search is in the tab's header line,
  // where a page's own search is: it filters the list in the page, or, when atm cut the list at 500, asks atm (`?q=`),
  // which searches before it cuts. The view and the search are in the address; the open folders are kept per project
  // in the preferences. A click opens the document in the viewer over this page (`?doc=`, a step Back undoes);
  // ⌘-click a new tab. Asked again now and then while in view, by its `ETag`.
  import { onDestroy, untrack } from 'svelte';
  import { loadDocuments, type DocumentEntry, type DocumentsListing } from '../api/documents';
  import { inApp, type Validators } from '../api/http';
  import {
    byRecent,
    documentsListOf,
    fileName,
    matching,
    treeOf,
    withDocumentsList,
    type DocumentsView,
    type Folder,
  } from '../document-list';
  import { withDoc } from '../documents';
  import { formatAgoOrDate } from '../format';
  import { latest } from '../latest';
  import { currentAddress, navigate } from '../navigation';
  import { getPref, setPref } from '../prefs';
  import { offerReviewSearch } from '../reviews/search.svelte';
  import { dashboard } from '../state.svelte';
  import { inBackground, tabActions, wantsNewTab } from '../tab-actions';
  import Icon from '../ui/Icon.svelte';

  const project = $derived(dashboard.projectId);
  const list = $derived(documentsListOf(dashboard.address));
  const tabs = tabActions();

  // The whole listing (every document atm lists, at most 500), asked again by its validators
  let listing = $state.raw<DocumentsListing | null>(null);
  let problem = $state<string | null>(null);
  let validators: Validators | undefined;
  let listed: string | null = null;
  const loads = latest();

  async function load() {
    const id = project;
    if (!id) return;
    // Another project: asked afresh, not by the last one's validators
    if (id !== listed) [validators, listing] = [undefined, null];
    listed = id;
    const ask = loads.ask();
    try {
      const result = await loadDocuments(id, '', undefined, validators);
      if (!ask.current()) return;
      problem = null;
      if (!result.unchanged) [listing, validators] = [result.listing, result.validators];
    } catch (e) {
      if (ask.current()) problem = `The documents couldn’t be read: ${(e as Error).message}`;
    } finally {
      ask.done();
    }
  }
  $effect(() => {
    void project;
    untrack(() => void load());
  });
  // Asked again every 10 s while the page is in view: a document written or committed shows
  const timer = setInterval(() => document.visibilityState === 'visible' && void load(), 10_000);
  onDestroy(() => clearInterval(timer));

  // A search when atm cut the list at 500: atm's answer for it, which searches every document
  let searched = $state.raw<{ q: string; listing: DocumentsListing } | null>(null);
  const searches = latest();
  async function searchAtm(id: string, q: string) {
    const ask = searches.ask();
    try {
      const result = await loadDocuments(id, q);
      if (ask.current() && !result.unchanged) searched = { q, listing: result.listing };
    } catch (e) {
      if (ask.current()) problem = `The search couldn’t be made: ${(e as Error).message}`;
    } finally {
      ask.done();
    }
  }
  const cut = $derived(!!listing && listing.more > 0);
  $effect(() => {
    const [id, q] = [project, list.q];
    if (!id || !q || !cut) return;
    untrack(() => {
      if (searched?.q !== q) void searchAtm(id, q);
    });
  });

  // What is shown: the documents the search keeps (atm's answer when the list was cut), or none while it is asked
  const shown = $derived.by((): DocumentEntry[] | null => {
    if (!listing) return null;
    if (!list.q) return listing.documents;
    if (cut) return searched?.q === list.q ? searched.listing.documents : null;
    return matching(listing.documents, list.q);
  });
  // The tree: with a search, its matches, every folder open; else every document, the submodules among the folders
  const tree = $derived(shown ? treeOf(shown, list.q ? [] : (listing?.submodules ?? [])) : null);
  const recent = $derived(shown ? byRecent(shown) : []);

  // The open folders, per project, kept in the preferences
  function openOf(id: string | null): ReadonlySet<string> {
    const kept = id ? getPref('documentFolders', {})[id] : undefined;
    return new Set(Array.isArray(kept) ? kept.filter((p): p is string => typeof p === 'string') : []);
  }
  let open = $state.raw<ReadonlySet<string>>(new Set());
  $effect(() => {
    const id = project;
    untrack(() => (open = openOf(id)));
  });
  function toggle(folder: Folder) {
    if (!project) return;
    // A new set each time, never changed in place: `open` is read as a whole ($state.raw)
    const next = open.has(folder.path) ? [...open].filter((p) => p !== folder.path) : [...open, folder.path];
    open = new Set(next);
    setPref('documentFolders', { ...getPref('documentFolders', {}), [project]: next });
  }
  const isOpen = (folder: Folder) => !!list.q || open.has(folder.path);

  // The search, in the tab's header line (Header shows the field while it is offered)
  $effect(() =>
    offerReviewSearch({
      get q() {
        return list.q;
      },
      search: (q) => navigate(withDocumentsList(currentAddress(), { ...list, q }), { replace: true }),
      placeholder: 'Search documents',
      title: 'Search the documents’ titles and paths (/)',
    }),
  );

  function show(view: DocumentsView) {
    navigate(withDocumentsList(currentAddress(), { ...list, view }));
  }

  /** A document over this page, in the viewer: in place (a step Back undoes), or ⌘-click a new tab */
  const hrefOf = (path: string) => withDoc(currentAddress(), { path });
  function openDocument(e: MouseEvent, path: string) {
    if (e.button !== 0 && e.button !== 1) return;
    // In the browser a new tab is the browser's own, from the link
    if (!inApp && wantsNewTab(e)) return;
    e.preventDefault();
    if (inApp && wantsNewTab(e)) tabs.open(hrefOf(path), inBackground(e));
    else navigate(hrefOf(path));
  }

  const ago = (at: string | null) => (at ? formatAgoOrDate(at, Date.now()) : '');
</script>

{#snippet documentRow(doc: DocumentEntry, depth: number)}
  <li class="documents-row" style:--depth={depth}>
    <a
      class="documents-link"
      href={hrefOf(doc.path)}
      data-path={doc.path}
      onclick={(e) => openDocument(e, doc.path)}
      onauxclick={(e) => openDocument(e, doc.path)}
    >
      <span class="documents-icon"><Icon name="file-text" size={14} /></span>
      <span class="documents-title">{doc.title}</span>
      <span class="documents-file" title={doc.path}>{list.view === 'recent' ? doc.path : fileName(doc.path)}</span>
      <span class="documents-mark"
        >{#if !doc.tracked}<span class="documents-new" title="Not committed yet">new</span>{/if}</span
      >
      <span class="documents-ago" title={doc.modified_at ?? ''}>{ago(doc.modified_at)}</span>
    </a>
  </li>
{/snippet}

{#snippet folderRows(folder: Folder, depth: number)}
  {#each folder.folders as sub (sub.path)}
    {@const opened = isOpen(sub)}
    <li class="documents-row documents-folder-row" style:--depth={depth}>
      <button
        class="documents-folder"
        data-folder={sub.path}
        aria-expanded={opened}
        disabled={!!list.q}
        onclick={() => toggle(sub)}
      >
        <span class="documents-icon"><Icon name={opened ? 'chevron-down' : 'chevron-right'} size={14} /></span><span
          class="documents-icon"><Icon name={sub.submodule ? 'git-branch' : 'folder'} size={14} /></span
        >
        <span class="documents-title">{sub.name}</span>
        <span class="documents-mark"
          >{#if sub.submodule}<span class="documents-submodule"
              >{sub.submodule.checked_out ? 'submodule' : 'submodule, not checked out'}</span
            >{/if}</span
        >
        <span class="documents-count">{sub.count}</span>
      </button>
    </li>
    {#if opened}{@render folderRows(sub, depth + 1)}{/if}
  {/each}
  {#each folder.files as doc (doc.path)}{@render documentRow(doc, depth)}{/each}
{/snippet}

<div class="documents-page" id="documents-page">
  <div class="documents-views" role="group" aria-label="How the documents are listed">
    <button class="documents-view" data-view="tree" aria-pressed={list.view === 'tree'} onclick={() => show('tree')}
      >Tree</button
    >
    <button
      class="documents-view"
      data-view="recent"
      aria-pressed={list.view === 'recent'}
      onclick={() => show('recent')}>Recent</button
    >
  </div>

  {#if problem}<p class="documents-note" role="alert" id="documents-problem">{problem}</p>{/if}

  {#if !listing}
    {#if !problem}<p class="documents-note" id="documents-loading">Reading the documents…</p>{/if}
  {:else if !listing.git}
    <p class="documents-note" id="documents-no-git">
      The project isn’t in git: atm lists a project’s documents from its git repository.
    </p>
  {:else if !listing.documents.length}
    <p class="documents-note" id="documents-empty">No documents yet: Markdown files (.md) in the project show here.</p>
  {:else if !shown}
    <p class="documents-note" id="documents-searching">Searching every document…</p>
  {:else if !shown.length}
    <p class="documents-note" id="documents-none">No document’s title or path holds “{list.q}”.</p>
  {:else}
    <ul class="documents-list" id="documents-list" data-view={list.view}>
      {#if list.view === 'recent'}
        {#each recent as doc (doc.path)}{@render documentRow(doc, 0)}{/each}
      {:else if tree}
        {@render folderRows(tree, 0)}
      {/if}
    </ul>
    {#if cut && !list.q}
      <p class="documents-note" id="documents-more">
        And {listing.more} more: search to find any of them.
      </p>
    {/if}
  {/if}
</div>

<style>
  .documents-page {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    max-width: 960px;
    padding: var(--space-7) var(--space-6);
  }
  .documents-views {
    display: flex;
    gap: var(--space-3);
  }
  .documents-view {
    height: 28px;
    padding: 0 var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: var(--text-sm);
    cursor: pointer;
  }
  .documents-view[aria-pressed='true'] {
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
  }
  .documents-list {
    list-style: none;
    margin: 0;
    padding: var(--space-2) 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
  }
  .documents-link,
  .documents-folder {
    align-items: center;
    background: none;
    border: none;
    color: var(--text);
    display: grid;
    font: inherit;
    font-size: var(--text-sm);
    gap: var(--space-4);
    grid-template-columns: auto minmax(0, 1fr) minmax(0, auto) auto auto;
    padding: var(--space-2) var(--space-6);
    padding-left: calc(var(--space-6) + var(--depth, 0) * var(--space-7));
    text-align: left;
    text-decoration: none;
    width: 100%;
  }
  .documents-folder {
    cursor: pointer;
    grid-template-columns: auto auto minmax(0, 1fr) auto auto;
  }
  .documents-folder:disabled {
    cursor: default;
  }
  .documents-link:hover,
  .documents-folder:hover:not(:disabled) {
    background: var(--surface2);
  }
  .documents-icon {
    color: var(--text-muted);
    display: inline-flex;
  }
  .documents-title {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .documents-folder .documents-title {
    font-weight: 600;
  }
  .documents-file {
    color: var(--text-muted);
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    font-size: var(--text-xs);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .documents-mark {
    display: inline-flex;
  }
  .documents-new,
  .documents-submodule {
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    color: var(--accent);
    font-size: var(--text-2xs);
    padding: 0 var(--space-3);
  }
  .documents-submodule {
    color: var(--text-muted);
  }
  .documents-ago,
  .documents-count {
    color: var(--text-muted);
    font-size: var(--text-xs);
    text-align: right;
    white-space: nowrap;
  }
  .documents-note {
    color: var(--text-muted);
  }
</style>
