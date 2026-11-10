<script lang="ts">
  // After a plugin update the server restarts with the new dashboard; this page is still the old one. Offer a reload,
  // never reload by itself (docs/multi-project-dashboard-proposal.md 3.3). In the atm app, a newer version of the app
  // is offered the same way (T-483): installed only on the click, after the app has checked its signature.
  import { dashboard } from '../state.svelte';
  import Icon from '../ui/Icon.svelte';
</script>

{#if dashboard.appUpdate}
  <div class="update-notice app-update" id="app-update" role="status">
    <div class="app-update-text">
      <span>atm {dashboard.appUpdate.version} is available (you have {dashboard.appUpdate.current_version}).</span>
      {#if dashboard.appUpdate.notes}
        <details class="app-update-notes">
          <summary>What’s new</summary>
          <p>{dashboard.appUpdate.notes}</p>
        </details>
      {/if}
      {#if dashboard.updateError}
        <span class="app-update-error" role="alert">Couldn’t install it: {dashboard.updateError}</span>
      {/if}
    </div>
    <button
      class="update-reload"
      id="app-update-install"
      disabled={dashboard.installingUpdate}
      onclick={() => dashboard.installAppUpdate()}
    >
      <Icon name="refresh-cw" size={14} />
      {dashboard.installingUpdate ? 'Installing…' : 'Install and restart'}
    </button>
    <button
      class="update-dismiss"
      aria-label="Not now"
      title="Not now"
      disabled={dashboard.installingUpdate}
      onclick={() => dashboard.dismissAppUpdate()}><Icon name="x" size={14} /></button
    >
  </div>
{:else if dashboard.newVersionReady}
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
