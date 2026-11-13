<script lang="ts">
  // The project selector next to the title (docs/multi-project-dashboard-proposal.md 3.4): Projects Overview, then each
  // registered project with its progress and a dot when something needs you. A filter field with a listbox (the
  // WAI-ARIA combobox pattern): typing filters, the arrows, Home and End move, Enter opens. It opens in Bits UI's
  // popover (T-431): focus moves to the field, Escape or a click outside closes it, and focus goes back to the button.
  import { Popover } from 'bits-ui';
  import type { ProjectEntry } from '../load';
  import { dashboard } from '../state.svelte';
  import Icon from '../ui/Icon.svelte';
  import ProjectLogo from '../ui/ProjectLogo.svelte';

  const OVERVIEW = '';

  /** `badge`: the other projects' needs-you count on the button (the browser's title bar); the app's tab label
   * shows none (T-513) */
  let { badge = true }: { badge?: boolean } = $props();

  let open = $state(false);
  let query = $state('');
  let active = $state(OVERVIEW);

  const current = $derived(dashboard.route.page === 'project' ? dashboard.route.id : OVERVIEW);
  const needsYou = (p: ProjectEntry) =>
    p.summary ? p.summary.needs_you.review + p.summary.needs_you.blocked_on_user : 0;
  /** What needs the user in the other projects: the badge on the button */
  const othersNeed = $derived(
    badge ? dashboard.projects.filter((p) => p.id !== current && !p.missing).reduce((n, p) => n + needsYou(p), 0) : 0,
  );
  const shown = $derived(dashboard.projects.filter((p) => p.name.toLowerCase().includes(query.trim().toLowerCase())));
  /** The options in order: Projects Overview (unless filtered out), then the projects that match and are there */
  const options = $derived([
    ...('projects overview'.includes(query.trim().toLowerCase()) ? [OVERVIEW] : []),
    ...shown.filter((p) => !p.missing).map((p) => p.id),
  ]);
  const optionId = (id: string) => (id === OVERVIEW ? 'project-option-overview' : `project-option-${id}`);

  /** Opening: the whole list again, the current page's entry active */
  function onOpenChange(next: boolean) {
    if (!next) return;
    query = '';
    active = current;
  }

  let filter = $state<HTMLInputElement>();

  function choose(id: string) {
    open = false;
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
    }
  }

  function onInput() {
    // Keep a visible option active while the list narrows
    if (!options.includes(active)) active = options[0] ?? OVERVIEW;
  }
</script>

<div class="menu-anchor project-selector">
  <!-- Open, typing goes to the filter field -->
  <Popover.Root bind:open {onOpenChange} onOpenChangeComplete={(next) => next && filter?.focus()}>
    <Popover.Trigger>
      {#snippet child({ props })}
        <button
          {...props}
          class="project-switch"
          id="project-selector"
          aria-label={othersNeed
            ? `Switch project (${othersNeed} task${othersNeed === 1 ? '' : 's'} in other projects need you)`
            : 'Switch project'}
          title="Switch project"
        >
          <Icon name="chevron-down" size={16} />
          {#if othersNeed}<span class="project-switch-badge" id="other-projects-need">{othersNeed}</span>{/if}
        </button>
      {/snippet}
    </Popover.Trigger>
    <Popover.ContentStatic role="dialog" aria-label="Projects">
      {#snippet child({ props })}
        <div {...props} class="menu project-menu" id="project-menu">
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
      {/snippet}
    </Popover.ContentStatic>
  </Popover.Root>
</div>
