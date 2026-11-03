<script lang="ts">
  import help from 'virtual:atm-help';
  import { dashboard } from '../lib/state.svelte';
  import CompMd from './CompMd.svelte';
  import Dialog from './Dialog.svelte';
  import HintScroll from './HintScroll.svelte';

  const open = $derived(dashboard.dialog === 'help');
  /** Rendered the first time Help opens, and kept: CompMD loads only once the page shows Markdown */
  let opened = $state(false);
  $effect(() => {
    if (open) opened = true;
  });
  /** The text has scrolled: the title compacts (as the dashboard's title bar does once the page scrolls) */
  let scrolled = $state(false);
</script>

<!-- docs/help.md, embedded at build time (build/help.ts), rendered by CompMD. Like What's new, the title stays and the
     text scrolls below it without a scrollbar, the hints counting the sections above and below -->
<Dialog id="help" title="Help" {open} {scrolled} onclose={() => dashboard.closeDialog()}>
  <HintScroll item=".compmd-page > h2" outer="help-scroll" id="help-body" onscrolled={(top) => (scrolled = top > 8)}>
    {#if opened}<CompMd text={help} />{/if}
  </HintScroll>
</Dialog>
