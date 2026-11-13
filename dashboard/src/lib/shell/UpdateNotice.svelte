<script lang="ts">
  // After a plugin update the server restarts with the new dashboard; this page is still the old one. Offer a reload,
  // never reload by itself (docs/multi-project-dashboard-proposal.md 3.3). In the atm app, a newer version of the app
  // is offered the same way (T-483): installed only on the click, after the app has checked its signature.
  // The install shows how it goes (T-526): the download's progress, then installing and restarting; if the app is
  // still here after the restart, the update is installed and Restart Now tries again.
  import { dashboard } from '../state.svelte';
  import Icon from '../ui/Icon.svelte';

  const MB = 1024 * 1024;
  const mb = (bytes: number) => (bytes / MB).toFixed(1);
  const download = $derived(dashboard.updateDownload);
  const step = $derived.by(() => {
    switch (dashboard.updateStep) {
      case 'downloading':
        if (!download) return 'Downloading…';
        return download.total
          ? `Downloading… ${mb(download.downloaded)} of ${mb(download.total)} MB`
          : `Downloading… ${mb(download.downloaded)} MB`;
      case 'installing':
        return 'Installing…';
      case 'restarting':
        return 'Restarting…';
      default:
        return null;
    }
  });
</script>

{#if dashboard.appUpdate}
  <div class="update-notice app-update" id="app-update" role="status">
    <div class="app-update-text">
      {#if dashboard.updateStep === 'installed'}
        <span>Agentic Task Manager {dashboard.appUpdate.version} is installed: restart to use it.</span>
      {:else}
        <span
          >Agentic Task Manager {dashboard.appUpdate.version} is available (you have {dashboard.appUpdate
            .current_version}).</span
        >
      {/if}
      {#if step}
        <span class="app-update-step" id="app-update-step">{step}</span>
        {#if dashboard.updateStep === 'downloading'}
          <progress
            class="app-update-progress"
            aria-label="Downloading the update"
            max={download?.total ?? undefined}
            value={download?.total ? download.downloaded : undefined}
          ></progress>
        {/if}
      {/if}
      {#if dashboard.appUpdate.notes && !step}
        <details class="app-update-notes">
          <summary>What’s new</summary>
          <p>{dashboard.appUpdate.notes}</p>
        </details>
      {/if}
      {#if dashboard.updateError}
        <span class="app-update-error" role="alert">Couldn’t install it: {dashboard.updateError}</span>
      {/if}
    </div>
    {#if dashboard.updateStep === 'installed'}
      <button class="update-reload" id="app-update-restart" onclick={() => dashboard.restartApp()}>
        <Icon name="refresh-cw" size={14} /> Restart now
      </button>
    {:else}
      <button
        class="update-reload"
        id="app-update-install"
        disabled={dashboard.installingUpdate}
        onclick={() => dashboard.installAppUpdate()}
      >
        <Icon name="refresh-cw" size={14} /> Install and restart
      </button>
    {/if}
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
