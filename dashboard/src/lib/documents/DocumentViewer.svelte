<script lang="ts">
  import { untrack } from 'svelte';
  import { browserFetch, inApp, v1FileUrl, type Validators } from '../api/http';
  import {
    clampGeometry,
    defaultGeometry,
    documentTitle,
    findTextFragment,
    isGeometry,
    parseTextFragment,
    loadDocument,
    pathToCopy,
    repositoryFile,
    resolvePath,
    VIEWER_NARROW,
    type DocumentError,
    type ProjectDocument,
    type ViewerGeometry,
  } from '../documents';
  import { getPref, setPref } from '../prefs';
  import { fileRef } from '../repo';
  import { gutterRows, slug, type DrawnLine, type GutterRow } from '../compmd';
  import { compactAt } from '../scroll';
  import { dashboard } from '../state.svelte';
  import CompMd from './CompMd.svelte';
  import Icon from '../ui/Icon.svelte';

  // The document viewer (docs/compmd-viewer-design.md §2, §6): a window over the details panel showing a file of the
  // project, as CompMD renders the document atm wraps it in. It moves by its title bar and resizes from its corner,
  // keeps where it was, and on a narrow window fills it. What is open is in the URL hash (#doc=), so Back, a link and
  // Reload work; the file is asked for again on the dashboard's refresh, a 304 while it is unchanged.

  const ref = $derived(dashboard.docRef);
  // The shown project's files; on the Omniboard, those of the project whose task is open over it (T-411)
  const project = $derived(dashboard.route.page === 'project' || dashboard.peeking ? dashboard.projectId : null);
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
  // Open on GitHub (or the project's host): only for a file git tracks, at the commit the server named; a file in a
  // submodule in the submodule's own repository (T-408)
  const repoFile = $derived.by(() => {
    const where = doc && ref ? repositoryFile(doc, ref.path, dashboard.meta) : null;
    return where && ref ? fileRef(where.meta, where.commit, where.path, ref.lines) : null;
  });

  /** A relative image in the document, resolved from its folder, loads from this server's files route (V5); any
   * other image shows its alt text */
  const localImage = (url: string): string | null => {
    const resolved = doc && project ? resolvePath(url, doc.path) : null;
    // In the atm app there is no files route to load from: the image shows its alt text for now (T-457's follow-up)
    return resolved && project && !inApp ? v1FileUrl(project, resolved) : null;
  };

  // A code or text file's lines (V2a): CompMD renders it as one code block, its lines never wrapping, so the gutter
  // numbers them at the block's top and line height, measured after each render and on resize. A Markdown document's
  // come from CompMD (T-406): every source line, where CompMD drew it (its origins and the view's lines(), CompMD
  // 0.5.0), and the lines that draw nothing in the gaps between (gutterRows), measured again each time the page's
  // layout moves.
  let pageEl = $state<HTMLElement>();
  let lineLayout = $state<{ top: number; lineHeight: number; fontSize: number } | null>(null);
  /** A Markdown document's gutter: a row per line (or per lines sharing a visual line), in the page's coordinates */
  let rows = $state.raw<GutterRow[]>([]);
  /** Where a Markdown document's lines were drawn, in the page's coordinates */
  let drawnLines = $state.raw<DrawnLine[]>([]);

  function placeMarkdownLines(lines: DrawnLine[], from: HTMLElement) {
    const el = pageEl;
    if (!el || doc?.kind !== 'markdown') return;
    // The lines' boxes are from the top left of CompMD's container's content; the gutter and the marks are in the page
    const page = el.getBoundingClientRect();
    const box = from.getBoundingClientRect();
    const style = getComputedStyle(from);
    const dx = box.left - page.left + from.clientLeft + parseFloat(style.paddingLeft || '0');
    const dy = box.top - page.top + from.clientTop + parseFloat(style.paddingTop || '0');
    const round = (n: number) => Math.round(n * 10) / 10;
    const moved = lines.map((l) => ({
      line: l.line,
      rects: l.rects.map((r) => ({
        x: round(r.x + dx),
        y: round(r.y + dy),
        width: round(r.width),
        height: round(r.height),
      })),
    }));
    const next = gutterRows(moved, doc.lines, { top: 0, bottom: el.clientHeight }).map((r) => ({
      ...r,
      top: round(r.top),
      height: round(r.height),
    }));
    if (JSON.stringify(moved) !== JSON.stringify(drawnLines)) drawnLines = moved;
    if (JSON.stringify(next) !== JSON.stringify(rows)) rows = next;
    scrollToMarkedRows();
  }

  const inLink = (from: number, to: number) => !!ref?.lines && from <= ref.lines[1] && to >= ref.lines[0];
  /** The gutter's rows a `:L<n>-L<m>` link names */
  const markedRows = $derived(ref?.lines ? rows.filter((r) => inLink(r.from, r.to)) : []);
  /** The boxes of the text of exactly those lines: what CompMD drew from them */
  const markedRects = $derived(
    ref?.lines ? drawnLines.filter((l) => inLink(l.line, l.line)).flatMap((l) => l.rects) : [],
  );

  // Another file: its own lines, once CompMD has drawn it; another link into the same file scrolls to its lines
  const docPath = $derived(ref?.path);
  $effect(() => {
    void docPath;
    untrack(() => {
      rows = [];
      drawnLines = [];
    });
  });
  $effect(() => {
    if (markedRows.length) untrack(scrollToMarkedRows);
  });

  /** `#L<n>` in a Markdown document: its first line a little below the window's top (once) */
  function scrollToMarkedRows() {
    if (!ref?.lines || !pageEl || !markedRows.length) return;
    const key = `${ref.path}:${ref.lines.join('-')}`;
    if (key === scrolledTo) return;
    scrolledTo = key;
    pageEl.closest('.doc-body')?.scrollTo({ top: Math.max(0, pageEl.offsetTop + markedRows[0].top - 48) });
  }
  /** The file and lines last scrolled to, so a refresh doesn't scroll again */
  let scrolledTo = '';

  /** `#<heading>`: once the document is drawn, its heading with that slug at the window's top (once per file and
   * heading) */
  function scrollToHeading() {
    if (!ref?.heading || !pageEl) return;
    const key = `${ref.path}#${ref.heading}`;
    if (key === scrolledTo) return;
    const wanted = ref.heading;
    const heading = [...pageEl.querySelectorAll<HTMLElement>('.compmd-page :is(h1, h2, h3, h4, h5, h6)')].find(
      (h) => slug(h.textContent ?? '') === wanted,
    );
    if (!heading) return;
    scrolledTo = key;
    const body = pageEl.closest('.doc-body');
    body?.scrollTo({
      top: Math.max(0, heading.getBoundingClientRect().top - body.getBoundingClientRect().top + body.scrollTop - 16),
    });
  }

  /** `#:~:text=…`: once the document is drawn, the first match marked (the CSS Custom Highlight API: CompMD's page
   * isn't changed) and scrolled to, once per file and fragment; no match, a note saying so (T-391) */
  let fragmentMissing = $state(false);
  const FRAGMENT_HIGHLIGHT = 'atm-doc-fragment';
  function showTextFragment() {
    if (!ref?.text || !pageEl) return;
    const key = `${ref.path}#${ref.text}`;
    if (key === scrolledTo) return;
    const page = pageEl.querySelector('.compmd-page');
    if (!page || pageEl.querySelector('.compmd')?.getAttribute('data-compmd') !== 'ready') return;
    scrolledTo = key;
    const range = parseTextFragment(ref.text)
      .map((d) => findTextFragment(page, d))
      .find((r): r is Range => r !== null);
    fragmentMissing = !range;
    if (!range) return;
    if (typeof CSS !== 'undefined' && 'highlights' in CSS) CSS.highlights.set(FRAGMENT_HIGHLIGHT, new Highlight(range));
    const body = pageEl.closest('.doc-body');
    const box = range.getBoundingClientRect();
    if (body) body.scrollTo({ top: Math.max(0, box.top - body.getBoundingClientRect().top + body.scrollTop - 64) });
  }
  /** The mark goes at the next click, and with the file */
  function clearTextFragment() {
    if (typeof CSS !== 'undefined' && 'highlights' in CSS) CSS.highlights.delete(FRAGMENT_HIGHLIGHT);
  }
  $effect(() => {
    if (!ref?.text) {
      clearTextFragment();
      fragmentMissing = false;
    }
  });

  // The contents (T-402): a Markdown document's headings, h1 to h3, read from what CompMD rendered (atm renders
  // nothing), again after each render; shown only for two or more. The list stays hidden until the faint button is
  // hovered or focused, and hides again after a pick, when the pointer leaves, or on Escape.
  type TocEntry = { level: number; text: string; el: HTMLElement };
  let toc = $state.raw<TocEntry[]>([]);
  let tocOpen = $state(false);
  /** The heading of the section being read */
  let tocCurrent = $state(-1);
  let tocEl = $state<HTMLElement>();

  function readHeadings() {
    if (!pageEl || doc?.kind !== 'markdown') {
      if (toc.length) toc = [];
      return;
    }
    const next = [...pageEl.querySelectorAll<HTMLElement>('.compmd-page :is(h1, h2, h3)')]
      .map((el) => ({ level: Number(el.tagName[1]), text: (el.textContent ?? '').trim(), el }))
      .filter((h) => h.text);
    if (next.length !== toc.length || next.some((h, i) => h.el !== toc[i].el || h.text !== toc[i].text)) {
      toc = next;
      followSection();
    }
  }

  /** The section being read: the last heading at or above the top of the window */
  function followSection() {
    const body = pageEl?.closest('.doc-body');
    if (!body || !toc.length) return;
    const top = body.getBoundingClientRect().top + 72;
    let found = -1;
    toc.forEach((h, i) => {
      if (h.el.getBoundingClientRect().top <= top) found = i;
    });
    tocCurrent = found;
  }

  function goToHeading(i: number) {
    const body = pageEl?.closest('.doc-body');
    const heading = toc[i]?.el;
    if (!body || !heading) return;
    body.scrollTo({
      top: Math.max(0, heading.getBoundingClientRect().top - body.getBoundingClientRect().top + body.scrollTop - 16),
    });
    tocOpen = false;
  }

  /** Up and down move between the entries; Escape closes the list (before the viewer sees it) */
  function onTocKeys(e: KeyboardEvent) {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    const entries = [...(tocEl?.querySelectorAll<HTMLElement>('.doc-toc-list button') ?? [])];
    if (!entries.length) return;
    e.preventDefault();
    const at = entries.indexOf(document.activeElement as HTMLElement);
    const next = at < 0 ? 0 : (at + (e.key === 'ArrowDown' ? 1 : -1) + entries.length) % entries.length;
    entries[next].focus();
  }

  function measureLines() {
    readHeadings();
    scrollToHeading();
    showTextFragment();
    const code = pageEl?.querySelector<HTMLElement>('.compmd-page pre > code');
    if (!pageEl || !code || !doc || (doc.kind !== 'code' && doc.kind !== 'text')) {
      lineLayout = null;
      return;
    }
    const style = getComputedStyle(code);
    const lineHeight = parseFloat(style.lineHeight) || parseFloat(style.fontSize) * 1.6;
    // The code is a block (app.css), so its top is its first line's
    const top = code.getBoundingClientRect().top - pageEl.getBoundingClientRect().top;
    const next = { top, lineHeight, fontSize: parseFloat(style.fontSize) };
    if (JSON.stringify(next) !== JSON.stringify(lineLayout)) lineLayout = next;
    scrollToLines();
  }

  /** `#L<n>`: once the lines are laid out, scroll them a little below the window's top (once per file and lines) */
  function scrollToLines() {
    if (!ref?.lines || !lineLayout || !pageEl) return;
    const key = `${ref.path}:${ref.lines.join('-')}`;
    if (key === scrolledTo) return;
    scrolledTo = key;
    const body = pageEl.closest('.doc-body');
    if (!body) return;
    const target = pageEl.offsetTop + lineLayout.top + (ref.lines[0] - 1) * lineLayout.lineHeight;
    body.scrollTo({ top: Math.max(0, target - 48) });
  }

  const lineNumbers = $derived(Array.from({ length: doc?.lines ?? 0 }, (_, i) => i + 1));
  const isMarked = (line: number) => !!ref?.lines && line >= ref.lines[0] && line <= ref.lines[1];

  $effect(() => {
    const el = pageEl;
    if (!el || !doc) return;
    measureLines();
    const resize = new ResizeObserver(measureLines);
    resize.observe(el);
    const rendered = new MutationObserver(measureLines);
    rendered.observe(el, { childList: true, subtree: true, attributes: true, attributeFilter: ['data-compmd'] });
    return () => {
      resize.disconnect();
      rendered.disconnect();
    };
  });

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
    if (tocOpen) {
      tocOpen = false; // the contents first, then the viewer
      return;
    }
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
    data-doc-style={dashboard.docStyle === 'auto' ? null : dashboard.docStyle}
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
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions (a click only clears the
         text fragment's mark) -->
    <div
      class="doc-body"
      id="doc-body"
      onscroll={(e) => {
        scrolled = compactAt(e.currentTarget.scrollTop);
        followSection();
      }}
      onclick={clearTextFragment}
    >
      {#if doc && toc.length >= 2}
        <!-- The contents: one box at the page's top left, in view as it scrolls. Closed, it is a faint button; hovered
             or focused, the box itself grows into the list, so the pointer is already inside it -->
        <div class="doc-toc" class:open={tocOpen} id="doc-toc">
          <div
            class="doc-toc-box"
            bind:this={tocEl}
            role="presentation"
            onmouseenter={() => (tocOpen = true)}
            onmouseleave={() => (tocOpen = false)}
            onfocusin={() => (tocOpen = true)}
            onfocusout={(e) => {
              if (!tocEl?.contains(e.relatedTarget as Node | null)) tocOpen = false;
            }}
            onkeydown={onTocKeys}
          >
            <button
              class="doc-toc-button"
              id="doc-toc-button"
              aria-label="Contents"
              title="Contents"
              aria-expanded={tocOpen}
              onclick={(e) => {
                e.stopPropagation();
                tocOpen = !tocOpen;
              }}><Icon name="list" size={14} /><span class="doc-toc-title">Contents</span></button
            >
            <nav class="doc-toc-list" aria-label="Contents" inert={!tocOpen}>
              {#each toc as heading, i (i)}
                <button
                  class="doc-toc-level-{heading.level}"
                  class:current={i === tocCurrent}
                  aria-current={i === tocCurrent ? 'location' : undefined}
                  onclick={(e) => {
                    e.stopPropagation();
                    goToHeading(i);
                  }}>{heading.text}</button
                >
              {/each}
            </nav>
          </div>
        </div>
      {/if}
      {#if doc && fragmentMissing}
        <div class="doc-note" id="doc-fragment-missing">The text this link points to isn't in this document.</div>
      {/if}
      {#if doc}
        <!-- A page on the desk: each theme may give it its own paper (--doc-page-*, themes/*.css) -->
        <article class="doc-page" data-kind={doc.kind} bind:this={pageEl}>
          {#if lineLayout}
            <!-- The code's line numbers and the lines the link names: the viewer's own, beside CompMD's block -->
            <div
              class="doc-gutter"
              id="doc-gutter"
              aria-hidden="true"
              style:top="{lineLayout.top}px"
              style:line-height="{lineLayout.lineHeight}px"
              style:font-size="{lineLayout.fontSize}px"
            >
              {#each lineNumbers as line (line)}<div class:marked={isMarked(line)}>{line}</div>{/each}
            </div>
            {#if ref.lines}
              <div
                class="doc-line-mark"
                id="doc-line-mark"
                style:top="{lineLayout.top + (ref.lines[0] - 1) * lineLayout.lineHeight}px"
                style:height="{(Math.min(ref.lines[1], doc.lines) - ref.lines[0] + 1) * lineLayout.lineHeight}px"
              ></div>
            {/if}
          {/if}
          {#if doc.kind === 'markdown' && rows.length}
            <!-- A Markdown document's line numbers (T-406): each line beside its text, where CompMD drew it; lines
                 that draw nothing in the gaps; a link's lines marked, in the gutter and on their text -->
            <div class="doc-gutter doc-gutter-lines" id="doc-gutter" aria-hidden="true">
              {#each rows as row (row.from)}
                <div
                  class:marked={inLink(row.from, row.to)}
                  class:blank={!row.drawn}
                  data-line={row.from}
                  style:top="{row.top}px"
                  style:height="{row.height}px"
                >
                  {row.from === row.to ? row.from : `${row.from}–${row.to}`}
                </div>
              {/each}
            </div>
            {#each markedRects as rect, i (i)}
              <div
                class="doc-line-mark doc-text-mark"
                id={i === 0 ? 'doc-line-mark' : undefined}
                style:left="{rect.x}px"
                style:top="{rect.y}px"
                style:width="{rect.width}px"
                style:height="{rect.height}px"
              ></div>
            {/each}
          {/if}
          {#key `${ref.path}`}
            <CompMd text={doc.document} path={doc.path} {localImage} onlines={placeMarkdownLines} />
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
