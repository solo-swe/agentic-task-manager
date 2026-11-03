<script lang="ts">
  import { browserFetch, type Validators } from '../lib/api/http';
  import {
    clampGeometry,
    defaultGeometry,
    documentTitle,
    isGeometry,
    loadDocument,
    pathToCopy,
    VIEWER_NARROW,
    type DocumentError,
    type ProjectDocument,
    type ViewerGeometry,
  } from '../lib/documents';
  import { getPref, setPref } from '../lib/prefs';
  import { fileRef } from '../lib/repo';
  import { compactAt } from '../lib/scroll';
  import { dashboard } from '../lib/state.svelte';
  import CompMd from './CompMd.svelte';
  import Icon from './Icon.svelte';

  // The document viewer (docs/compmd-viewer-design.md §2, §6): a window over the details panel showing a file of the
  // project, as CompMD renders the document atm wraps it in. It moves by its title bar and resizes from its corner,
  // keeps where it was, and on a narrow window fills it. What is open is in the URL hash (#doc=), so Back, a link and
  // Reload work; the file is asked for again on the dashboard's refresh, a 304 while it is unchanged.

  const ref = $derived(dashboard.docRef);
  const project = $derived(dashboard.route.page === 'project' ? dashboard.projectId : null);
  const open = $derived(ref !== null && project !== null);

  let doc = $state.raw<ProjectDocument | null>(null);
  let error = $state<{ kind: DocumentError; message: string } | null>(null);
  let loading = $state(false);
  let validators: Validators | undefined;
  /** The project and path the shown document is of */
  let shownKey = '';

  async function load(key: string, projectId: string, path: string, poll: boolean) {
    const result = await loadDocument(projectId, path, browserFetch, poll ? validators : undefined);
    if (key !== `${dashboard.projectId}\u0000${dashboard.docRef?.path}`) return; // another file was opened meanwhile
    loading = false;
    if (result.unchanged) return;
    if ('error' in result) {
      // A poll that fails keeps what is shown; a first load says what happened
      if (!poll || result.error !== 'failed') {
        doc = null;
        error = { kind: result.error, message: result.message };
      }
      return;
    }
    validators = result.validators;
    doc = result.document;
    error = null;
  }

  // A file opened: load it (what was shown goes, so a stale file never shows under the new name)
  $effect(() => {
    if (!open || !ref || !project) return;
    const key = `${project}\u0000${ref.path}`;
    if (key === shownKey) return;
    shownKey = key;
    doc = null;
    error = null;
    validators = undefined;
    loading = true;
    void load(key, project, ref.path, false);
  });

  // Closed: the next open starts afresh
  $effect(() => {
    if (!open) shownKey = '';
  });

  // Kept current as the dashboard refreshes: a conditional request, 304 while the file is unchanged
  $effect(() => {
    if (!open || !ref || !project || !dashboard.autoRefresh) return;
    const key = `${project}\u0000${ref.path}`;
    const timer = setInterval(() => void load(key, project, ref.path, true), dashboard.autoRefreshSeconds * 1000);
    return () => clearInterval(timer);
  });

  const title = $derived(doc ? documentTitle(doc) : (ref?.path.split('/').pop() ?? ''));
  // Open on GitHub (or the project's host): only for a file git tracks, at the commit the server named
  const repoFile = $derived(
    doc?.tracked && doc.commit && ref ? fileRef(dashboard.meta, doc.commit, ref.path, ref.lines) : null,
  );

  /** The document has scrolled: the title bar compacts, as the dashboard's and the dialogs' do (lib/scroll.ts) */
  let scrolled = $state(false);
  // Another file opens at its top, with the full title bar
  $effect(() => {
    if (ref?.path) scrolled = false;
  });

  let copied = $state(false);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;
  async function copyPath() {
    if (!ref) return;
    try {
      await navigator.clipboard.writeText(pathToCopy(ref));
      copied = true;
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copied = false), 1500);
    } catch (e) {
      console.error('Copying the path failed:', e);
    }
  }

  // Where it is and how big: kept per browser, inside the window
  let width = $state(window.innerWidth);
  let height = $state(window.innerHeight);
  const narrow = $derived(width < VIEWER_NARROW);
  const stored = getPref('documentViewer', null);
  let geometry = $state<ViewerGeometry>(
    isGeometry(stored) ? stored : defaultGeometry(window.innerWidth, window.innerHeight),
  );
  const placed = $derived(clampGeometry(geometry, width, height));

  function save() {
    geometry = placed;
    setPref('documentViewer', { ...placed });
  }

  // Moving by the title bar, and resizing from the corner: the pointer is captured until it is let go
  let drag: { kind: 'move' | 'resize'; x: number; y: number; from: ViewerGeometry } | null = null;

  function startDrag(e: PointerEvent, kind: 'move' | 'resize') {
    if (narrow || e.button !== 0) return;
    if (kind === 'move' && (e.target as Element).closest('button, a')) return;
    e.preventDefault();
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    drag = { kind, x: e.clientX, y: e.clientY, from: placed };
  }

  function onDrag(e: PointerEvent) {
    if (!drag) return;
    const dx = e.clientX - drag.x;
    const dy = e.clientY - drag.y;
    const from = drag.from;
    geometry =
      drag.kind === 'move' ? { ...from, x: from.x + dx, y: from.y + dy } : { ...from, w: from.w + dx, h: from.h + dy };
  }

  function endDrag() {
    if (!drag) return;
    drag = null;
    save();
  }

  const STEP = 16;
  /** Arrows on the focused title bar move it; with Shift (or on the corner grip), they resize it */
  function onKeys(e: KeyboardEvent, resize: boolean) {
    if (narrow) return;
    const delta = { ArrowLeft: [-STEP, 0], ArrowRight: [STEP, 0], ArrowUp: [0, -STEP], ArrowDown: [0, STEP] }[e.key] as
      [number, number] | undefined;
    if (!delta) return;
    e.preventDefault();
    const [dx, dy] = delta;
    geometry =
      resize || e.shiftKey
        ? { ...placed, w: placed.w + dx, h: placed.h + dy }
        : { ...placed, x: placed.x + dx, y: placed.y + dy };
    save();
  }

  // Escape is the viewer's Back, before the details panel under it sees it
  function onKeydownCapture(e: KeyboardEvent) {
    if (!open || e.key !== 'Escape') return;
    e.stopImmediatePropagation();
    e.preventDefault();
    dashboard.documentBack();
  }

  const MESSAGES: Record<DocumentError, string> = {
    missing: 'Not in the project',
    refused: "Can't be shown here",
    failed: "Couldn't load it",
  };
