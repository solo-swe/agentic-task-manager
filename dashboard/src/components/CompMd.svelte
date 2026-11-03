<script lang="ts">
  import { onDestroy } from 'svelte';
  import {
    dashboardHash,
    loadCompmd,
    processText,
    safeHref,
    secureLinks,
    slug,
    taskData,
    type CompmdModule,
  } from '../lib/compmd';
  import { parseDocHash, referenceTo, withDocHash } from '../lib/documents';
  import { scrollBehavior } from '../lib/scroll';
  import { dashboard } from '../lib/state.svelte';
  import type { Task } from '../lib/types';

  // Markdown rendered with CompMD (docs/compmd-rendering-design.md): a task text (description, note, question), which
  // is untrusted, or a text that comes with the plugin (Help, What's new; no `task`). Processed with the dashboard's
  // frontmatter and the task's own data, rendered in the dashboard's page by CompMD's sandbox in host mode. Until
  // CompMD has loaded the place is empty; if it can't load or the text can't be processed, the text shows as written
  // (R2). CompMD's diagnostics are never shown here (R2a). With `path`, `text` is a file of the project in the document
  // viewer: a document of its own, under that path, with its own frontmatter (docs/compmd-viewer-design.md §5).
  // `localImage`, in the document viewer: a relative image's address on this server (null: shown as its alt text).
  let {
    text,
    task,
    path,
    localImage,
    inline = false,
  }: {
    text: string;
    task?: Task;
    path?: string;
    localImage?: (url: string) => string | null;
    inline?: boolean;
  } = $props();

  /** A project's text (a task's, or a document's): a relative link in it opens the document viewer (V1a–V1c), resolved
   * from the project's root for task text and from the document's folder in a document. Help and What's new come with
   * the plugin, not the project: their relative links stay text. */
  const docLink = $derived(
    task || path
      ? (url: string) => {
          const ref = referenceTo(url, path);
          return ref ? withDocHash('', ref) : null;
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

  /** Links open as the dashboard's always have (§3): `#` ones scroll to the heading in this text, the URL hash being
   * the dashboard's state; external ones in a new tab without access to this page; any other one not at all */
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
      const doc = parseDocHash(href);
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
    const hash = dashboardHash(href);
    if (hash !== null) {
      e.preventDefault();
      location.hash = hash;
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
