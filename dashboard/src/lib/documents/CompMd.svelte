<script lang="ts">
  import { onDestroy } from 'svelte';
  import {
    dashboardAddress,
    loadCompmd,
    processText,
    safeHref,
    secureLinks,
    slug,
    taskData,
    type CompmdModule,
    type DrawnLine,
  } from '../compmd';
  import { docOf, referenceTo, viewerLink } from '../documents';
  import { navigate } from '../navigation';
  import { scrollBehavior } from '../scroll';
  import { dashboard } from '../state.svelte';
  import type { Task } from '../types';

  // Markdown rendered with CompMD (docs/compmd-rendering-design.md): a task text (description, note, question), which
  // is untrusted, or a text that comes with the plugin (Help, What's new; no `task`). Processed with the dashboard's
  // frontmatter and the task's own data, rendered in the dashboard's page by CompMD's sandbox in host mode. Until
  // CompMD has loaded the place is empty; if it can't load or the text can't be processed, the text shows as written
  // (R2). CompMD's diagnostics are never shown here (R2a). With `path`, `text` is a file of the project in the document
  // viewer: a document of its own, under that path, with its own frontmatter (docs/compmd-viewer-design.md §5).
  // `localImage`, in the document viewer: a relative image's address on this server (null: shown as its alt text).
  // `onlines`, in the document viewer: each time the page's layout moved, where each line of the text was drawn
  // (CompMD's origins and its view's lines(), CompMD 0.5.0; T-406), for the viewer's line numbers, with the element the
  // lines' boxes are measured from.
  let {
    text,
    task,
    path,
    localImage,
    inline = false,
    onlines,
  }: {
    text: string;
    task?: Task;
    path?: string;
    localImage?: (url: string) => string | null;
    inline?: boolean;
    onlines?: (lines: DrawnLine[], from: HTMLElement) => void;
  } = $props();

  /** A project's text (a task's, or a document's): a relative link in it opens the document viewer (V1a–V1c), resolved
   * from the project's root for task text and from the document's folder in a document. Help and What's new come with
   * the plugin, not the project: their relative links stay text. */
  const docLink = $derived(
    task || path
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
  /** Whether CompMD has drawn this text yet: `data-compmd` says so, for what measures the panel once it has */
  let drawn = $state(false);
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

  $effect(() => {
    const data = task ? taskData(task) : null;
    const key = text + '\u0000' + JSON.stringify(data) + '\u0000' + (path ?? '');
    const target = host;
    if (!target || key === rendered) return;
    rendered = key;
    loadCompmd()
      .then((compmd) => {
        if (key !== rendered) return; // a newer text came in while CompMD loaded
        const { tree, components } = processText(compmd, text, task, path, { localImage, docLink });
        if (!view) {
          // Host mode: the text renders here, in the page; a pack's component (a Mermaid diagram) in a strict frame
          // of its own. Where the frames and the packs' modules are, with their hashes, is @compmd/vite's (§3c)
          view = compmd.createSandbox({ ...compmd.sandbox, page: 'host' }).open(target, { components });
          view.on('rendered', () => {
            secureLinks(target);
            drawn = true;
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
      })
      .catch((e: unknown) => {
        console.error('CompMD could not render a text:', e);
        failed = true;
      });
  });

  onDestroy(() => {
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
      // A document of the project (a relative link, made a viewer link by safeTree): the viewer opens it
      const doc = docOf(href);
      if (doc) {
        dashboard.openDocument(doc);
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
  data-compmd={failed ? 'failed' : drawn ? 'ready' : 'loading'}
  onclick={follow}
>
  <div class="compmd-page" hidden={failed} bind:this={host}></div>
  {#if failed}<div class="compmd-plain">{text}</div>{/if}
</div>
