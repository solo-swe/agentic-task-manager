<script lang="ts">
  import { taskLabel } from '../keys';
  import type { Task } from '../types';

  // A task as people read it (ADR-007 §9): its key, plain from creation (ADR-009), else its ID. `label` shows a
  // reference that isn't a loaded task (lib/keys.ts refView).
  let { task, label }: { task?: Pick<Task, 'id' | 'key'>; label?: string } = $props();
  const text = $derived(label ?? (task ? taskLabel(task) : ''));
</script>

{text}
