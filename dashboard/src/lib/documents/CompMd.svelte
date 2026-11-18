<script lang="ts">
  import { onDestroy } from 'svelte';
  import {
    blocksIn,
    dashboardAddress,
    loadCompmd,
    processText,
    safeHref,
    sandboxOptions,
    secureLinks,
    slug,
    taskData,
    taskOfHref,
    type CompmdModule,
    type TaskLinks,
    type DrawnLine,
  } from '../compmd';
  import { docOf, referenceTo, viewerLink, type DocumentRef } from '../documents';
  import { navigate } from '../navigation';
  import { takeTurn } from '../render-turns';
  import { scrollBehavior } from '../scroll';

  /** The data a task text is rendered with, as CompMD's processing takes it (the viewer knows no task model) */
  type TextData = Parameters<typeof processText>[2];

  // Markdown rendered with CompMD (docs/compmd-rendering-design.md): a task text (description, note, question), which
  // is untrusted, or a text that comes with the plugin (Help, What's new; no `task`). Processed with the dashboard's
  // frontmatter and the task's own data, rendered in the dashboard's page by CompMD's sandbox in host mode. Until
  // CompMD has loaded the place is empty; if it can't load or the text can't be processed, the text shows as written
  // (R2). CompMD's diagnostics are never shown here (R2a). With `path`, `text` is a file of the project in the document
  // viewer: a document of its own, under that path, with its own frontmatter (docs/compmd-viewer-design.md §5).
  // `localImage`, in the document viewer: a relative image's address on this server (null: shown as its alt text).
  // `onlines`, in the document viewer: each time the page's layout moved, where each line of the text was drawn
  // (CompMD's origins and its view's lines(), CompMD 0.5.0; T-406), for the viewer's line numbers, with the element the
  // lines' boxes are measured from. `onopen`: what a link to a document of the project does (the page showing the
  // text says: the task manager opens its document viewer); without it, such a link does nothing (ATM-571).
  let {
    text,
    task,
    path,
    localImage,
    inline = false,
    plain,
    only,
    tasks,
    onlines,
    onopen,
  }: {
    text: string;
    task?: TextData;
    path?: string;
    localImage?: (url: string) => string | null;
    inline?: boolean;
    /** Code shown as written until CompMD has drawn the text, which then waits its turn among the page's other such
     * renders, the one nearest the view first (ATM-587: a review's runs of code, `render-turns.ts`) */
    plain?: string;
    /** A document's top-level blocks reaching into these source lines only, the whole text processed (ATM-591: a
     * rendered diff's part, its references and data the document's) */
    only?: readonly [number, number];
    /** Task keys written in the text (ATM-593: a review's description and comments): those `known` link to their
     * task, which `open` opens (`newTab`: ⌘, Ctrl or a middle click); without it, keys stay text */
    tasks?: TaskLinks;
    onlines?: (lines: DrawnLine[], from: HTMLElement) => void;
    onopen?: (ref: DocumentRef) => void;
  } = $props();

  /** The turn this render waits for, cancelled when the text changes or it is gone */
  let cancelTurn: (() => void) | null = null;
  /** The turn's render, done once CompMD has drawn */
  let turnDone: (() => void) | null = null;
  /** How long a turn waits for CompMD to say it has drawn before the next one goes anyway */
  const TURN_LIMIT_MS = 3000;

  /** A project's text (a task's, a document's, or one given `onopen`: a code review's summary and comments, ATM-586): a
   * relative link in it opens the document viewer (V1a–V1c), resolved from the project's root, or from the document's
   * folder in a document. Help and What's new come with the plugin, not the project: their relative links stay text. */
  const docLink = $derived(
    task || path || onopen
      ? (url: string) => {
          const ref = referenceTo(url, path);
          return ref ? viewerLink(ref) : null;
        }
      : undefined,
  );

  type View = ReturnType<ReturnType<CompmdModule['createSandbox']>['open']>;

  /** CompMD's own element: the sandbox's page owns it, Svelte never renders into it */
  let host: HTMLElement | undefined = $state();
  let failed = $state(false);
  /** Why CompMD couldn't load, shown over the text as written (T-503: it was only in the console) */
  let loadError = $state('');
  /** Components that drew nothing, not even their fallback (a frame that never answered, a missing module): one note
   * each, by component, so the reader knows something is missing (the sandbox's `error`, T-503) */
  let undrawn = $state<string[]>([]);
  /** Whether CompMD has drawn this text yet: `data-compmd` says so, for what measures the panel once it has */
  let drawn = $state(false);
  /** Drawn, but for an earlier text: hidden, the new text as written shown in its place until its turn (ATM-625) */
  let stale = $state(false);
  let view: View | null = null;
  let schemes: MutationObserver | null = null;
  /** What was last rendered: the text and the task's data, so an unchanged text is never processed again (R8) */
  let rendered = '';
  /** The last measurement of the lines asked for: an older one that ends after it is dropped */
  let measuring = 0;

  /** Where each line is, read to the end (a document has a size limit, so all of it), then given to `onlines` */
  async function measureLines(shown: View, target: HTMLElement) {
    if (!onlines) return;
    const asked = ++measuring;
    const lines: DrawnLine[] = [];
    for await (const line of shown.lines()) {
      if (asked !== measuring) return;
      lines.push(line);
    }
    if (asked === measuring && shown === view) onlines(lines, target);
  }

  /** What is drawn, as one text: an unchanged one is never processed again (R8) */
  const key = $derived(
    text +
      '\u0000' +
      JSON.stringify(task ? taskData(task) : null) +
      '\u0000' +
      (path ?? '') +
      '\u0000' +
      (only?.join('-') ?? ''),
  );
  // A drawn text that changed waits for its turn as the new text, as written (ATM-625): the old drawing, kept until
  // then, had the old lines, so a run cut in two by a comment stood as tall as before it was cut, and the comment box
  // under its first half was far below, where focusing it scrolled the pane. Before the page is updated, so nothing
  // measures or focuses against the old drawing
  $effect.pre(() => {
    if (plain !== undefined && view && key !== rendered) {
      stale = true;
      drawn = false;
    }
  });

  $effect(() => {
    const target = host;
    if (!target || key === rendered) return;
    rendered = key;
    cancelTurn?.();
    cancelTurn = null;
    const paced = plain !== undefined;
    loadCompmd()
      .then(async (compmd) => {
        if (key !== rendered) return; // a newer text came in while CompMD loaded
        if (!paced) return draw(compmd, target);
        cancelTurn = takeTurn(
          () => target,
          () =>
            new Promise<void>((done) => {
              if (key !== rendered) return done();
              turnDone = done;
              setTimeout(done, TURN_LIMIT_MS);
              try {
                draw(compmd, target);
              } catch (e) {
                fail(e);
                done();
              }
            }),
        );
      })
      .catch(fail);

    function fail(e: unknown) {
      console.error('CompMD could not render a text:', e);
      loadError = e instanceof Error ? e.message : String(e);
      failed = true;
    }

    function draw(compmd: CompmdModule, target: HTMLElement) {
      const processed = processText(compmd, text, task, path, { localImage, docLink, taskKnown: tasks?.known });
      const tree = only ? blocksIn(processed.tree, only) : processed.tree;
      const components = processed.components;
      if (!view) {
        // Host mode: the text renders here, in the page; a pack's component (a Mermaid diagram) in a strict frame
        // of its own. Where the frames and the packs' modules are, with their hashes: sandboxOptions (§3c; in the
        // app, under the origin CompMD's plugin gives the page, ADR-006)
        view = compmd.createSandbox(sandboxOptions(compmd)).open(target, { components });
        view.on('rendered', () => {
          secureLinks(target);
          drawn = true;
          stale = false;
          turnDone?.();
          turnDone = null;
        });
        // A component that shows its fallback says why. One that threw shows its code in its frame; one whose frame
        // never answered, or whose module is missing, shows nothing at all, so the text says so (T-503)
        view.on('error', ({ component, reason, message }) => {
          const nothing = reason === 'failed' || reason === 'missing';
          const log = nothing ? console.error : console.warn;
          log(
            `CompMD: ${component} shows ${nothing ? 'nothing' : 'its fallback'} (${reason}): ${message}` +
              (reason === 'failed'
                ? '. A frame that never answers is almost always refused by a policy: are the frames’ origin ' +
                  'and folder allowed in the page’s script-src, frame-src and connect-src, and the frames served ' +
                  'with no policy but their own?'
                : ''),
          );
          if (nothing && !undrawn.includes(component)) undrawn = [...undrawn, component];
        });
        const shown = view;
        // After a render is painted, when the page or a block in it changes size, when fonts load
        view.on('layout', () => void measureLines(shown, target));
        // The dashboard switches theme and mode by attributes on <html> (its color-scheme follows): the frames,
        // and a diagram's light or dark theme, follow when told
        schemes = new MutationObserver(() => view?.scheme());
        schemes.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'data-mode'] });
      }
      view.render(tree);
      failed = false;
    }
  });

  onDestroy(() => {
    cancelTurn?.();
    turnDone?.();
    schemes?.disconnect();
    view?.destroy();
  });

  /** Links open as the dashboard's always have (§3): `#` ones scroll to the heading in this text (or, a viewer link,
   * open the document); a link to the dashboard itself here; external ones in a new tab without access to this page;
   * any other one not at all */
  function follow(e: MouseEvent) {
    const a = (e.target as Element | null)?.closest('a[href]');
    if (!a || !host?.contains(a)) return;
    const href = (a.getAttribute('href') ?? '').trim();
    if (!safeHref(href)) {
      e.preventDefault();
      return;
    }
    if (href.startsWith('#')) {
      e.preventDefault();
      // A task named in the text (ATM-593): the page showing it says where it opens; ⌘-click, Ctrl-click or a middle
      // click asks for a tab of its own
      const key = taskOfHref(href);
      if (key) {
        tasks?.open(key, e.metaKey || e.ctrlKey || e.button === 1);
        return;
      }
      // A document of the project (a relative link, made a viewer link by safeTree): the viewer opens it
      const doc = docOf(href);
      if (doc) {
        onopen?.(doc);
        return;
      }
      const wanted = decodeURIComponent(href.slice(1));
      const heading = [...host.querySelectorAll('h1, h2, h3, h4, h5, h6')].find(
        (h) => slug(h.textContent ?? '') === wanted,
      );
      heading?.scrollIntoView({ behavior: scrollBehavior(), block: 'start' });
      return;
    }
    // A link to the dashboard itself (a task's or a document's URL) opens here, a new history entry for Back
    const address = dashboardAddress(href);
    if (address !== null) {
      e.preventDefault();
      navigate(address);
      return;
    }
    a.setAttribute('target', '_blank');
    a.setAttribute('rel', 'noopener noreferrer');
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div
  class="compmd"
  class:compmd-inline={inline}
  class:compmd-stale={stale}
  data-compmd={failed ? 'failed' : drawn ? 'ready' : 'loading'}
  onclick={follow}
  onauxclick={(e) => {
    // A middle click on a task's key: its tab of its own (other links: the browser's own)
    const a = (e.target as Element | null)?.closest('a[href^="#task="]');
    if (a && e.button === 1) follow(e);
  }}
>
  {#if failed}
    <p class="compmd-note" role="note">Shown as written: the formatting couldn’t load ({loadError}).</p>
  {/if}
  <div class="compmd-page" hidden={failed || stale} bind:this={host}></div>
  {#if plain !== undefined && !drawn && !failed}
    <!-- Its turn not come yet: the code as written, where CompMD will draw it (ATM-587). Hidden by the style below as
         soon as CompMD has drawn anything, before it says so: both at once made the run twice as tall for a moment -->
    <div class="compmd-page compmd-waiting"><pre><code>{plain}</code></pre></div>
  {/if}
  {#if failed}<div class="compmd-plain">{text}</div>{/if}
  {#each undrawn as component (component)}
    <p class="compmd-note" role="note">A {component} here couldn’t be drawn; the console says why.</p>
  {/each}
</div>

<style>
  /* The code as written gives way the moment CompMD has drawn the text (ATM-587); not to a drawing of an earlier text
     (ATM-625) */
  .compmd:not(.compmd-stale) > .compmd-page:not(:empty) + .compmd-waiting {
    display: none;
  }
</style>
