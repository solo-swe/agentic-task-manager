<script lang="ts">
  // The workspace switcher next to the project selector (ADR-003 §5), only when there is more than one workspace: the
  // workspace shown, and a Bits UI menu of the others (T-431: the arrows, Home and End move, Enter opens, Escape or a
  // click outside closes, and focus goes back to the button). Choosing one opens its landing page, and the server
  // records it as the last opened.
  import { DropdownMenu } from 'bits-ui';
  import { dashboard } from '../state.svelte';
  import Icon from '../ui/Icon.svelte';

  let open = $state(false);
  const plural = (n: number) => `${n} project${n === 1 ? '' : 's'}`;
</script>

{#if dashboard.workspaces.length > 1}
  <span class="menu-anchor workspace-selector">
    <DropdownMenu.Root bind:open>
      <DropdownMenu.Trigger>
        {#snippet child({ props })}
          <button
            {...props}
            class="workspace-switch"
            id="workspace-selector"
            title="Switch workspace"
            aria-label="Workspace {dashboard.workspace}: switch workspace"
          >
            <span class="workspace-name">{dashboard.workspace}</span>
            <Icon name="chevron-down" size={14} />
          </button>
        {/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.ContentStatic preventScroll={false} aria-label="Workspaces">
        {#snippet child({ props })}
          <span {...props} class="menu workspace-menu" id="workspace-menu">
            {#each dashboard.workspaces as workspace (workspace.id)}
              <DropdownMenu.Item onSelect={() => dashboard.openWorkspace(workspace.id)}>
                {#snippet child({ props })}
                  <button
                    {...props}
                    class="workspace-option"
                    class:current={workspace.id === dashboard.workspace}
                    aria-current={workspace.id === dashboard.workspace ? 'true' : undefined}
                    data-id={workspace.id}
                  >
                    <span class="workspace-option-name">{workspace.id}</span>
                    <span class="workspace-option-count">{plural(workspace.projects)}</span>
                  </button>
                {/snippet}
              </DropdownMenu.Item>
            {/each}
          </span>
        {/snippet}
      </DropdownMenu.ContentStatic>
    </DropdownMenu.Root>
  </span>
{/if}
