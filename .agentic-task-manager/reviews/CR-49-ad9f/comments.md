## dashboard/src/lib/shell/NeedsYouMenu.svelte:118-138 · round 1 · t1
>               {#each items as task (task.id)}
>                 <DropdownMenu.Item onSelect={() => onopen(task.id)}>
>                   {#snippet child({ props })}
>                     <button {...props} class="needs-you-row" data-id={task.id}>
>                       <span class="needs-you-id"><TaskKey {task} /></span>
>                       <span class="needs-you-name">{task.name}</span>
>                       <span class="needs-you-reason">{REASONS[task.reason]}</span>
>                     </button>
>                   {/snippet}
>                 </DropdownMenu.Item>
>               {/each}
>               {#each listed as review (review.reference)}
>                 <DropdownMenu.Item onSelect={() => onreview?.(review.reference)}>
>                   {#snippet child({ props })}
>                     <button {...props} class="needs-you-row" data-review={review.reference}>
>                       <span class="needs-you-id">{review.reference}</span>
>                       <span class="needs-you-name">{review.title}</span>
>                       <span class="needs-you-reason">{REVIEW_REASONS[review.why]}</span>
>                     </button>
>                   {/snippet}
>                 </DropdownMenu.Item>

- [x] **Hakan Serce** · 2026-11-19T05:45:47Z · c1
  We need to dedupe here, for tasks in review with CRs open, we have two items.
  - **claude:0f56a916** · 2026-11-19T05:48:37Z · c3
    Agreed: a task in review whose CR waits for you is listed twice, as the task and as its review. I am fixing it in this review: the review row stays (it opens the CR, and says why it waits), the task row goes, and the count is the deduplicated one. That needs atm to say which tasks each listed review is for, so it touches the needs-you API too. A new round follows.
  - **claude:0f56a916** · 2026-11-19T05:54:34Z · c5
    Done in round 2 (f7c2adb). The chip now lists such a task once, as its review: a task in review whose CR is among the reviews waiting on you is left out of the task rows, and the count drops with it. The match is by the review ID, as atm does it, since a CR number can change at a clash. For that, atm summary needs_you_reviews entries now carry the review id (schema, atm-lib, the API doc). Tests: a unit table (left out through a waiting review; kept through another one, with none, and when blocked) and an e2e on a project page (T-004 through CR-7 plus T-005 blocked: 2 need you, rows T-005 then CR-7). The overview cards were already right: atm summary skipped such tasks.

## (the review) · round 1 · t2
- [poke] **Hakan Serce** · 2026-11-19T05:47:14Z · c2
  - **claude:0f56a916** · 2026-11-19T05:48:37Z · c4
    Where it stands: round 1 (the long list scrolls in a small popup, ▲/▼ N more, no scroll bar) is done and tested. Your dedupe comment (c1) is next: I am on it now, then a round 2 on this review. Sorry for the delay: my review watch had stopped and I only saw both just now.
