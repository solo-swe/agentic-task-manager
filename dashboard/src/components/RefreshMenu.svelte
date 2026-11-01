<script lang="ts">
  import { dashboard } from '../lib/state.svelte';
  import Icon from './Icon.svelte';

  // The reload button: it shows the auto-refresh state, and runs the auto-refresh timer. Its settings are in the ⋯
  // menu (SettingsMenu.svelte, T-323).
  const lastLoaded = $derived(
    new Date(dashboard.loadedAt).toLocaleTimeString(undefined, {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    }),
  );
  const status = $derived(dashboard.autoRefresh ? `every ${dashboard.autoRefreshSeconds} s` : 'off');

  $effect(() => {
    if (!dashboard.autoRefresh) return;
    const timer = setInterval(() => dashboard.reload(), dashboard.autoRefreshSeconds * 1000);
    return () => clearInterval(timer);
  });
</script>

<button
  class="menu-trigger reload-btn"
  class:spinning={dashboard.reloading}
  id="reload-btn"
  title="Reload tasks.json · auto-refresh {status} · last loaded {lastLoaded}"
  aria-label="Reload tasks.json"
  onclick={() => dashboard.reload()}
>
  <Icon name="refresh-cw" />
  <span class="refresh-state" class:off={!dashboard.autoRefresh}
    >{#if dashboard.autoRefresh}{dashboard.autoRefreshSeconds}s{:else}<Icon name="pause" size={12} />{/if}</span
  >
</button>
