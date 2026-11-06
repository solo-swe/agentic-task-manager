<script lang="ts">
  import { dashboard } from '../state.svelte';
  import type { Task } from '../types';
  import Icon from './Icon.svelte';

  // The branch a task is shown from, when a linked git worktree has it further along than the main one (T-179);
  // nothing otherwise
  let { task }: { task: Task } = $props();
  const branch = $derived(dashboard.branches.get(task.id));
</script>

{#if branch}
  <span class="branch-chip" data-branch={branch} title="As on branch {branch} (a git worktree of this project)"
    ><Icon name="git-branch" size={11} />{branch}</span
  >
{/if}
