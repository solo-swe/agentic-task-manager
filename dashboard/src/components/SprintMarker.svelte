<script lang="ts">
  // A sprint's state at a glance (T-323): ▶ the active sprint, ✓ a closed one; a planned sprint has none
  import type { SprintState } from '../lib/sprints';
  import Icon from './Icon.svelte';

  let { state }: { state: SprintState | undefined } = $props();
</script>

{#if state === 'active'}
  <span class="sprint-marker active" title="The active sprint" aria-label="active"><Icon name="play" size={9} /></span>
{:else if state === 'closed'}
  <span class="sprint-marker closed" title="Closed" aria-label="closed"><Icon name="check" size={11} /></span>
{/if}
