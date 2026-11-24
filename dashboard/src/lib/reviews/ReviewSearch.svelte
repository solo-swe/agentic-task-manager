<script lang="ts">
  // The reviews search (ATM-738): titles, summaries and comments, every word. In the title bar, where a project's
  // search is, with its look ("/" focuses it); the list it searches offers it (search.svelte.ts). What is typed goes
  // to the list's query a moment after typing stops, trimmed.
  import Icon from '../ui/Icon.svelte';
  import type { OfferedSearch } from './search.svelte';

  let { offered }: { offered: OfferedSearch } = $props();

  /** After typing stops, the query takes it */
  const SEARCH_PAUSE_MS = 250;

  // What is typed, until the query takes it, then the query's
  let text = $derived(offered.q);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let input = $state<HTMLInputElement>();

  function typed(next: string) {
    text = next;
    clearTimeout(timer);
    timer = setTimeout(() => offered.search(next.trim()), SEARCH_PAUSE_MS);
  }

  function clear() {
    clearTimeout(timer);
    text = '';
    offered.search('');
    input?.focus();
  }

  function onKeydown(e: KeyboardEvent) {
    // "/" focuses the search, unless the user is typing somewhere already
    const typing = e.target instanceof Element && e.target.closest('input, textarea, select, [contenteditable]');
    if (e.key === '/' && !e.metaKey && !e.ctrlKey && !typing) {
      e.preventDefault();
      input?.focus();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<label class="task-search review-search-box">
  <Icon name="search" size={14} />
  <input
    class="review-search"
    type="search"
    placeholder="Search reviews"
    aria-label="Search reviews"
    title="Search the reviews’ titles, summaries and comments (/)"
    bind:this={input}
    value={text}
    oninput={(e) => typed(e.currentTarget.value)}
  />
  {#if text}
    <button class="search-clear" aria-label="Clear search" onclick={clear}><Icon name="x" size={12} /></button>
  {:else}
    <kbd class="search-key" aria-hidden="true">/</kbd>
  {/if}
</label>
