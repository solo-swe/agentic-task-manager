<script lang="ts">
  import help from 'virtual:atm-help';
  import { dashboard } from '../state.svelte';
  import CompMd from '../documents/CompMd.svelte';
  import Dialog from '../ui/Dialog.svelte';

  const open = $derived(dashboard.dialog === 'help');
  /** Rendered the first time Help opens, and kept: CompMD loads only once the page shows Markdown */
  let opened = $state(false);
  $effect(() => {
    if (open) opened = true;
  });
</script>

<!-- docs/help.md, embedded at build time (build/help.ts), rendered by CompMD. A scrolling dialog, like What's new: the
     hints count its sections -->
<Dialog id="help" title="Help" {open} onclose={() => dashboard.closeDialog()} scrollItem=".compmd-page > h2">
  {#if opened}<CompMd text={help} />{/if}
</Dialog>
