<script lang="ts">
  import { onDestroy } from 'svelte';
  import {
    compmdAssetUrl,
    compmdRequest,
    loadCompmd,
    safeTree,
    secureLinks,
    taskData,
    type CompmdModule,
  } from '../lib/compmd';
  import { safeHref, slug } from '../lib/markdown';
  import { scrollBehavior } from '../lib/scroll';
  import type { Task } from '../lib/types';

  // A task text (description, note, question) rendered with CompMD (docs/compmd-rendering-design.md): untrusted text,
  // processed with the dashboard's frontmatter and the task's own data, rendered in the dashboard's page by CompMD's
  // sandbox in host mode. Until CompMD has loaded the place is empty; if it can't load or the text can't be processed,
  // the text shows as written (R2). CompMD's diagnostics are never shown here (R2a).
  let { text, task, inline = false }: { text: string; task: Task; inline?: boolean } = $props();

  type View = ReturnType<ReturnType<CompmdModule['createSandbox']>['open']>;

  /** CompMD's own element: the sandbox's page owns it, Svelte never renders into it */
  let host: HTMLElement | undefined = $state();
  let failed = $state(false);
  /** Whether CompMD has drawn this text yet: `data-compmd` says so, for what measures the panel once it has */
  let drawn = $state(false);
  let view: View | null = null;
  /** What was last rendered: the text and the task's data, so an unchanged text is never processed again (R8) */
  let rendered = '';

  $effect(() => {
    const data = taskData(task);
    const key = text + '\u0000' + JSON.stringify(data);
    const target = host;
    if (!target || key === rendered) return;
    rendered = key;
    loadCompmd()
      .then((compmd) => {
        if (key !== rendered) return; // a newer text came in while CompMD loaded
        const processed = compmd.process(compmdRequest(text, task));
        if (!view) {
          // Host mode, no components: the page renders here; packs' frames (T-285) will come from the asset folder
          view = compmd.createSandbox({ base: compmdAssetUrl(''), page: 'host' }).open(target, { components: [] });
          view.on('rendered', () => {
            secureLinks(target);
            drawn = true;
          });
        }
        view.render(safeTree(processed.expanded));
        failed = false;
      })
      .catch((e: unknown) => {
        console.error('CompMD could not render a task text:', e);
        failed = true;
      });
  });

  onDestroy(() => view?.destroy());

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
      const wanted = decodeURIComponent(href.slice(1));
      const heading = [...host.querySelectorAll('h1, h2, h3, h4, h5, h6')].find(
        (h) => slug(h.textContent ?? '') === wanted,
      );
      heading?.scrollIntoView({ behavior: scrollBehavior(), block: 'start' });
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
