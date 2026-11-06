<script lang="ts">
  import { pullRef } from '../repo';
  import { prOf, stageOf } from '../stages';
  import { dashboard } from '../state.svelte';
  import type { Task } from '../types';
  import RepoRef from './RepoRef.svelte';

  // A done task's stage when it has one (D-4.2): "PR #12" or "Merged", linking to the pull request (lib/repo.ts);
  // nothing otherwise
  let { task }: { task: Task } = $props();
  const stage = $derived(stageOf(task, dashboard.vocab));
  const pr = $derived(prOf(task));
  const merged = $derived(
    `Merged${pr ? ` in #${pr.number}` : ''}${task.merged_at ? ` on ${new Date(task.merged_at).toLocaleString()}` : ''}`,
  );
</script>

{#if stage === 'in_pr' && pr}
  <RepoRef ref={pullRef(dashboard.meta, pr)} className="stage-chip in-pr" title="In pull request #{pr.number}"
    ><span data-stage="in_pr">PR #{pr.number}</span></RepoRef
  >
{:else if stage === 'merged'}
  {#if pr}
    <RepoRef ref={pullRef(dashboard.meta, pr)} className="stage-chip merged" title={merged}
      ><span data-stage="merged">Merged</span></RepoRef
    >
  {:else}
    <span class="stage-chip merged" data-stage="merged" title={merged}>Merged</span>
  {/if}
{/if}
