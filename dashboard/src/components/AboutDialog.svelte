<script lang="ts">
  import { homepage as compmdHomepage, license as compmdLicense, version as compmdVersion } from 'virtual:atm-compmd';
  import { pluginInfo } from '../lib/plugin-info';
  import { dashboard } from '../lib/state.svelte';
  import Dialog from './Dialog.svelte';
  import Logo from './Logo.svelte';
  import ProjectLogo from './ProjectLogo.svelte';

  // The open project: its name as the title bar shows it, and its logo from api/projects. Only on a project's page:
  // elsewhere (the overview) dashboard.vocab and .meta still hold the last project that was open, stale here.
  const project = $derived.by(() => {
    const route = dashboard.route;
    if (route.page !== 'project') return undefined;
    const listed = dashboard.projects.find((p) => p.id === route.id);
    return { id: route.id, name: dashboard.projectName, logo: listed?.logo };
  });
  // The project's vocabularies, short, so a user can see what the agents will use (D-1.4); the rest (priorities,
  // workflow, people, stats) is on the Project tab (T-255)
  const vocab = $derived(dashboard.vocab);
  const phases = $derived([...vocab.phaseNames].sort(([a], [b]) => a - b));

  function openProjectTab() {
    dashboard.closeDialog();
    dashboard.selectTab('project');
  }
</script>

<Dialog id="about" title="About" open={dashboard.dialog === 'about'} onclose={() => dashboard.closeDialog()}>
  <p class="about-name"><Logo size={28} id="about-logo" />{pluginInfo.fullName} ({pluginInfo.name})</p>
  <p class="about-description">{pluginInfo.description}</p>
  <dl class="about-facts">
    <dt>Version</dt>
    <dd>{pluginInfo.version}</dd>
    <dt>Author</dt>
    <dd>{pluginInfo.author}</dd>
    <dt>Licence</dt>
    <dd>{pluginInfo.license}</dd>
    <dt>Source</dt>
    <dd><a href={pluginInfo.repository} target="_blank" rel="noopener noreferrer">{pluginInfo.repository}</a></dd>
    <!-- What renders task text, Help and What's new, shipped with the plugin -->
    <dt>Rendering</dt>
    <dd id="about-compmd">
      <a href={compmdHomepage} target="_blank" rel="noopener noreferrer">CompMD</a>
      {compmdVersion} ({compmdLicense})
    </dd>
  </dl>
  {#if project}
    <h3 class="about-section-title" id="about-project">
      <ProjectLogo {project} size={20} /><span>{project.name}</span>
    </h3>
    <dl class="about-facts" id="about-vocab">
      <dt>Statuses</dt>
      <dd>{vocab.statuses.map((s) => `${s.label} (${s.kind})`).join(' · ')}</dd>
      <dt>Phases</dt>
      <dd>{phases.length ? phases.map(([id, name]) => `${id} ${name}`).join(' · ') : 'Numbered, without names'}</dd>
      <dt>Types</dt>
      <dd>{vocab.types.map((t) => t.label).join(' · ')}</dd>
    </dl>
    <p class="about-project-details">
      <button id="about-project-details" onclick={openProjectTab}>Project details</button>: priorities, workflow,
      people, agents and stats, on the Project tab
    </p>
  {/if}
</Dialog>
