<script lang="ts">
  import { ICONS, type IconName } from '../icons';

  let { name, size = 16 }: { name: IconName; size?: number } = $props();
</script>

<!-- The markup is a fixed, embedded icon (lib/icons.ts), never data from tasks.json -->
<svg class="icon icon-{name}" width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
  <!-- eslint-disable-next-line svelte/no-at-html-tags -->
  {@html ICONS[name]}
</svg>
