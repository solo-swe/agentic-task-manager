<script lang="ts">
  import help from 'virtual:atm-help';
  import { dashboard } from '../lib/state.svelte';
  import CompMd from './CompMd.svelte';
  import Dialog from './Dialog.svelte';

  const open = $derived(dashboard.dialog === 'help');
  /** Rendered the first time Help opens, and kept: CompMD loads only once the page shows Markdown */
  let opened = $state(false);
  $effect(() => {
    if (open) opened = true;
  });
</script>

<!-- docs/help.md, embedded at build time (build/help.ts), rendered by CompMD -->
<Dialog id="help" title="Help" {open} onclose={() => dashboard.closeDialog()}>
  {#if opened}<CompMd text={help} />{/if}
</Dialog>
