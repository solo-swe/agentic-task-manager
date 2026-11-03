<script lang="ts">
  import { entryMarkdown, UNRELEASED } from '../lib/changelog';
  import { releaseNotes } from '../lib/release-notes';
  import { dashboard } from '../lib/state.svelte';
  import CompMd from './CompMd.svelte';
  import Dialog from './Dialog.svelte';

  /** The notes render the first time What's new opens, and stay: CompMD loads only once the page shows Markdown */
  let opened = $state(false);
  $effect(() => {
    if (dashboard.whatsNewOpen) opened = true;
  });
</script>

<!-- A scrolling dialog (T-282), like Help: the title stays, the releases scroll below it, the hints count them -->
<Dialog
  id="whats-new"
  title="What's new"
  open={dashboard.whatsNewOpen}
  onclose={() => dashboard.closeWhatsNew()}
  scrollItem=".release"
>
  {#each releaseNotes as entry (entry.version)}
    <section class="release" data-version={entry.version}>
      <h4>
        {entry.version === UNRELEASED ? 'Unreleased' : 'v' + entry.version}{#if entry.date}<span class="release-date"
            >{entry.date}</span
          >{/if}
      </h4>
      {#if opened}<CompMd text={entryMarkdown(entry)} />{/if}
    </section>
  {/each}
</Dialog>
