<script lang="ts">
  import { claimSummary } from '../lib/claims';
  import { scrollToTask } from '../lib/scroll';
  import { dashboard } from '../lib/state.svelte';
  import { kindStyle, phaseColor, typeStyle } from '../lib/theme';
  import type { Task } from '../lib/types';
  import { cardFacts, cardNote } from '../lib/cardfacts';
  import { markdownPlainText } from '../lib/plain-text';
  import { noteKindLabel } from '../lib/notes';
  import { kindOf, statusLabel, typeLabel } from '../lib/vocab';
  import { blockSummary, blockWait, reviewSummary, reviewWait } from '../lib/waiting';
  import BranchChip from './BranchChip.svelte';
  import ComponentChips from './ComponentChips.svelte';
  import LabelChips from './LabelChips.svelte';
  import PriorityBadge from './PriorityBadge.svelte';
  import SprintMarker from './SprintMarker.svelte';
  import StageChip from './StageChip.svelte';

  // `showSprint`: the list shows tasks of more than one sprint and doesn't group by them, so the card names its own
  let { task, showSprint = false }: { task: Task; showSprint?: boolean } = $props();

  const sprint = $derived(dashboard.sprints.sprintOf(task));
  // A sprint keeps its colour: by its place in the project's sprints; the backlog is muted (as on the Kanban)
  const sprintIndex = $derived(dashboard.sprints.sprints.findIndex((s) => s.id === sprint));
  const pc = $derived(sprintIndex < 0 ? 'var(--text-muted)' : phaseColor(sprintIndex));
  const ts = $derived(typeStyle(task.type));
  // Open in a closed sprint (reopened after it closed, T-323): the badge says so
  const stranded = $derived(dashboard.leftovers.includes(task));
  const kind = $derived(kindOf(dashboard.vocab, task.status));
  const ss = $derived(kindStyle(kind));
  const deps = $derived(task.dependencies ?? []);
  const claim = $derived(kind === 'active' ? dashboard.claims.get(task.id) : undefined);
  const review = $derived(reviewWait(task, dashboard.loadedAt, dashboard.vocab));
  const block = $derived(blockWait(task, dashboard.loadedAt, dashboard.vocab));
  // The row's width (T-338): the description's start and the latest note on the left, who and when on the right
  const excerpt = $derived(task.description ? markdownPlainText(task.description) : '');
  const note = $derived(cardNote(task));
  const facts = $derived(cardFacts(task, dashboard.loadedAt, dashboard.vocab));
  const ROLE = { created: 'Created by', worked: 'Worked on by', finished: 'Finished by' } as const;
</script>

<div class="task-card status-{task.status} kind-{kind ?? 'none'}" id="task-{task.id}" data-id={task.id}>
  <div class="task-id">{task.id}</div>
  <div class="task-body">
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="task-name" onclick={() => dashboard.showDetail(task.id)}>{task.name}</div>
    {#if excerpt}<div class="task-excerpt">{excerpt}</div>{/if}
    <div class="task-meta">
      {#if showSprint}
        <span
          class="badge task-card-sprint"
          data-sprint={sprint ?? 'backlog'}
          class:leftover={stranded}
          title={stranded ? `${sprint} is closed: no loop takes this task until it is moved` : undefined}
          style="background:color-mix(in srgb, {pc} 12.5%, transparent);color:{pc}"
          >{#if stranded}<SprintMarker state="closed" />{/if}{sprint ?? 'Backlog'}</span
        >
      {/if}
      <span class="badge" style="background:{ts.bg};color:{ts.color}" title={typeLabel(dashboard.vocab, task.type)}
        >{task.type}</span
      >
      <span class="badge" style="background:{ss.bg};color:{ss.color}">{statusLabel(dashboard.vocab, task.status)}</span>
      <PriorityBadge {task} />
      <StageChip {task} />
      <BranchChip {task} />
      <ComponentChips {task} />
      <LabelChips {task} />
      {#if deps.length}
        <span class="deps-label">needs:</span>
        {#each deps as dep (dep)}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions, a11y_missing_attribute -->
          <a class="dep-link" title={dashboard.taskMap.get(dep)?.name || dep} onclick={() => scrollToTask(dep)}>{dep}</a
          >
        {/each}
      {/if}
    </div>
    {#if claim}
      <div class="task-claim" title="Claimed by {claim.agent}">{claimSummary(claim, dashboard.loadedAt)}</div>
    {/if}
    {#if review}
      <div class="task-waiting review-wait" title={review.note && markdownPlainText(review.note.text)}>
        {reviewSummary(review)}
      </div>
    {/if}
    {#if block}
      <div
        class="task-waiting block-wait"
        class:on-user={block.on === 'user'}
        title={block.questions.map(markdownPlainText).join('\n')}
      >
        {blockSummary(block)}
      </div>
    {/if}
    {#if note}
      <div class="task-note" data-kind={note.kind}>
        <span class="task-note-kind">{noteKindLabel(note.kind)}:</span>
        {markdownPlainText(note.text)}
      </div>
    {/if}
  </div>
  <div class="task-facts" class:empty={!facts.people.length && !facts.when && !facts.took}>
    {#each facts.people as p (p.role)}
      <div class="task-fact" data-role={p.role} title="{ROLE[p.role]} {p.who}">
        <span class="task-fact-label">{ROLE[p.role]}</span>
        <span class="task-fact-who">{p.who}</span>
      </div>
    {/each}
    {#if facts.when || facts.took}
      <div class="task-fact task-fact-time">{[facts.when, facts.took].filter(Boolean).join(' · ')}</div>
    {/if}
  </div>
  <div class="task-tags">
    {#each task.use_cases ?? [] as uc (uc)}<span class="uc-badge" title={uc}>{uc}</span
      >{/each}{#each task.requirements ?? [] as req (req)}<span class="req-badge">{req}</span>{/each}
  </div>
</div>
