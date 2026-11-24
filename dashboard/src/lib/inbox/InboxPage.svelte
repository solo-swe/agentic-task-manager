<script lang="ts">
  // The Inbox item's page (docs/inbox-design.md §6, ATM-762): what the user notes for later, for an agent to make tasks
  // of. The quick-add box on top (focused, Enter adds, ⇧Enter a new line, its draft kept on its tab); the filter (Open,
  // Discarded, Injected, All; archived) in the address; each item by its first line, the rest on a click, with what
  // its state allows: Edit and Discard (asking why) while open, its reason and Restore once discarded, its tasks once
  // injected. Asked again after each change, and now and then while in view.
  import { onDestroy, onMount, untrack } from 'svelte';
  import { inboxListOf, withInboxList, type InboxListAddress } from '../address';
  import { inApp } from '../api/http';
  import {
    addItem,
    discardItem,
    editItem,
    loadInbox,
    restoreItem,
    type InboxItem,
    type InboxListing,
  } from '../api/inbox';
  import type { ChangeResult } from '../api/review-changes';
  import { appTabs } from '../app-tabs.svelte';
  import CompMd from '../documents/CompMd.svelte';
  import { formatAgoOrDate } from '../format';
  import { latest } from '../latest';
  import { currentAddress, navigate } from '../navigation';
  import { withTask } from '../routes';
  import { dashboard } from '../state.svelte';
  import { park, unpark } from '../tab-parked';
  import type { Validators } from '../api/http';
  import Dialog from '../ui/Dialog.svelte';

  const project = $derived(dashboard.projectId);
  const list = $derived(inboxListOf(dashboard.address));
  const FILTERS: { state: InboxListAddress['state']; label: string }[] = [
    { state: 'created', label: 'Open' },
    { state: 'discarded', label: 'Discarded' },
    { state: 'injected', label: 'Injected' },
    { state: 'all', label: 'All' },
  ];

  let listing = $state.raw<InboxListing | null>(null);
  let problem = $state<string | null>(null);
  let validators: Validators | undefined;
  let asked = '';
  const loads = latest();

  async function load() {
    const id = project;
    if (!id) return;
    const key = JSON.stringify([id, list]);
    // Another listing: asked afresh, not by the last one's validators
    if (key !== asked) validators = undefined;
    asked = key;
    const ask = loads.ask();
    try {
      const result = await loadInbox(id, list.state, list.archived, undefined, validators);
      if (!ask.current()) return;
      problem = null;
      if (!result.unchanged) [listing, validators] = [result.listing, result.validators];
    } catch (e) {
      if (ask.current()) problem = `The inbox couldn’t be read: ${(e as Error).message}`;
    } finally {
      ask.done();
    }
  }
  $effect(() => {
    void [project, list.state, list.archived];
    untrack(() => void load());
  });
  // Asked again every 10 s while the page is in view (an agent's triage changes it too)
  const timer = setInterval(() => document.visibilityState === 'visible' && void load(), 10_000);
  onDestroy(() => clearInterval(timer));

  function choose(next: InboxListAddress) {
    navigate(withInboxList(currentAddress(), next));
  }

  // The quick-add box: its draft kept on its tab (docs/tabs-and-navigation-design.md §4.2)
  // The tab it is on: the first one it is shown on (the app's first page is made before its tabs are started)
  let shownOn = untrack(() => (inApp ? appTabs.active || undefined : undefined));
  $effect(() => {
    if (inApp && !shownOn) shownOn = appTabs.active || undefined;
  });
  // By the tab alone: its page is one project's inbox at a time, and on the way back the project may not be known yet
  const draftKey = 'inbox-draft';
  let draft = $state(
    unpark<string>(
      untrack(() => shownOn),
      draftKey,
    ) ?? '',
  );
  onDestroy(() => {
    if (draft) park(shownOn, draftKey, draft);
  });
  let box = $state<HTMLTextAreaElement>();
  let adding = $state(false);
  // Focused when the page opens, unless the focus is somewhere already (the item tabs, walked with the arrow keys)
  onMount(() => {
    const at = document.activeElement;
    if (!at || at === document.body) box?.focus();
  });

  /** A change made: what it refused said, or the listing asked again */
  async function changed(result: ChangeResult): Promise<boolean> {
    if (!result.ok) {
      problem = result.message;
      return false;
    }
    problem = null;
    await load();
    return true;
  }

  async function add() {
    const text = draft.trim();
    if (!text || !project || adding) return;
    adding = true;
    if (await changed(await addItem(project, text))) draft = '';
    adding = false;
    box?.focus();
  }

  function onboxkey(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      void add();
    }
  }

  // An item's text: its first line as its title, the rest on a click
  const firstLine = (text: string) =>
    text
      .trim()
      .split('\n')[0]
      .replace(/^#+\s*/, '');
  const more = (text: string) => text.trim().includes('\n');
  let expanded = $state<string | null>(null);

  // Editing an open item, in place
  let editing = $state<{ id: string; text: string } | null>(null);
  async function saveEdit() {
    if (!editing || !project || !editing.text.trim()) return;
    if (await changed(await editItem(project, editing.id, editing.text.trim()))) editing = null;
  }

  // Discarding asks why
  let discarding = $state<{ id: string; reason: string } | null>(null);
  async function discard() {
    if (!discarding || !project || !discarding.reason.trim()) return;
    if (await changed(await discardItem(project, discarding.id, discarding.reason.trim()))) discarding = null;
  }

  async function restore(item: InboxItem) {
    if (project) await changed(await restoreItem(project, item.id));
  }

  const countOf = (state: InboxListAddress['state']) =>
    !listing ? null : state === 'all' ? listing.created + listing.discarded + listing.injected : listing[state];
  const ago = (at: string | undefined) => (at ? formatAgoOrDate(at, Date.now()) : '');
  /** A task the item became: its details over this page, when it is one of this project's */
  const taskLink = (key: string) => (dashboard.findTask(key) ? withTask(currentAddress(), key) : null);
</script>

<div class="inbox-page" id="inbox-page">
  <form
    class="inbox-add"
    onsubmit={(e) => {
      e.preventDefault();
      void add();
    }}
  >
    <textarea
      id="inbox-add-text"
      class="inbox-add-text"
      rows="2"
      placeholder="Jot something down… (Enter adds, ⇧Enter a new line)"
      aria-label="A note for the inbox"
      bind:this={box}
      bind:value={draft}
      onkeydown={onboxkey}></textarea>
    <button type="submit" class="inbox-button primary" id="inbox-add" disabled={adding || !draft.trim()}>Add</button>
  </form>

  <div class="inbox-filters" role="group" aria-label="Which items">
    {#each FILTERS as f (f.state)}
      {@const n = countOf(f.state)}
      <button
        class="inbox-chip"
        data-state={f.state}
        aria-pressed={list.state === f.state}
        onclick={() => choose({ ...list, state: f.state })}
        >{f.label}{#if n !== null}<span class="inbox-count">{n}</span>{/if}</button
      >
    {/each}
    <label class="inbox-archived"
      ><input
        type="checkbox"
        id="inbox-archived"
        checked={list.archived}
        onchange={(e) => choose({ ...list, archived: e.currentTarget.checked })}
      /> archived</label
    >
  </div>

  {#if problem}<p class="inbox-problem" role="alert" id="inbox-problem">{problem}</p>{/if}

  {#if listing && !listing.items.length}
    <p class="inbox-empty" id="inbox-empty">
      {list.state === 'created' && !list.archived
        ? 'Nothing in the inbox. Note something above: an agent turns it into tasks with you (/atm-triage-inbox).'
        : 'No item here.'}
    </p>
  {:else if listing}
    <ul class="inbox-items" id="inbox-items">
      {#each listing.items as item (item.id)}
        <li class="inbox-item" data-item={item.id} data-state={item.state}>
          <div class="inbox-item-head">
            <span class="inbox-id">{item.id}</span>
            {#if more(item.text)}
              <button
                class="inbox-title link"
                aria-expanded={expanded === item.id}
                onclick={() => (expanded = expanded === item.id ? null : item.id)}>{firstLine(item.text)}</button
              >
            {:else}
              <span class="inbox-title">{firstLine(item.text)}</span>
            {/if}
            {#if item.state === 'injected' && item.tasks?.length}
              <span class="inbox-tasks"
                >→ {#each item.tasks as key, i (key)}{i ? ', ' : ''}{#if taskLink(key)}<a
                      href={taskLink(key)}
                      data-task={key}>{key}</a
                    >{:else}{key}{/if}{/each}</span
              >
            {/if}
          </div>
          {#if expanded === item.id}<div class="inbox-text"><CompMd text={item.text} /></div>{/if}
          {#if editing?.id === item.id}
            <div class="inbox-edit">
              <textarea class="inbox-add-text" aria-label="The item's text" bind:value={editing.text} rows="3"
              ></textarea>
              <button class="inbox-button primary" onclick={() => void saveEdit()} disabled={!editing.text.trim()}
                >Save</button
              >
              <button class="inbox-button" onclick={() => (editing = null)}>Cancel</button>
            </div>
          {/if}
          <div class="inbox-meta">
            <span title={item.created_at}>{ago(item.created_at)}{item.created_by ? ` · ${item.created_by}` : ''}</span>
            {#if item.from_project}<span
                class="inbox-from"
                title="Left here by {item.created_by ?? 'someone'} working in {item.from_project}"
                >from {item.from_project}</span
              >{/if}
            {#if item.state === 'discarded'}<span class="inbox-reason">discarded: {item.reason}</span>{/if}
            <span class="inbox-actions">
              {#if item.state === 'created'}
                <button
                  class="inbox-button"
                  data-action="edit"
                  onclick={() => (editing = { id: item.id, text: item.text })}>Edit</button
                >
                <button
                  class="inbox-button"
                  data-action="discard"
                  onclick={() => (discarding = { id: item.id, reason: '' })}>Discard</button
                >
              {:else if item.state === 'discarded'}
                <button class="inbox-button" data-action="restore" onclick={() => void restore(item)}>Restore</button>
              {/if}
            </span>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<Dialog
  id="inbox-discard"
  title="Discard {discarding?.id ?? ''}"
  open={!!discarding}
  onclose={() => (discarding = null)}
>
  {#if discarding}
    <form
      class="inbox-discard"
      onsubmit={(e) => {
        e.preventDefault();
        void discard();
      }}
    >
      <label for="inbox-discard-reason">Why? It stays in the inbox's Discarded, and can be restored.</label>
      <textarea id="inbox-discard-reason" class="inbox-add-text" rows="3" bind:value={discarding.reason}></textarea>
      <div class="inbox-discard-actions">
        <button type="button" class="inbox-button" onclick={() => (discarding = null)}>Cancel</button>
        <button
          type="submit"
          class="inbox-button primary"
          id="inbox-discard-confirm"
          disabled={!discarding.reason.trim()}>Discard</button
        >
      </div>
    </form>
  {/if}
</Dialog>

<style>
  .inbox-page {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    max-width: 860px;
    padding: var(--space-7) var(--space-6);
  }
  .inbox-add,
  .inbox-edit {
    display: flex;
    gap: var(--space-4);
    align-items: flex-start;
  }
  .inbox-add-text {
    flex: 1;
    min-width: 0;
    padding: var(--space-4) var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg);
    color: var(--text);
    font: inherit;
    resize: vertical;
  }
  .inbox-add-text:focus {
    outline: none;
    border-color: var(--accent);
  }
  .inbox-filters {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
  }
  .inbox-chip,
  .inbox-button {
    height: 28px;
    padding: 0 var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: var(--text-sm);
    cursor: pointer;
  }
  .inbox-chip[aria-pressed='true'] {
    border-color: var(--accent);
    color: var(--accent);
    font-weight: 600;
  }
  .inbox-button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .inbox-button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .inbox-count {
    margin-left: var(--space-3);
    color: var(--text-muted);
  }
  .inbox-archived {
    margin-left: var(--space-5);
    color: var(--text-muted);
    font-size: var(--text-sm);
  }
  .inbox-items {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--surface);
  }
  .inbox-item {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-6) var(--space-7);
    border-top: 1px solid var(--border);
  }
  .inbox-item:first-child {
    border-top: none;
  }
  .inbox-item[data-state='discarded'] .inbox-title {
    color: var(--text-muted);
  }
  .inbox-item-head {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4);
    align-items: baseline;
  }
  .inbox-id {
    font-family: 'SF Mono', Menlo, Consolas, monospace;
    color: var(--text-muted);
  }
  .inbox-title {
    font-weight: 600;
  }
  .inbox-title.link {
    padding: 0;
    border: none;
    background: none;
    color: var(--text);
    font: inherit;
    font-weight: 600;
    text-align: left;
    cursor: pointer;
  }
  .inbox-tasks a {
    color: var(--accent);
  }
  .inbox-meta {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-6);
    align-items: center;
    color: var(--text-muted);
    font-size: var(--text-xs);
  }
  .inbox-from {
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    padding: 0 var(--space-3);
  }
  .inbox-actions {
    display: flex;
    gap: var(--space-3);
    margin-left: auto;
  }
  .inbox-empty,
  .inbox-problem {
    color: var(--text-muted);
  }
  .inbox-discard {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .inbox-discard-actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-3);
  }
</style>
