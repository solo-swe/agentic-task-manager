<script lang="ts">
  import { dashboard } from '../state.svelte';
  import Icon from '../ui/Icon.svelte';

  // The reload button: it shows the auto-refresh state, and runs the auto-refresh timer. Its settings are in the ⋯
  // menu (SettingsMenu.svelte, T-323). In the atm app the page reacts to changes instead (T-506): no timer, no
  // interval shown, the button only reloads.
  const lastLoaded = $derived(
    new Date(dashboard.loadedAt).toLocaleTimeString(undefined, {
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
    }),
  );
  const status = $derived(
    dashboard.live
      ? 'updates as tasks change'
      : dashboard.autoRefresh
        ? `auto-refresh every ${dashboard.autoRefreshSeconds} s`
        : 'auto-refresh off',
  );

  $effect(() => {
    if (!dashboard.polls) return;
    const timer = setInterval(() => dashboard.reload(), dashboard.autoRefreshSeconds * 1000);
    return () => clearInterval(timer);
  });
</script>

<button
  class="menu-trigger reload-btn"
  class:spinning={dashboard.reloading}
  id="reload-btn"
  title="Reload tasks.json · {status} · last loaded {lastLoaded}"
  aria-label="Reload tasks.json"
  onclick={() => dashboard.reload()}
>
  <Icon name="refresh-cw" />
  {#if !dashboard.live}
    <span class="refresh-state" class:off={!dashboard.autoRefresh}
      >{#if dashboard.autoRefresh}{dashboard.autoRefreshSeconds}s{:else}<Icon name="pause" size={12} />{/if}</span
    >
  {/if}
</button>
