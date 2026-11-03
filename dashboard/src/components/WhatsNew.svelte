<script lang="ts">
  import { entryMarkdown, UNRELEASED } from '../lib/changelog';
  import { releaseNotes } from '../lib/release-notes';
  import { dashboard } from '../lib/state.svelte';
  import CompMd from './CompMd.svelte';
  import HintScroll from './HintScroll.svelte';

  /** The notes render the first time What's new opens, and stay: CompMD loads only once the page shows Markdown */
  let opened = $state(false);
  $effect(() => {
    if (dashboard.whatsNewOpen) opened = true;
  });

  function onOverlayClick(e: MouseEvent) {
    if (e.target === e.currentTarget) dashboard.closeWhatsNew();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') dashboard.closeWhatsNew();
  }
</script>

<svelte:document onkeydown={onKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions (Escape closes, see above) -->
<div class="detail-overlay" class:open={dashboard.whatsNewOpen} id="whats-new-overlay" onclick={onOverlayClick}>
  <div class="detail-panel whats-new" id="whats-new-panel" role="dialog" aria-label="What's new">
    <!-- The title stays; the releases scroll below it without a scrollbar (T-282) -->
    <div class="detail-head">
      <h3>What's new</h3>
      <button class="detail-close" aria-label="Close" onclick={() => dashboard.closeWhatsNew()}>&times;</button>
    </div>
    <HintScroll item=".release" outer="whats-new-scroll" id="whats-new-body">
      {#each releaseNotes as entry (entry.version)}
        <section class="release" data-version={entry.version}>
          <h4>
            {entry.version === UNRELEASED ? 'Unreleased' : 'v' + entry.version}{#if entry.date}<span
                class="release-date">{entry.date}</span
              >{/if}
          </h4>
          {#if opened}<CompMd text={entryMarkdown(entry)} />{/if}
        </section>
      {/each}
    </HintScroll>
  </div>
</div>
