<script lang="ts">
  import { isProvisional, PROVISIONAL_TIP, taskLabel } from '../keys';
  import type { Task } from '../types';

  // A task as people read it (ADR-007 §9): its key, else its ID; a provisional key lighter, with a "draft" mark that
  // says why when pointed at (§3). `label` and `provisional` show a reference that isn't a loaded task (lib/keys.ts
  // refView).
  let {
    task,
    label,
    provisional,
  }: { task?: Pick<Task, 'id' | 'key' | 'key_final'>; label?: string; provisional?: boolean } = $props();
  const text = $derived(label ?? (task ? taskLabel(task) : ''));
  const draft = $derived(provisional ?? (task ? isProvisional(task) : false));
</script>

{#if draft}<span class="task-key provisional" title={PROVISIONAL_TIP}>{text}<span class="draft-mark">draft</span></span
  >{:else}{text}{/if}

<style>
  .provisional {
    opacity: 0.7;
    font-style: italic;
  }
  .draft-mark {
    margin-left: var(--space-2);
    padding: 0 var(--space-2);
    border: 1px dashed currentColor;
    border-radius: var(--radius-sm);
    font-size: var(--text-2xs);
    font-style: normal;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    vertical-align: middle;
  }
</style>
