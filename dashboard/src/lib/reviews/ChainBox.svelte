<script lang="ts">
  import CompMd from '../documents/CompMd.svelte';
  import { focusOnShow } from './focus';
  import type { Chain, ReviewComment } from './types';

  // A chain of comments, in place in the diff viewer (docs/code-review-design.md §3.4, §3.7, ATM-573): each comment
  // with its replies, a reply box, and above a top-level comment where it stands: a Resolve button while it is open,
  // a resolved check (and Reopen) once it is. A resolved chain folds to that check until it is opened. Comments are
  // task text (I1): rendered by CompMD as task text is, never run. What a reply or a resolve does is the page's
  // (`onreply`, `onresolve`); without them the chain is only read. `onwhere` makes its place a link (the Comments tab).
  let {
    chain,
    onreply,
    onresolve,
    onwhere,
  }: {
    chain: Chain;
    onreply?: (to: string, body: string) => void;
    onresolve?: (comment: string, resolved: boolean) => void;
    onwhere?: () => void;
  } = $props();

  let replyingTo = $state<string | null>(null);
  let draft = $state('');

  function send() {
    const body = draft.trim();
    if (!body || !replyingTo) return;
    onreply?.(replyingTo, body);
    draft = '';
    replyingTo = null;
  }

  const where = $derived.by(() => {
    const a = chain.anchor;
    if (!a.path) return 'The review';
    const lines = a.lines ? (a.lines[0] === a.lines[1] ? `:${a.lines[0]}` : `:${a.lines[0]}–${a.lines[1]}`) : '';
    return `${a.path}${lines}${a.side === 'old' ? ' (before)' : ''}`;
  });

  const resolved = $derived(chain.comments.length > 0 && chain.comments.every((c) => c.resolved === true));
  /** A resolved chain opened to read it */
  let opened = $state(false);
  const folded = $derived(resolved && !opened);
</script>

{#snippet comment(c: ReviewComment, depth: number)}
  <li class="chain-comment" data-comment={c.id} data-depth={depth}>
    {#if c.resolved !== null}
      <div class="chain-state" class:resolved={c.resolved}>
        {#if c.resolved}
          <span class="chain-check" aria-hidden="true">✓</span>
          <span class="chain-state-label">Resolved</span>
          {#if onresolve}
            <button class="chain-state-button" onclick={() => onresolve?.(c.id, false)}>Reopen</button>
          {/if}
        {:else if onresolve}
          <button class="chain-state-button chain-resolve" onclick={() => onresolve?.(c.id, true)}
            ><span aria-hidden="true">✓</span> Resolve</button
          >
        {/if}
      </div>
    {/if}
    <div class="chain-meta">
      <strong>{c.author}</strong>
      <span class="chain-time">{c.at}</span>
      {#if onreply}
        <button class="chain-reply-button" onclick={() => (replyingTo = replyingTo === c.id ? null : c.id)}
          >Reply</button
        >
      {/if}
    </div>
    <div class="chain-body"><CompMd text={c.body} /></div>
    {#if replyingTo === c.id}
      <form
        class="chain-reply"
        onsubmit={(e) => {
          e.preventDefault();
          send();
        }}
      >
        <textarea aria-label="Reply to {c.author}" bind:value={draft} rows="3" {@attach focusOnShow}></textarea>
        <div class="chain-actions">
          <button type="button" onclick={() => (replyingTo = null)}>Cancel</button>
          <button type="submit" class="primary" disabled={!draft.trim()}>Reply</button>
        </div>
      </form>
    {/if}
    {#if c.replies.length}
      <ul>
        {#each c.replies as reply (reply.id)}{@render comment(reply, depth + 1)}{/each}
      </ul>
    {/if}
  </li>
{/snippet}

<section
  class="chain"
  class:resolved
  data-chain={chain.id}
  data-resolved={resolved ? '' : undefined}
  aria-label="Comments on {where}"
>
  <header class="chain-where">
    {#if onwhere}<button class="chain-where-link" onclick={onwhere}>{where}</button>{:else}<span>{where}</span>{/if}
    {#if resolved}
      <span class="chain-folded-note">
        <span class="chain-check" aria-hidden="true">✓</span> Resolved{folded
          ? ` · ${chain.comments[0]?.author ?? ''}`
          : ''}
      </span>
      <button class="chain-toggle" aria-expanded={!folded} onclick={() => (opened = !opened)}
        >{folded ? 'Show' : 'Hide'}</button
      >
    {/if}
  </header>
  {#if !folded}
    {#if chain.anchor.quote}<blockquote class="chain-quote">{chain.anchor.quote}</blockquote>{/if}
    <ul>
      {#each chain.comments as c (c.id)}{@render comment(c, 0)}{/each}
    </ul>
  {/if}
</section>

<style>
  .chain {
    margin: var(--space-3) var(--space-4) var(--space-4) var(--space-10);
    padding: var(--space-4) var(--space-6);
    border: 1px solid var(--border);
    border-left: 3px solid var(--accent);
    border-radius: var(--radius-md);
    background: var(--surface);
    font-family: var(--font-sans);
    font-size: var(--text-md);
    box-shadow: 0 1px 2px color-mix(in srgb, var(--text) 8%, transparent);
  }
  .chain.resolved {
    border-left-color: var(--green);
  }
  .chain ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .chain-comment ul {
    margin-left: var(--space-7);
    border-left: 2px solid var(--border);
    padding-left: var(--space-4);
  }
  .chain-where {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    color: var(--text-muted);
    font-size: var(--text-xs);
    margin-bottom: var(--space-2);
  }
  .chain-where-link {
    padding: 0;
    border: none;
    background: none;
    color: var(--accent);
    font: inherit;
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    cursor: pointer;
  }
  .chain-folded-note {
    color: var(--green);
  }
  .chain-toggle {
    margin-left: auto;
    padding: 0;
    border: none;
    background: none;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
  }
  .chain-quote {
    margin: 0 0 var(--space-3);
    padding: var(--space-1) var(--space-4);
    border-left: 3px solid var(--border);
    color: var(--text-muted);
    white-space: pre-wrap;
  }
  /* Where a top-level comment stands, above it */
  .chain-state {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin-bottom: var(--space-3);
    font-size: var(--text-xs);
  }
  .chain-state.resolved {
    padding: var(--space-2) var(--space-4);
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--green) 12%, transparent);
    color: var(--green);
  }
  .chain-check {
    font-weight: 700;
  }
  .chain-state-label {
    font-weight: 600;
  }
  .chain-state-button {
    padding: var(--space-1) var(--space-4);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg);
    color: var(--text-muted);
    font: inherit;
    cursor: pointer;
  }
  .chain-state.resolved .chain-state-button {
    margin-left: auto;
  }
  .chain-resolve:hover {
    color: var(--green);
    border-color: var(--green);
  }
  .chain-meta {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  .chain-time {
    color: var(--text-muted);
    font-size: var(--text-xs);
  }
  .chain-reply-button {
    margin-left: auto;
    border: none;
    background: none;
    color: var(--accent);
    cursor: pointer;
    font: inherit;
    font-size: var(--text-xs);
  }
  .chain-body {
    margin: var(--space-1) 0 var(--space-4);
  }
  .chain-reply textarea {
    width: 100%;
    box-sizing: border-box;
    padding: var(--space-3);
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    font: inherit;
  }
  .chain-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-3);
    margin: var(--space-2) 0 var(--space-4);
  }
  .chain-actions button {
    padding: var(--space-2) var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg);
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .chain-actions button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .chain-actions button:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
