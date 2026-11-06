<script lang="ts">
  import { formatWhen } from '../format';
  import { releaseSummaries, splitReleases, type ReleaseSummary } from '../releases';
  import { dashboard } from '../state.svelte';
  import { safeUrl } from '../url';

  // The releases (SR-7.5): each with its sprints, its tasks by stage, its PR and state, and the notes draft to copy.
  // Over the whole history (archived tasks too, once loaded), whatever the filters: a release ships all its tasks.
  // The open ones first; the released ones fold into one row (T-323), newest first when opened, as closed sprints do.
  const releases = $derived(releaseSummaries(dashboard.meta, dashboard.history, dashboard.sprints, dashboard.vocab));
  const parts = $derived(splitReleases(releases));
  const folded = $derived(parts.released.length >= 2);
  let showReleased = $state(false);
  const releasedLabel = $derived.by(() => {
    const last = parts.released[0];
    return `✓ ${parts.released.length} released` + (last ? ` · last ${last.release.id}` : '');
  });
  let copied = $state<string | null>(null);

  async function copy(id: string, notes: string) {
    await navigator.clipboard.writeText(notes);
    copied = id;
  }
</script>

{#snippet row(r: ReleaseSummary)}
  {@const url = safeUrl(r.release.pr?.url)}
  <div class="release-row" data-release={r.release.id} data-state={r.release.state}>
    <div class="release-head">
      <strong class="release-id">{r.release.id}</strong>
      {#if r.release.name}<span class="muted">“{r.release.name}”</span>{/if}
      <span class="sprint-state">{r.release.state}</span>
      {#if r.release.released_at}<span class="muted release-date"
          >{formatWhen(Date.parse(r.release.released_at), dashboard.loadedAt)}</span
        >{/if}
      {#if r.release.pr}
        {#if url}<a href={url} target="_blank" rel="noopener noreferrer">PR #{r.release.pr.number}</a>{:else}<span
            >PR #{r.release.pr.number}</span
          >{/if}
      {/if}
      {#if r.release.target && r.release.state === 'planned'}<span class="muted">target {r.release.target}</span>{/if}
      {#if r.sprints.length}<span class="muted release-sprints">{r.sprints.join(', ')}</span>{/if}
    </div>
    <!-- A release recorded without its tasks (recovered from git history) has nothing to count or draft -->
    {#if r.total || r.release.state !== 'released'}
      <div class="release-counts">
        {r.done} of {r.total} done · {r.inPr} in PR · {r.merged} merged
      </div>
      <details class="release-notes">
        <summary
          >Notes draft <button class="link-button" onclick={() => copy(r.release.id, r.notes)}
            >{copied === r.release.id ? 'Copied' : 'Copy'}</button
          ></summary
        >
        <pre>{r.notes}</pre>
      </details>
    {/if}
  </div>
{/snippet}

{#if releases.length}
  <section class="release-view" id="release-view" aria-label="Releases">
    <h3 class="burndown-title">Releases</h3>
    {#each parts.open as r (r.release.id)}{@render row(r)}{/each}
    {#if folded}
      <button
        class="phase-finished-toggle release-released-toggle"
        id="release-released-toggle"
        aria-expanded={showReleased}
        onclick={() => (showReleased = !showReleased)}
        ><span>{releasedLabel}</span><span class="phase-finished-action">{showReleased ? 'Hide' : 'Show'}</span></button
      >
    {/if}
    {#if !folded || showReleased}
      {#each parts.released as r (r.release.id)}{@render row(r)}{/each}
    {/if}
  </section>
{/if}
