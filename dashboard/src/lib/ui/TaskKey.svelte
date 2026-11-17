<script lang="ts">
  import { isProvisional, PROVISIONAL_TIP, taskLabel } from '../keys';
  import type { Task } from '../types';
  import Icon from './Icon.svelte';

  // A task as people read it (ADR-007 §9): its key, else its ID; a provisional key lighter and in italics, with a small
  // ⓘ whose tooltip (the key's, on hover) says why (§3). `label` and `provisional` show a reference that isn't a loaded
  // task (lib/keys.ts refView).
  let {
    task,
    label,
    provisional,
  }: { task?: Pick<Task, 'id' | 'key' | 'key_final'>; label?: string; provisional?: boolean } = $props();
  const text = $derived(label ?? (task ? taskLabel(task) : ''));
  const draft = $derived(provisional ?? (task ? isProvisional(task) : false));
</script>

{#if draft}<span class="task-key provisional" title={PROVISIONAL_TIP}
    >{text}<span class="draft-info" aria-hidden="true"><Icon name="info" size={11} /></span></span
  >{:else}{text}{/if}

<style>
  .provisional {
    opacity: 0.7;
    font-style: italic;
    cursor: help;
  }
  .draft-info {
    display: inline-flex;
    margin-left: var(--space-1);
    vertical-align: middle;
  }
</style>
