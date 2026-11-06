<script lang="ts">
  // After a plugin update the server restarts with the new dashboard; this page is still the old one. Offer a reload,
  // never reload by itself (docs/multi-project-dashboard-proposal.md 3.3)
  import { dashboard } from '../state.svelte';
  import Icon from '../ui/Icon.svelte';
</script>

{#if dashboard.newVersionReady}
  <div class="update-notice" id="update-notice" role="status">
    <span>A new version of the dashboard is ready.</span>
    <button class="update-reload" id="update-reload" onclick={() => location.reload()}>
      <Icon name="refresh-cw" size={14} /> Reload
    </button>
    <button class="update-dismiss" aria-label="Not now" title="Not now" onclick={() => dashboard.dismissNewVersion()}
      ><Icon name="x" size={14} /></button
    >
  </div>
{/if}
