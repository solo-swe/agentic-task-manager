<script lang="ts">
  import { formatWhen } from '../format';
  import { notesText, splitReleases, type Release } from '../releases';
  import { dashboard } from '../state.svelte';
  import { safeUrl } from '../url';

  // The releases (SR-7.5): each with its sprints, its tasks by stage, its PR and state, and the notes draft to copy,
  // as atm serves them with the tasks (ATM-720): over the whole history (archived tasks too), whatever the filters, as
  // a release ships all its tasks. The open ones first; the released ones fold into one row (T-323), newest first when
  // opened, as closed sprints do. Archived releases aren't shown.
  const parts = $derived(splitReleases(dashboard.releases));
  const shown = $derived(parts.open.length + parts.released.length);
  const folded = $derived(parts.released.length >= 2);
  let showReleased = $state(false);
  const releasedLabel = $derived.by(() => {
    const last = parts.released[0];
    return `✓ ${parts.released.length} released` + (last ? ` · last ${last.id}` : '');
  });
  let copied = $state<string | null>(null);

  async function copy(id: string, notes: string) {
    await navigator.clipboard.writeText(notes);
    copied = id;
  }
</script>

{#snippet row(r: Release)}
  {@const url = safeUrl(r.pr?.url)}
  {@const notes = notesText(r)}
  <div class="release-row" data-release={r.id} data-state={r.state}>
    <div class="release-head">
      <strong class="release-id">{r.id}</strong>
      {#if r.name}<span class="muted">“{r.name}”</span>{/if}
      <span class="sprint-state">{r.state}</span>
      {#if r.released_at}<span class="muted release-date"
          >{formatWhen(Date.parse(r.released_at), dashboard.loadedAt)}</span
        >{/if}
      {#if r.pr}
        {#if url}<a href={url} target="_blank" rel="noopener noreferrer">PR #{r.pr.number}</a>{:else}<span
            >PR #{r.pr.number}</span
          >{/if}
      {/if}
      {#if r.target && r.state === 'planned'}<span class="muted">target {r.target}</span>{/if}
      {#if r.sprints.length}<span class="muted release-sprints">{r.sprints.join(', ')}</span>{/if}
    </div>
    <!-- A release recorded without its tasks (recovered from git history) has nothing to count or draft -->
    {#if r.total || r.state !== 'released'}
      <div class="release-counts">
        {r.done} of {r.total} done · {r.in_pr} in PR · {r.merged} merged
      </div>
      <details class="release-notes">
        <summary
          >Notes draft <button class="link-button" onclick={() => copy(r.id, notes)}
            >{copied === r.id ? 'Copied' : 'Copy'}</button
          ></summary
        >
        <pre>{notes}</pre>
      </details>
    {/if}
  </div>
{/snippet}

{#if shown}
  <section class="release-view" id="release-view" aria-label="Releases">
    <h3 class="burndown-title">Releases</h3>
    {#each parts.open as r (r.id)}{@render row(r)}{/each}
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
      {#each parts.released as r (r.id)}{@render row(r)}{/each}
    {/if}
  </section>
{/if}
