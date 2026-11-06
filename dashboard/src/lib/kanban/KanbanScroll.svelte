<script lang="ts">
  import type { Snippet } from 'svelte';
  import HintScroll from '../ui/HintScroll.svelte';

  // A column's cards, scrolling on their own without a scrollbar (T-262), with "▲ N more" / "▼ N more" cards
  let { children }: { children: Snippet } = $props();
</script>

<HintScroll item=".kanban-card" outer="kanban-scroll" body="kanban-cards">
  {@render children()}
</HintScroll>
