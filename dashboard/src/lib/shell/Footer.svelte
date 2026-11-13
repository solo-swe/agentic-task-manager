<script lang="ts">
  import { inApp } from '../api/http';
  import { archiveNotice } from '../archive';
  import { problemCount } from '../load';
  import { copyright, pluginInfo } from '../plugin-info';
  import { dashboard } from '../state.svelte';
  import Logo from '../ui/Logo.svelte';

  // tasks.json has grown past the archive threshold (D-5.4): atm's own hint, the one agents act on (T-372)
  const notice = $derived(archiveNotice(dashboard.archiveHint));
  // A malformed tasks.json, shown as far as atm could read it (T-377): how much is wrong, the file's own problems in
  // the tooltip (each task's are in its details)
  const problems = $derived(problemCount(dashboard.fileProblems, dashboard.tasks));

  // In the app, the agents at work: the project's live claims, on the overview every project's
  const working = $derived(
    dashboard.onProject
      ? dashboard.claims.size
      : dashboard.projects.reduce((n, p) => n + (p.summary?.agents_working ?? 0), 0),
  );
  function showAgents() {
    if (dashboard.onProject) dashboard.selectTab('agents');
    else dashboard.selectOverviewTab('agents');
  }
</script>

{#snippet notices()}
  {#if notice}<span class="archive-notice" id="archive-notice">{notice}</span>{/if}
  {#if problems}<span
      class="archive-notice"
      id="problems-notice"
      title={[...dashboard.fileProblems, 'Run `atm check` for the list.'].join('\n')}
      >atm check finds {problems} problem{problems === 1 ? '' : 's'} in tasks.json</span
    >{/if}
{/snippet}

<!-- The version (What's new), GitHub, Help and About: the browser's footer and the app's status bar both have them -->
{#snippet links()}
  <nav class="footer-links" aria-label="About this dashboard">
    <button class="footer-link" id="footer-version" title="What's new" onclick={() => dashboard.openWhatsNew()}
      >v{pluginInfo.version}</button
    >
    <a class="footer-link" id="footer-github" href={pluginInfo.repository} target="_blank" rel="noopener noreferrer"
      >GitHub</a
    >
    <button class="footer-link" id="footer-help" onclick={() => dashboard.openDialog('help')}>Help</button>
    <button class="footer-link" id="footer-about" onclick={() => dashboard.openDialog('about')}>About</button>
  </nav>
{/snippet}

{#if inApp}
  <!-- In the atm app (T-516), a status bar in place of the footer: whether the page is up to date and the agents at
       work at the left; the notices, the copyright, the version and the links (Hakan: easier to find than the menus')
       at the right. Connect your agents is in the menu -->

  <footer class="status-bar" id="status-bar">
    <span class="status-live" class:off={dashboard.loadError !== null} id="status-live"
      ><span class="status-dot" aria-hidden="true"></span>{dashboard.loadError === null
        ? 'Up to date'
        : 'Can’t read the project'}</span
    >
    {#if working}
      <button class="status-item" id="status-agents" onclick={showAgents}
        >{working} agent{working === 1 ? '' : 's'} working</button
      >
    {/if}
    <span class="status-end">
      {@render notices()}
      <span class="footer-copyright">{copyright(pluginInfo)}</span>
      {@render links()}
    </span>
  </footer>
{:else}
  <footer class="site-footer" id="site-footer">
    <span class="footer-start">
      <span class="footer-brand" id="footer-brand"
        ><Logo size={16} id="footer-logo" />{pluginInfo.fullName} ({pluginInfo.name})</span
      >
      <span class="footer-copyright">{copyright(pluginInfo)}</span>
    </span>
    {@render notices()}
    {@render links()}
  </footer>
{/if}

<style>
  /* The app's status bar: one line at the window's bottom, edge to edge (out of the page's padding, body in app.css) */
  .status-bar {
    align-items: center;
    background: var(--surface2);
    border-top: 1px solid var(--border);
    bottom: 0;
    color: var(--text-muted);
    display: flex;
    font-size: var(--text-xs);
    gap: var(--space-4);
    height: 24px;
    margin: var(--space-6) calc(-1 * var(--space-9)) calc(-1 * var(--space-9));
    margin-left: calc(-1 * (var(--space-9) + var(--sidebar-offset, 0px)));
    padding: 0 var(--space-4);
    padding-left: calc(var(--space-4) + var(--sidebar-offset, 0px));
    position: sticky;
    z-index: 20;
  }
  .status-live {
    align-items: center;
    display: inline-flex;
    gap: var(--space-2);
  }
  .status-dot {
    background: var(--green);
    border-radius: var(--radius-round);
    height: 7px;
    width: 7px;
  }
  .status-live.off .status-dot {
    background: var(--red);
  }
  .status-item {
    background: none;
    border: none;
    border-radius: var(--radius-sm);
    color: inherit;
    font-size: inherit;
    padding: 0 var(--space-2);
  }
  .status-item:hover {
    background: var(--surface);
    color: var(--text);
  }
  .status-end {
    align-items: center;
    display: flex;
    gap: var(--space-4);
    margin-left: auto;
  }
  /* The footer's notices, copyright and links, at the status bar's size */
  .status-end :global(:is(.archive-notice, .footer-copyright, .footer-links, .footer-link)) {
    font-size: inherit;
    margin: 0;
  }
  .status-end :global(.footer-links) {
    display: flex;
    gap: var(--space-1);
  }
  .status-end :global(.footer-link) {
    border-radius: var(--radius-sm);
    color: inherit;
    padding: 0 var(--space-2);
  }
  .status-end :global(.footer-link:hover) {
    background: var(--surface);
    color: var(--text);
  }
</style>
