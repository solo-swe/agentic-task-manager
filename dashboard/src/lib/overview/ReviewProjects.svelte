<script lang="ts">
  // All reviews' project filter (ATM-757, docs/sidebar-overhaul-design.md §2.5): All projects, or one or more of
  // them. The page keeps the choice in its address (`?projects=`); this shows it and says what is chosen
  import { DropdownMenu } from 'bits-ui';
  import Icon from '../ui/Icon.svelte';

  let {
    projects,
    chosen,
    onchoose,
  }: {
    /** The projects it can narrow to, in the workspace's order */
    projects: readonly { id: string; name: string }[];
    /** Those chosen; none is every project */
    chosen: readonly string[];
    /** Another choice: the projects chosen, in the projects' order (none: every project) */
    onchoose: (ids: string[]) => void;
  } = $props();

  let open = $state(false);
  const picked = $derived(new Set(chosen));
  const label = $derived(
    chosen.length === 0
      ? 'All projects'
      : chosen.length === 1
        ? (projects.find((p) => p.id === chosen[0])?.name ?? chosen[0])
        : `${chosen.length} projects`,
  );

  /** `id` added to the choice, or taken out, in the projects' order */
  function toggle(id: string) {
    const chosenNext = (p: { id: string }) => (p.id === id ? !picked.has(id) : picked.has(p.id));
    onchoose(projects.filter(chosenNext).map((p) => p.id));
  }
</script>

<span class="review-projects-anchor">
  <DropdownMenu.Root bind:open>
    <DropdownMenu.Trigger>
      {#snippet child({ props })}
        <button
          {...props}
          class="review-projects"
          id="review-projects"
          aria-label="Projects: {label}; choose others"
          data-chosen={chosen.join(',')}
          ><Icon name="folder" size={14} /><span class="review-projects-label">{label}</span><Icon
            name="chevron-down"
            size={14}
          /></button
        >
      {/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.ContentStatic preventScroll={false} aria-label="Projects">
      {#snippet child({ props })}
        <span {...props} class="menu review-projects-menu" id="review-projects-menu">
          <DropdownMenu.Item onSelect={() => onchoose([])}>
            {#snippet child({ props })}
              <button {...props} class="menu-item" data-project="" aria-current={chosen.length ? undefined : 'true'}
                ><span class="review-projects-check"
                  >{#if !chosen.length}<Icon name="check" size={14} />{/if}</span
                >All projects</button
              >
            {/snippet}
          </DropdownMenu.Item>
          {#each projects as project (project.id)}
            <DropdownMenu.CheckboxItem
              checked={picked.has(project.id)}
              closeOnSelect={false}
              onCheckedChange={() => toggle(project.id)}
            >
              {#snippet child({ props, checked })}
                <button {...props} class="menu-item" data-project={project.id}
                  ><span class="review-projects-check"
                    >{#if checked}<Icon name="check" size={14} />{/if}</span
                  >{project.name}</button
                >
              {/snippet}
            </DropdownMenu.CheckboxItem>
          {/each}
        </span>
      {/snippet}
    </DropdownMenu.ContentStatic>
  </DropdownMenu.Root>
</span>

<style>
  .review-projects-anchor {
    position: relative;
    display: inline-flex;
  }
  .review-projects {
    display: inline-flex;
    align-items: center;
    gap: var(--space-3);
    height: 32px;
    padding: 0 var(--space-5);
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: var(--text-sm);
    cursor: pointer;
    white-space: nowrap;
  }
  .review-projects:hover {
    border-color: var(--accent);
  }
  .review-projects-label {
    max-width: 14rem;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .review-projects-menu {
    gap: var(--space-1);
  }
  .review-projects-menu :global(.menu-item) {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    width: 100%;
    padding: var(--space-3) var(--space-4);
    border: none;
    border-radius: var(--radius-md);
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--text-sm);
    text-align: left;
    cursor: pointer;
  }
  .review-projects-menu :global(.menu-item:hover),
  .review-projects-menu :global(.menu-item[data-highlighted]) {
    background: var(--surface2);
  }
  .review-projects-check {
    display: inline-flex;
    width: 14px;
  }
</style>
