<script lang="ts">
  import { componentLabel, taskComponents } from '../components';
  import { dashboard } from '../state.svelte';
  import type { Task } from '../types';

  // A task's components, by their labels (T-323), in the project's order
  let { task }: { task: Task } = $props();
  const ids = $derived(taskComponents(task, dashboard.components));
</script>

{#each ids as id (id)}<span class="badge component-chip" data-component={id} title="Component: {id}"
    >{componentLabel(dashboard.components, id)}</span
  >{/each}
