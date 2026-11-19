<script lang="ts">
  import Icon from '../ui/Icon.svelte';

  // The actions under a box that writes a comment or a reply (ATM-612): Cancel, *Add to my review* (a draft only you
  // see, sent with your decision) and *Comment now* (sent at once). The main one, the form's submit (and ⌘ Enter),
  // is *Comment now* until you have drafts, then *Add to my review*. Without `ondraft` (a page that keeps no drafts),
  // only Cancel and the send, as before.
  let {
    now,
    ready,
    drafting = false,
    reply = false,
    buttonClass = '',
    oncancel,
    ondraft,
  }: {
    /** Sending at once: the form's own submit unless drafting is the main action */
    now: () => void;
    /** Something to send (and not sending already) */
    ready: boolean;
    /** The user has drafts: adding one is the main action */
    drafting?: boolean;
    /** A reply's box: *Reply now*, *Reply* without drafts */
    reply?: boolean;
    /** The page's own button class (`chain-button`, `review-button`) */
    buttonClass?: string;
    oncancel: () => void;
    ondraft?: () => void;
  } = $props();

  const sendLabel = $derived(ondraft ? (reply ? 'Reply now' : 'Comment now') : reply ? 'Reply' : 'Comment');
</script>

<button type="button" class={buttonClass} onclick={oncancel}>Cancel</button>
{#if ondraft && drafting}
  <button type="button" class="{buttonClass} composer-now" disabled={!ready} onclick={now}>{sendLabel}</button>
  <button type="submit" class="{buttonClass} primary composer-draft" disabled={!ready}
    ><Icon name="plus" size={13} /> Add to my review</button
  >
{:else}
  {#if ondraft}
    <button type="button" class="{buttonClass} composer-draft" disabled={!ready} onclick={ondraft}
      ><Icon name="plus" size={13} /> Add to my review</button
    >
  {/if}
  <button type="submit" class="{buttonClass} primary composer-now" disabled={!ready}
    >{#if reply}<Icon name="send" size={13} />{/if}
    {sendLabel}</button
  >
{/if}
