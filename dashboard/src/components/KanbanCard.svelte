<script lang="ts">
  import { claimSummary } from '../lib/claims';
  import type { KanbanCard } from '../lib/kanban';
  import { dashboard } from '../lib/state.svelte';
  import { phaseColor, typeStyle } from '../lib/theme';
  import { markdownPlainText } from '../lib/markdown';
  import { REVIEW_STATE_LABELS } from '../lib/review';
  import { kindOf, priorityOf } from '../lib/vocab';
  import { blockSummary, blockWait, reviewSummary, reviewWait } from '../lib/waiting';
  import BranchChip from './BranchChip.svelte';
  import ComponentChips from './ComponentChips.svelte';
  import LabelChips from './LabelChips.svelte';
  import PriorityBadge from './PriorityBadge.svelte';
  import SprintMarker from './SprintMarker.svelte';
  import StageChip from './StageChip.svelte';

  // `showSprint`: the board shows tasks of more than one sprint, so the card names its own (SR-7.3)
  let { card, showSprint = false }: { card: KanbanCard; showSprint?: boolean } = $props();
  const task = $derived(card.task);
  const sprint = $derived(dashboard.sprints.sprintOf(task));
  // A sprint keeps its colour: by its place in the project's sprints; the backlog is muted
  const sprintIndex = $derived(dashboard.sprints.sprints.findIndex((s) => s.id === sprint));
  const pc = $derived(sprintIndex < 0 ? 'var(--text-muted)' : phaseColor(sprintIndex));
  const ts = $derived(typeStyle(task.type));
  // Open in a closed sprint (reopened after it closed, T-323): the badge says so
  const stranded = $derived(dashboard.leftovers.includes(task));
  const kind = $derived(kindOf(dashboard.vocab, task.status));
  const claim = $derived(kind === 'active' ? dashboard.claims.get(task.id) : undefined);
  const review = $derived(reviewWait(task, dashboard.loadedAt, dashboard.vocab));
  const block = $derived(blockWait(task, dashboard.loadedAt, dashboard.vocab));

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      dashboard.showDetail(task.id);
    }
  }
</script>

<div
  class="kanban-card status-{task.status} kind-{kind ?? 'none'}"
  class:next-up={card.nextUp}
  data-id={task.id}
  data-type={task.type}
  data-priority={priorityOf(dashboard.vocab, task)}
  role="button"
  tabindex="0"
  onclick={() => dashboard.showDetail(task.id)}
  onkeydown={onKeydown}
>
  <div class="kanban-card-top">
    <span class="kanban-card-id">{task.id}</span>
    {#if showSprint}
      <span
        class="badge kanban-card-sprint"
        data-sprint={sprint ?? 'backlog'}
        class:leftover={stranded}
        title={stranded ? `${sprint} is closed: no loop takes this task until it is moved` : undefined}
        style="background:color-mix(in srgb, {pc} 12.5%, transparent);color:{pc}"
        >{#if stranded}<SprintMarker state="closed" />{/if}{sprint ?? 'Backlog'}</span
      >
    {/if}
    <span class="badge" style="background:{ts.bg};color:{ts.color}">{task.type}</span>
    <PriorityBadge {task} />
    <StageChip {task} />
    {#if kind === 'review' && task.review_state && task.review_state !== 'waiting'}
      <span class="badge review-state-chip" data-review-state={task.review_state}
        >{REVIEW_STATE_LABELS[task.review_state] ?? task.review_state}</span
      >
    {/if}
    {#if task.with === 'user'}<span class="badge with-user-chip" title="Done together with you">with you</span>{/if}
    <ComponentChips {task} />
    <BranchChip {task} />
    <LabelChips {task} compact />
    {#if card.nextUp}<span class="kanban-next-up">Next up</span>{/if}
  </div>
  <div class="kanban-card-name">{task.name}</div>
  {#if claim}
    <div class="kanban-claim" title="Claimed by {claim.agent}">{claimSummary(claim, dashboard.loadedAt)}</div>
  {/if}
  {#if review}
    <div class="kanban-waiting review-wait" title={review.note && markdownPlainText(review.note.text)}>
      {reviewSummary(review)}
    </div>
  {/if}
  {#if block}
    <div
      class="kanban-waiting block-wait"
      class:on-user={block.on === 'user'}
      title={block.questions.map(markdownPlainText).join('\n')}
    >
      {blockSummary(block)}
    </div>
  {/if}
  {#if card.readiness}
    {#if card.readiness.ready}
      <div class="kanban-readiness ready">Ready</div>
    {:else}
      <div class="kanban-readiness waiting">Waiting on {card.readiness.waitingOn.join(', ')}</div>
    {/if}
  {/if}
</div>