</script>

<svelte:window bind:innerWidth={width} bind:innerHeight={height} />
<svelte:document onkeydowncapture={onKeydownCapture} />

{#if open && ref}
  <div
    class="doc-viewer"
    class:narrow
    class:scrolled
    id="doc-viewer"
    role="dialog"
    aria-label="Document: {title}"
    data-kind={doc?.kind ?? ''}
    style:left={narrow ? null : `${placed.x}px`}
    style:top={narrow ? null : `${placed.y}px`}
    style:width={narrow ? null : `${placed.w}px`}
    style:height={narrow ? null : `${placed.h}px`}
  >
    <!-- The title bar: drag or arrow keys to move it (Shift: resize) -->
    <div
      class="doc-titlebar"
      role="toolbar"
      tabindex="0"
      aria-label="Move the document window with the arrow keys; Shift and the arrows resize it"
      onpointerdown={(e) => startDrag(e, 'move')}
      onpointermove={onDrag}
      onpointerup={endDrag}
      onpointercancel={endDrag}
      onkeydown={(e) => onKeys(e, false)}
    >
      <button class="doc-back" id="doc-back" aria-label="Back" title="Back" onclick={() => dashboard.documentBack()}
        ><Icon name="chevron-left" /></button
      >
      <div class="doc-heading">
        <h3 class="doc-title" id="doc-title">{title}</h3>
        <div class="doc-path" id="doc-path">
          {ref.path}{ref.lines ? ` · lines ${ref.lines[0]}–${ref.lines[1]}` : ''}
        </div>
      </div>
      <button class="doc-action" id="doc-copy" title="Copy the path" onclick={copyPath}
        ><Icon name={copied ? 'check' : 'copy'} size={14} />{copied ? 'Copied' : 'Copy path'}</button
      >
      {#if repoFile}
        <a class="doc-action" id="doc-repo" href={repoFile.href} target="_blank" rel="noopener noreferrer"
          ><Icon name="external-link" size={14} />Open on {repoFile.host}</a
        >
      {/if}
      <button
        class="doc-close"
        id="doc-close"
        aria-label="Close"
        title="Close"
        onclick={() => dashboard.closeDocument()}><Icon name="x" size={13} /></button
      >
    </div>
    <div class="doc-body" id="doc-body" onscroll={(e) => (scrolled = compactAt(e.currentTarget.scrollTop))}>
      {#if doc}
        <!-- A page on the desk: each theme may give it its own paper (--doc-page-*, themes/*.css) -->
        <article class="doc-page" data-kind={doc.kind}>
          {#key `${ref.path}`}
            <CompMd text={doc.document} path={doc.path} />
          {/key}
        </article>
      {:else if error}
        <div class="doc-fallback" id="doc-fallback" data-error={error.kind}>
          <strong>{MESSAGES[error.kind]}</strong>
          <span>{error.message}</span>
        </div>
      {:else if loading}
        <div class="doc-fallback">Loading…</div>
      {/if}
    </div>
    {#if !narrow}
      <!-- The corner grip: drag, or arrow keys when focused, to resize -->
      <div
        class="doc-grip"
        id="doc-grip"
        role="slider"
        tabindex="0"
        aria-label="Resize the document window"
        aria-valuenow={placed.w}
        onpointerdown={(e) => startDrag(e, 'resize')}
        onpointermove={onDrag}
        onpointerup={endDrag}
        onpointercancel={endDrag}
        onkeydown={(e) => onKeys(e, true)}
      ></div>
    {/if}
  </div>
{/if}
