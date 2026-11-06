<script lang="ts">
  // A commit or pull request reference (lib/repo.ts): a link to the repository host; plain text for a host atm can't
  // link yet; or, without a repository address, something that looks clickable and explains how to set one
  import type { Snippet } from 'svelte';
  import { REPOSITORY_HINT, type RepoRef } from '../repo';

  let {
    ref,
    title,
    className = '',
    children,
  }: { ref: RepoRef; title: string; className?: string; children: Snippet } = $props();
  let hint = $state(false);
</script>

{#if ref.kind === 'link'}
  <a
    class="repo-ref {className}"
    href={ref.href}
    target="_blank"
    rel="noopener noreferrer"
    {title}
    onclick={(e) => e.stopPropagation()}>{@render children()}</a
  >
{:else if ref.kind === 'hint'}
  <span class="repo-ref-wrap">
    <button
      class="repo-ref repo-ref-unset {className}"
      type="button"
      {title}
      aria-expanded={hint}
      onclick={(e) => {
        e.stopPropagation();
        hint = !hint;
      }}>{@render children()}</button
    >
    {#if hint}<span class="repo-hint" role="status">{REPOSITORY_HINT}</span>{/if}
  </span>
{:else}
  <span class={className} {title}>{@render children()}</span>
{/if}
