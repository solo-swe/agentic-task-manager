<script lang="ts">
  import CompMd from '../documents/CompMd.svelte';
  import { formatWhen } from '../format';
  import Icon from '../ui/Icon.svelte';
  import { focusOnShow } from './focus';
  import { documentLinks } from './links';
  import type { Chain, ReviewComment } from './types';

  // A chain of comments (docs/code-review-design.md §3.4, §3.7, ATM-573, ATM-574), in place under its lines in the diff
  // viewer: its lines (the diff shows the file and the text, so the chain doesn't repeat them), each comment with its
  // author and time, its replies, Resolve while open and a resolved check (and Reopen) once resolved; a resolved
  // chain folds to one line until shown, Reopen still at hand. Hovering or focusing it, or clicking it, selects it
  // (`onselect`): the diff marks its lines. `detailed` (the Comments tab, away from the diff), it names its file,
  // as a link (`onwhere`), and shows the text it was made on. Comments are task text (I1): rendered by CompMD, never
  // run. What a reply or a resolve does is the page's (`onreply`, `onresolve`); without them the chain is only read.
  let {
    chain,
    detailed = false,
    selected = false,
    onreply,
    onresolve,
    onwhere,
    onselect,
  }: {
    chain: Chain;
    detailed?: boolean;
    selected?: boolean;
    onreply?: (to: string, body: string) => void;
    onresolve?: (comment: string, resolved: boolean) => void;
    onwhere?: () => void;
    /** Selected (true) while hovered or focused, or clicked to keep it so; let go (false) */
    onselect?: (selected: boolean, keep: boolean) => void;
  } = $props();

  /** A link in a comment to a document of the project: what the review's page does with it (ATM-586) */
  const openLink = documentLinks();

  let replyingTo = $state<string | null>(null);
  let draft = $state('');

  function send() {
    const body = draft.trim();
    if (!body || !replyingTo) return;
    onreply?.(replyingTo, body);
    draft = '';
    replyingTo = null;
  }

  const a = $derived(chain.anchor);
  const lines = $derived(
    a.lines ? (a.lines[0] === a.lines[1] ? `Line ${a.lines[0]}` : `Lines ${a.lines[0]}–${a.lines[1]}`) : null,
  );
  /** Where it is: its lines (and side); `detailed`, its file too; the review's own, the review */
  const where = $derived(
    !a.path
      ? 'The review'
      : [detailed ? a.path : null, lines ?? (detailed ? null : 'The file'), a.side === 'old' ? 'before' : null]
          .filter(Boolean)
          .join(' · '),
  );

  const resolved = $derived(chain.comments.length > 0 && chain.comments.every((c) => c.resolved === true));
  /** A resolved chain opened to read it */
  let opened = $state(false);
  const folded = $derived(resolved && !opened);
  const first = $derived(chain.comments[0]);

  const when = (at: string) => {
    const ts = Date.parse(at);
    return Number.isNaN(ts) ? at : formatWhen(ts, Date.now());
  };
  const initials = (name: string) =>
    name
      .split(/[\s:._-]+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((w) => w[0]!.toUpperCase())
      .join('');
</script>

{#snippet comment(c: ReviewComment, depth: number)}
  <li class="chain-comment" data-comment={c.id} data-depth={depth}>
    {#if c.resolved !== null && onresolve && !c.resolved}
      <div class="chain-state">
        <button class="chain-button chain-resolve" onclick={() => onresolve?.(c.id, true)}
          ><Icon name="circle-check" size={14} /> Resolve</button
        >
      </div>
    {:else if c.resolved}
      <div class="chain-state resolved">
        <Icon name="circle-check" size={14} />
        <span class="chain-state-label">Resolved</span>
        {#if onresolve}
          <button class="chain-button" onclick={() => onresolve?.(c.id, false)}
            ><Icon name="rotate-ccw" size={13} /> Reopen</button
          >
        {/if}
      </div>
    {/if}
    <div class="chain-meta">
      <span class="chain-avatar" aria-hidden="true">{initials(c.author)}</span>
      <strong class="chain-author">{c.author}</strong>
      <time class="chain-time" datetime={c.at}>{when(c.at)}</time>
    </div>
    <div class="chain-body"><CompMd text={c.body} onopen={openLink} /></div>
    {#if onreply}
      {#if replyingTo === c.id}
        <form
          class="chain-reply"
          onsubmit={(e) => {
            e.preventDefault();
            send();
          }}
        >
          <textarea
            aria-label="Reply to {c.author}"
            bind:value={draft}
            rows="3"
            placeholder="Reply…"
            {@attach focusOnShow}
            onkeydown={(e) => {
              if (e.key === 'Escape') replyingTo = null;
              if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) send();
            }}></textarea>
          <div class="chain-actions">
            <button type="button" class="chain-button" onclick={() => (replyingTo = null)}>Cancel</button>
            <button type="submit" class="chain-button primary" disabled={!draft.trim()}
              ><Icon name="send" size={13} /> Reply</button
            >
          </div>
        </form>
      {:else}
        <button class="chain-link chain-reply-button" onclick={() => (replyingTo = c.id)}
          ><Icon name="reply" size={13} /> Reply</button
        >
      {/if}
    {/if}
    {#if c.replies.length}
      <ul class="chain-replies">
        {#each c.replies as reply (reply.id)}{@render comment(reply, depth + 1)}{/each}
      </ul>
    {/if}
  </li>
{/snippet}

<!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_click_events_have_key_events (hovering or
     clicking a chain marks its lines; focusing it, with the keyboard, does too) -->
<section
  class="chain"
  class:resolved
  class:selected
  data-chain={chain.id}
  data-resolved={resolved ? '' : undefined}
  aria-label="Comments on {where}"
  onmouseenter={() => onselect?.(true, false)}
  onmouseleave={() => onselect?.(false, false)}
  onfocusin={() => onselect?.(true, false)}
  onclick={(e) => {
    if (!(e.target as Element).closest('button, a, textarea, input')) onselect?.(true, true);
  }}
>
  <header class="chain-where">
    {#if onwhere}<button class="chain-link chain-where-link" onclick={onwhere}>{where}</button>{:else}<span
        >{where}</span
      >{/if}
    {#if resolved}
      <span class="chain-folded-note"
        ><Icon name="circle-check" size={13} /> Resolved{folded && first ? ` · ${first.author}` : ''}</span
      >
      {#if folded && onresolve && first}
        <button class="chain-button" onclick={() => onresolve?.(first.id, false)}
          ><Icon name="rotate-ccw" size={13} /> Reopen</button
        >
      {/if}
      <button class="chain-link chain-toggle" aria-expanded={!folded} onclick={() => (opened = !opened)}
        >{folded ? 'Show' : 'Hide'}</button
      >
    {/if}
  </header>
  {#if !folded}
    {#if detailed && a.quote}<blockquote class="chain-quote">{a.quote}</blockquote>{/if}
    <ul class="chain-comments">
      {#each chain.comments as c (c.id)}{@render comment(c, 0)}{/each}
    </ul>
  {/if}
</section>

<style>
  .chain {
    margin: var(--space-4) var(--space-6) var(--space-4) var(--space-10);
    padding: var(--space-5) var(--space-7);
    border: 1px solid var(--border);
    border-left: 3px solid var(--accent);
    border-radius: var(--radius-lg);
    background: var(--surface);
    font-family: var(--font-sans);
    font-size: var(--text-md);
    line-height: 1.5;
    white-space: normal;
    transition:
      border-color 0.12s ease,
      box-shadow 0.12s ease;
  }
  .chain.resolved {
    border-left-color: var(--green);
    padding-top: var(--space-4);
    padding-bottom: var(--space-4);
  }
  .chain.selected {
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 35%, transparent);
  }
  .chain ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .chain-comment + .chain-comment {
    margin-top: var(--space-6);
    padding-top: var(--space-6);
    border-top: 1px solid var(--border);
  }
  .chain-replies {
    margin: var(--space-4) 0 0 var(--space-5);
    padding-left: var(--space-6);
    border-left: 2px solid var(--border);
  }
  .chain-replies > .chain-comment + .chain-comment {
    border-top: none;
    padding-top: 0;
  }
  .chain-where {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    min-height: var(--space-9);
    color: var(--text-muted);
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.02em;
  }
  .chain-folded-note {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--green);
  }
  .chain-toggle {
    margin-left: auto;
  }
  .chain-quote {
    margin: var(--space-2) 0 var(--space-4);
    padding: var(--space-2) var(--space-5);
    border-left: 3px solid var(--border);
    color: var(--text-muted);
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    font-size: var(--text-sm);
    white-space: pre-wrap;
  }
  /* Where a top-level comment stands, above it */
  .chain-state {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin: var(--space-2) 0 var(--space-4);
    font-size: var(--text-xs);
  }
  .chain-state.resolved {
    padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4);
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--green) 12%, transparent);
    color: var(--green);
  }
  .chain-state-label {
    font-weight: 600;
  }
  .chain-state.resolved .chain-button {
    margin-left: auto;
  }
  .chain-meta {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }
  .chain-avatar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--space-9);
    height: var(--space-9);
    border-radius: var(--radius-round);
    background: color-mix(in srgb, var(--accent) 18%, var(--surface2));
    color: var(--accent);
    font-size: var(--text-2xs);
    font-weight: 700;
  }
  .chain-author {
    font-weight: 600;
  }
  .chain-time {
    color: var(--text-muted);
    font-size: var(--text-xs);
  }
  .chain-body {
    margin: var(--space-3) 0 var(--space-3) calc(var(--space-9) + var(--space-4));
  }
  .chain-reply-button {
    margin-left: calc(var(--space-9) + var(--space-4));
  }
  .chain-reply {
    margin: var(--space-3) 0 0 calc(var(--space-9) + var(--space-4));
  }
  .chain-reply textarea {
    width: 100%;
    box-sizing: border-box;
    padding: var(--space-4) var(--space-5);
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    font: inherit;
    resize: vertical;
  }
  .chain-reply textarea:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 20%, transparent);
  }
  .chain-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-3);
    margin-top: var(--space-3);
  }
  .chain-button {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1) var(--space-4);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg);
    color: var(--text-muted);
    font: inherit;
    font-size: var(--text-xs);
    font-weight: 500;
    cursor: pointer;
  }
  .chain-button:hover {
    color: var(--text);
    border-color: color-mix(in srgb, var(--text-muted) 60%, var(--border));
  }
  .chain-button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .chain-button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .chain-resolve:hover {
    color: var(--green);
    border-color: var(--green);
  }
  .chain-link {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0;
    border: none;
    background: none;
    color: var(--accent);
    font: inherit;
    font-size: var(--text-xs);
    cursor: pointer;
  }
  .chain-link:hover {
    text-decoration: underline;
  }
  .chain-where-link {
    font-weight: 600;
  }
</style>
