<script lang="ts">
  import CompMd from '../documents/CompMd.svelte';
  import type { Chain, ReviewComment } from './types';

  // A chain of comments, in place in the diff viewer (docs/code-review-design.md §3.4, §3.7, ATM-573): each comment
  // with its replies, a top-level comment's resolved box, and a reply box. Comments are task text (I1): rendered by
  // CompMD as task text is, never run. What a reply or a resolve does is the page's (`onreply`, `onresolve`); without
  // them the chain is only read.
  let {
    chain,
    onreply,
    onresolve,
  }: {
    chain: Chain;
    onreply?: (to: string, body: string) => void;
    onresolve?: (comment: string, resolved: boolean) => void;
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
</script>

{#snippet comment(c: ReviewComment, depth: number)}
  <li class="chain-comment" data-comment={c.id} data-depth={depth}>
    <div class="chain-meta">
      {#if c.resolved !== null}
        <label class="chain-resolved" title={c.resolved ? 'Resolved' : 'Open'}>
          <input
            type="checkbox"
            checked={c.resolved}
            disabled={!onresolve}
            onchange={(e) => onresolve?.(c.id, e.currentTarget.checked)}
          />
          <span class="visually-hidden">Resolved</span>
        </label>
      {/if}
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
        <textarea aria-label="Reply to {c.author}" bind:value={draft} rows="3"></textarea>
        <div class="chain-actions">
          <button type="button" onclick={() => (replyingTo = null)}>Cancel</button>
          <button type="submit" disabled={!draft.trim()}>Reply</button>
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

<section class="chain" data-chain={chain.id} aria-label="Comments on {where}">
  <header class="chain-where">{where}</header>
  {#if chain.anchor.quote}<blockquote class="chain-quote">{chain.anchor.quote}</blockquote>{/if}
  <ul>
    {#each chain.comments as c (c.id)}{@render comment(c, 0)}{/each}
  </ul>
</section>

<style>
  .chain {
    margin: var(--space-3) var(--space-4) var(--space-4) var(--space-10);
    padding: var(--space-4) var(--space-6);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface);
    font-family: var(--font-sans);
    font-size: var(--text-md);
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
    color: var(--text-muted);
    font-size: var(--text-xs);
    margin-bottom: var(--space-2);
  }
  .chain-quote {
    margin: 0 0 var(--space-3);
    padding: var(--space-1) var(--space-4);
    border-left: 3px solid var(--border);
    color: var(--text-muted);
    white-space: pre-wrap;
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
  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
</style>
