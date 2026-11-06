<script lang="ts">
  // The pages without a project: the overview of all projects, how to add a project when none is registered, and a
  // project the address names but the server doesn't list (lib/routes.ts). Their title bar is Header's.
  import { dashboard } from '../state.svelte';
  import ProjectsOverview from '../overview/ProjectsOverview.svelte';

  const present = $derived(dashboard.projects.filter((p) => !p.missing));
</script>

<div class="landing" id="landing" data-page={dashboard.route.page}>
  {#if dashboard.route.page === 'unknown'}
    <section class="landing-note" id="unknown-project" role="alert">
      <h2>No project “{dashboard.route.id}”</h2>
      <p>
        The atm server doesn't list it, or its folder or tasks.json is gone. <a href="/projects">See all projects</a>.
      </p>
    </section>
  {:else if dashboard.route.page === 'none' || (dashboard.projectsLoaded && present.length === 0)}
    <section class="landing-note" id="no-projects">
      <h2>No projects yet</h2>
      <p>The dashboard shows every project atm works in. To add one:</p>
      <ul>
        <li>run <code>/atm:create-tasks</code> in a project, or</li>
        <li>run <code>atm projects add &lt;path&gt;</code> for a project that already has tasks.</li>
      </ul>
    </section>
  {:else}
    <ProjectsOverview />
  {/if}
</div>
