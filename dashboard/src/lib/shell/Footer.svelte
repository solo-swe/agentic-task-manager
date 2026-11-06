<script lang="ts">
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
</script>

<footer class="site-footer" id="site-footer">
  <span class="footer-start">
    <span class="footer-brand" id="footer-brand"
      ><Logo size={16} id="footer-logo" />{pluginInfo.fullName} ({pluginInfo.name})</span
    >
    <span class="footer-copyright">{copyright(pluginInfo)}</span>
  </span>
  {#if notice}<span class="archive-notice" id="archive-notice">{notice}</span>{/if}
  {#if problems}<span
      class="archive-notice"
      id="problems-notice"
      title={[...dashboard.fileProblems, 'Run `atm check` for the list.'].join('\n')}
      >atm check finds {problems} problem{problems === 1 ? '' : 's'} in tasks.json</span
    >{/if}
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
</footer>
