<script lang="ts">
  import { claimSummary } from '../claims';
  import { followInPlace } from '../new-tab-clicks';
  import ReviewLink from '../details/ReviewLink.svelte';
  import type { KanbanCard } from '../kanban';
  import { dashboard } from '../state.svelte';
  import { phaseColor, typeStyle } from '../theme';
  import { markdownPlainText } from '../plain-text';
  import { REVIEW_STATE_LABELS } from '../review';
  import { priorityOf, taskKind } from '../vocab';
  import { waitLine } from '../waiting';
  import BranchChip from '../ui/BranchChip.svelte';
  import ComponentChips from '../ui/ComponentChips.svelte';
  import LabelChips from '../ui/LabelChips.svelte';
  import PriorityBadge from '../ui/PriorityBadge.svelte';
  import SprintMarker from '../progress/SprintMarker.svelte';
  import StageChip from '../ui/StageChip.svelte';
  import TaskKey from '../ui/TaskKey.svelte';

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
  const kind = $derived(taskKind(task));
  const claim = $derived(kind === 'active' ? dashboard.claims.get(task.id) : undefined);
  // Who it waits on, as atm says it (ATM-717): one line, agreeing with its review's chip
  const wait = $derived(waitLine(task, dashboard.loadedAt));
  // The tasks it waits for, by their keys (ADR-007)
  const waitingOn = $derived(
    card.readiness && !card.readiness.ready
      ? card.readiness.waitingOn.map((id) => dashboard.refOf(id).label).join(', ')
      : '',
  );

  // Its code review, while it is in review (ATM-619): a link on the card that opens the review, not the task
  const codeReview = $derived(kind === 'review' ? (task.derived?.review ?? null) : null);
  const href = $derived(dashboard.taskAddress(task));
</script>

<div
  class="kanban-card status-{task.status} kind-{kind ?? 'none'}"
  class:next-up={card.nextUp}
  data-id={task.id}
  data-type={task.type}
  data-priority={priorityOf(dashboard.vocab, task)}
>
  <div class="kanban-card-top">
    <span class="kanban-card-id"><TaskKey {task} /></span>
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
  <!-- A real link to its details (ATM-698): Copy Link, ⌘-click and a middle click work as on any link; a plain
       click opens them here. It covers the card (`.card-link`), under the card's own links -->
  <a class="kanban-card-name card-link" {href} onclick={(e) => followInPlace(e, () => dashboard.showDetail(task.id))}
    >{task.name}</a
  >
  {#if claim}
    <div class="kanban-claim" title="Claimed by {claim.agent}">{claimSummary(claim, dashboard.loadedAt)}</div>
  {/if}
  {#if wait}
    <div
      class="kanban-waiting {wait.kind}-wait"
      class:on-user={wait.onUser}
      title={wait.note ? markdownPlainText(wait.note.text) : wait.questions.map(markdownPlainText).join('\n')}
    >
      {wait.text}
    </div>
  {/if}
  {#if codeReview && dashboard.projectId}
    <div class="card-review">
      <ReviewLink project={dashboard.projectId} reference={codeReview.ref} status={codeReview.status} prominent />
    </div>
  {/if}
  {#if card.readiness}
    {#if card.readiness.ready}
      <div class="kanban-readiness ready">Ready</div>
    {:else}
      <div class="kanban-readiness waiting">Waiting on {waitingOn}</div>
    {/if}
  {/if}
</div>
