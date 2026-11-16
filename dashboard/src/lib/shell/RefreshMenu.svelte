<script lang="ts">
  import { dashboard } from '../state.svelte';
  import Icon from '../ui/Icon.svelte';

  // The reload button. The page reacts to changes, in the atm app (T-506) and from atm serve's change stream in the
  // browser (T-507): no timer, the button only reloads, for what the watcher can't see
  const lastLoaded = $derived(
    new Date(dashboard.loadedAt).toLocaleTimeString(undefined, {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    }),
  );
</script>

<button
  class="menu-trigger reload-btn"
  class:spinning={dashboard.reloading}
  id="reload-btn"
  title="Reload tasks.json · updates as tasks change · last loaded {lastLoaded}"
  aria-label="Reload tasks.json"
  onclick={() => dashboard.refresh()}
>
  <Icon name="refresh-cw" />
</button>
