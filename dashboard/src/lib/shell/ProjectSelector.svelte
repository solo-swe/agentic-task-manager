<script lang="ts">
  // The project selector next to the title (docs/multi-project-dashboard-proposal.md 3.4): Projects Overview, then each
  // registered project with its progress and a dot when something needs you. A filter field with a listbox (the
  // WAI-ARIA combobox pattern): typing filters, the arrows, Home and End move, Enter opens, Escape closes.
  import { tick } from 'svelte';
  import type { ProjectEntry } from '../load';
  import { dashboard } from '../state.svelte';
  import Icon from '../ui/Icon.svelte';
  import ProjectLogo from '../ui/ProjectLogo.svelte';

  const OVERVIEW = '';

  let open = $state(false);
  let query = $state('');
  let active = $state(OVERVIEW);
  let root: HTMLElement;
  let button: HTMLButtonElement;
  let filter = $state<HTMLInputElement>();

  const current = $derived(dashboard.route.page === 'project' ? dashboard.route.id : OVERVIEW);
  const needsYou = (p: ProjectEntry) =>
    p.summary ? p.summary.needs_you.review + p.summary.needs_you.blocked_on_user : 0;
  /** What needs the user in the other projects: the badge on the button */
  const othersNeed = $derived(
    dashboard.projects.filter((p) => p.id !== current && !p.missing).reduce((n, p) => n + needsYou(p), 0),
  );
  const shown = $derived(dashboard.projects.filter((p) => p.name.toLowerCase().includes(query.trim().toLowerCase())));
  /** The options in order: Projects Overview (unless filtered out), then the projects that match and are there */
  const options = $derived([
    ...('projects overview'.includes(query.trim().toLowerCase()) ? [OVERVIEW] : []),
    ...shown.filter((p) => !p.missing).map((p) => p.id),
  ]);
  const optionId = (id: string) => (id === OVERVIEW ? 'project-option-overview' : `project-option-${id}`);

  async function show() {
    open = true;
    query = '';
    active = current;
    await tick();
    filter?.focus();
  }

  function close(refocus = true) {
    open = false;
    if (refocus) button.focus();
  }

  function choose(id: string) {
    close();
    if (id !== current) dashboard.openProject(id === OVERVIEW ? null : id);
  }

  function onKey(e: KeyboardEvent) {
    const i = options.indexOf(active);
    const move = (to: number) => {
      e.preventDefault();
      if (options.length) active = options[Math.max(0, Math.min(options.length - 1, to))];
    };
    if (e.key === 'ArrowDown') move(i + 1);
    else if (e.key === 'ArrowUp') move(i - 1);
    else if (e.key === 'Home') move(0);
    else if (e.key === 'End') move(options.length - 1);
    else if (e.key === 'Enter' && options.includes(active)) {
      e.preventDefault();
      choose(active);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close();
    }
  }

  function onInput() {
    // Keep a visible option active while the list narrows
    if (!options.includes(active)) active = options[0] ?? OVERVIEW;
  }

  function onWindowClick(e: MouseEvent) {
    if (open && !root.contains(e.target as Node)) close(false);
  }
</script>

<svelte:window onclick={onWindowClick} />

<div class="menu-anchor project-selector" bind:this={root}>
  <button
    class="project-switch"
    id="project-selector"
    bind:this={button}
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-label={othersNeed
      ? `Switch project (${othersNeed} task${othersNeed === 1 ? '' : 's'} in other projects need you)`
      : 'Switch project'}
    title="Switch project"
    onclick={() => (open ? close() : show())}
  >
    <Icon name="chevron-down" size={16} />
    {#if othersNeed}<span class="project-switch-badge" id="other-projects-need">{othersNeed}</span>{/if}
  </button>
  {#if open}
    <div class="menu project-menu" id="project-menu">
      <input
        class="project-filter"
        id="project-filter"
        type="text"
        placeholder="Filter projects"
        autocomplete="off"
        role="combobox"
        aria-label="Filter projects"
        aria-expanded="true"
        aria-controls="project-options"
        aria-activedescendant={options.includes(active) ? optionId(active) : undefined}
        bind:this={filter}
        bind:value={query}
        oninput={onInput}
        onkeydown={onKey}
      />
      <ul class="project-options" id="project-options" role="listbox" aria-label="Projects">
        {#if options.includes(OVERVIEW)}
          <li
            class="project-option"
            id={optionId(OVERVIEW)}
            role="option"
            aria-selected={current === OVERVIEW}
            class:active={active === OVERVIEW}
            data-id="overview"
            onclick={() => choose(OVERVIEW)}
            onkeydown={() => {}}
            onmouseenter={() => (active = OVERVIEW)}
          >
            <!-- Where each project shows its logo (T-348) -->
            <span class="project-option-icon"><Icon name="layout-grid" size={16} /></span>
            <span class="project-option-name">Projects Overview</span>
          </li>
        {/if}
        {#each shown as project (project.id)}
          {@const percent = project.summary?.percent ?? 0}
          <li
            class="project-option"
            class:missing={project.missing}
            class:active={active === project.id}
            id={optionId(project.id)}
            role="option"
            aria-selected={current === project.id}
            aria-disabled={project.missing}
            data-id={project.id}
            title={project.path}
            onclick={() => !project.missing && choose(project.id)}
            onkeydown={() => {}}
            onmouseenter={() => !project.missing && (active = project.id)}
          >
            <ProjectLogo {project} size={16} />
            <span class="project-option-name">{project.name}</span>
            {#if project.missing}
              <span class="project-option-note">not found</span>
            {:else}
              {#if needsYou(project)}
                <span class="project-option-dot" title="{needsYou(project)} need you" aria-label="needs you"></span>
              {/if}
              <span class="project-option-bar" aria-label="{percent}% done"
                ><span class="project-option-fill" style:width="{percent}%"></span></span
              >
            {/if}
          </li>
        {/each}
        {#if !options.length && !shown.length}
          <li class="project-option-empty">No project matches</li>
        {/if}
      </ul>
    </div>
  {/if}
</div>
