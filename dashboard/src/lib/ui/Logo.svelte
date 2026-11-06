<script lang="ts">
  // The ATM logo (src/assets/atm-logo.svg), in the colour of its surroundings (currentColor)
  import logo from '../../assets/atm-logo.svg?raw';

  let { size = 24, id }: { size?: number; id?: string } = $props();
  const plain = logo.replace(/<!--[\s\S]*?-->/g, '');
  const svg = $derived(plain.replace('<svg ', `<svg width="${size}" height="${size}" `));
</script>

<!-- The markup is the embedded logo file, never data from tasks.json -->
<span class="atm-logo" {id} aria-hidden="true">
  <!-- eslint-disable-next-line svelte/no-at-html-tags -->
  {@html svg}
</span>
