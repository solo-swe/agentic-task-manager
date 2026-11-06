<script lang="ts">
  // One project on the overview (docs/multi-project-dashboard-proposal.md 3.5), from its server summary
  import { formatAgo } from '../claims';
  import type { ProjectEntry } from '../load';
  import { needsYou, statsLine } from '../projects';
  import { dashboard } from '../state.svelte';
  import { daysUntil, dueText } from '../sprints';
  import { phaseColor } from '../theme';
  import { safeUrl } from '../url';
  import Icon from '../ui/Icon.svelte';
  import NeedsYouMenu from '../shell/NeedsYouMenu.svelte';
  import ProjectLogo from '../ui/ProjectLogo.svelte';
  import Ring from '../ui/Ring.svelte';

  let { project }: { project: ProjectEntry } = $props();

  const s = $derived(project.summary);
  const need = $derived(needsYou(project));
  const needItems = $derived(project.missing ? [] : (s?.needs_you_tasks ?? []));
  const counted = $derived(s ? s.total - s.skipped : 0);
  // A project with sprints (SR-7.7): rings for its recent sprints, and the active sprint's line
  const sprints = $derived(s?.recent_sprints ?? null);
  const active = $derived(s?.active_sprint ?? null);
  const dueIn = $derived(active?.due ? daysUntil(active.due, dashboard.loadedAt) : undefined);
  // The whole project's count, in the rings' tooltip
  const overall = $derived(
    s ? `${s.done} / ${counted} done${s.skipped ? ` · ${s.skipped} skipped` : ''} (${s.percent}%)` : '',
  );
  // meta.description, only on hover over the name (T-209)
  const nameTitle = $derived([project.name, s?.description, project.path].filter(Boolean).join('\n'));
  // What's going on, by task name (T-207): the agents' tasks, or else what the loop takes next. The first shows as a
  // mini task card on a stack of the others (T-335: up to two cards' edges behind it, and "+N more"). A server before
  // 0.18 sends neither: the foot then counts the agents instead.
  const working = $derived(s?.working ?? []);
  const nextUp = $derived(s?.next ?? []);
  const byName = $derived(s?.working !== undefined);
  const queue = $derived(working.length ? working : nextUp);
  const top = $derived(queue[0]);
  // What else is in progress or ready, from the board's counts (the active sprint's, else every task's): the stack
  // under the card and "+N more". A server without boards: the tasks it listed.
  const board = $derived(s ? (s.board_active_sprint ?? s.board ?? null) : null);
  const others = $derived(
    Math.max(0, board ? board.active.count + board.todo.count - 1 : queue.length - 1, queue.length - 1),
  );
  const stack = $derived(Math.min(3, others));
  // The cards under it, deepest first (painted first): each tilted by an angle from its task's ID, so the stack looks
  // tossed but stays still from one refresh to the next
  const tiltOf = (key: string) => {
    let h = 0;
    for (const c of key) h = (h * 31 + c.charCodeAt(0)) | 0;
    return ((Math.abs(h) % 7) - 3) * 1.1;
  };
  const layers = $derived(
    Array.from({ length: stack }, (_, i) => {
      const depth = stack - i;
      const t = queue[depth];
      return { depth, tilt: tiltOf(t ? ('task' in t ? t.task : t.id) : String(depth)) || (depth % 2 ? 1.5 : -1.5) };
    }),
  );
  // The git line and the task finished last (T-208; absent from servers before 0.18)
  const git = $derived(s?.git);
  // Only an http(s) address becomes a link (T-225)
  const prUrl = $derived(safeUrl(git?.pr?.url));
  const lastDone = $derived(s?.last_done);

  // Completions per day over the last 14 days: thin bars, like the Activity widget (narrow, so the last finished
  // task's name has room beside it)
  const W = 84;
  const H = 20;
  const days = $derived(s?.completions_14d ?? []);
  const max = $derived(Math.max(1, ...days));
  const step = $derived(days.length ? W / days.length : W);
</script>

<article class="project-card" class:missing={project.missing} data-id={project.id}>
  <div class="project-card-head">
    <ProjectLogo {project} size={24} />
    {#if project.missing}
      <h3 class="project-card-name" title={nameTitle}>{project.name}</h3>
    {:else}
      <h3 class="project-card-name">
        <button class="project-card-open" title={nameTitle} onclick={() => dashboard.openProject(project.id)}
          >{project.name}</button
        >
      </h3>
    {/if}
    {#if s && !project.missing}{@render spark()}{/if}
  </div>

  <!-- What needs you: a badge on the card's top right corner, half outside it (T-335) -->
  {#if needItems.length || need}<div class="project-card-badge">
      {#if needItems.length}
        <!-- The tasks, from the server's summary: one opens the project on it, several are listed under the chip -->
        <NeedsYouMenu
          items={needItems}
          onopen={(task) => dashboard.openProject(project.id, { task })}
          chipClass="project-card-need"
          align="right"
        />
      {:else if need}
        <!-- A server without the list (before 0.17): the project's Kanban, filtered to review and blocked -->
        <button
          class="attention-chip project-card-need"
          title="Open its Kanban board, filtered to the tasks in review and blocked"
          onclick={() => dashboard.openProject(project.id, { needsYou: true })}
          >{need} need{need === 1 ? 's' : ''} you</button
        >
      {/if}
    </div>{/if}

  <!-- Completions per day, at the right of the title band (T-335) -->
  {#snippet spark()}
    <svg
      class="sparkline project-card-spark"
      width={W}
      height={H}
      viewBox="0 0 {W} {H}"
      role="img"
      aria-label="Tasks completed per day, last {days.length} days: {days.join(', ')}"
    >
      {#each days as n, i (i)}
        <rect
          x={i * step + 1}
          y={n ? H - (n / max) * (H - 2) : H - 1}
          width={Math.max(1, step - 2)}
          height={n ? (n / max) * (H - 2) : 1}
          rx="1.5"
          fill={n ? 'var(--series-remaining)' : 'var(--border)'}
        />
      {/each}
    </svg>
  {/snippet}

  {#if project.missing}
    <p class="project-card-note">
      Not found: its folder or tasks.json is gone. Remove it with <code>atm projects remove {project.id}</code>.
    </p>
  {:else if !s}
    <p class="project-card-note">No numbers yet</p>
  {:else}
    {#if sprints}
      <!-- The same layout with sprints: a ring per recent sprint, then the active sprint's progress and due date -->
      <div class="project-card-progress" title={overall}>
        {#if sprints.length}
          <ul class="project-card-rings" aria-label="Recent sprints; {overall}">
            {#each sprints as sp, i (sp.id)}
              <li
                class="project-card-ring"
                class:active={sp.state === 'active'}
                data-sprint={sp.id}
                title="{sp.id} ({sp.state}): {sp.done} / {sp.total} done"
              >
                <Ring
                  ratio={sp.total ? sp.done / sp.total : 0}
                  color={phaseColor(i)}
                  size={48}
                  stroke={5}
                  ariaLabel="{sp.id}: {sp.done} of {sp.total} done"
                />
                <span class="project-card-ring-label">{sp.id}</span>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="project-card-note">No tasks in a sprint yet</p>
        {/if}
      </div>
      <p class="project-card-phase project-card-sprint" title={active?.goal ?? null}>
        {#if active}
          <span class="project-card-label">Sprint</span> <span class="project-card-phase-name">{active.id}</span><span
            class="project-card-phase-count"
            >&nbsp;— {active.done} / {active.total}{#if dueIn !== undefined}&nbsp;<span
                class="muted"
                class:overdue={dueIn < 0}>· {dueText(dueIn)}</span
              >{/if}</span
          >
        {:else}
          <span class="muted">No active sprint</span>
        {/if}
      </p>
    {:else}
      <!-- A project without sprints: one ring for all its tasks, in the same rows, so the cards of a row line up -->
      <div class="project-card-progress" title={overall}>
        {#if counted}
          <ul class="project-card-rings" aria-label="All tasks; {overall}">
            <li class="project-card-ring" title="All tasks: {s.done} / {counted} done">
              <Ring
                ratio={s.done / counted}
                color={phaseColor(0)}
                size={48}
                stroke={5}
                ariaLabel="All tasks: {s.done} of {counted} done"
              />
              <span class="project-card-ring-label">All tasks</span>
            </li>
          </ul>
        {:else}
          <p class="project-card-note">No tasks yet</p>
        {/if}
      </div>
      <p class="project-card-phase"><span class="muted">No sprints yet</span></p>
    {/if}

    {#if s.stats}
      <!-- One line of stats (T-255), archived tasks included; the Project tab has the rest -->
      <p class="project-card-stats">{statsLine(s.stats, dashboard.loadedAt)}</p>
    {/if}

    {#if byName}
      <!-- The task at work (or next up) as a mini task card, on a stack of the others (T-335) -->
      <ul
        class="project-card-work"
        class:next={!working.length}
        data-stack={stack}
        aria-label={working.length ? 'Working now' : 'Next up'}
      >
        {#each layers as layer (layer.depth)}
          <li
            class="project-card-task-layer"
            aria-hidden="true"
            style:--depth={layer.depth}
            style:--tilt="{layer.tilt}deg"
          ></li>
        {/each}
        {#if top}
          {@const id = 'task' in top ? top.task : top.id}
          {@const who = 'agent' in top ? [top.agent, top.branch].filter(Boolean).join(' · ') : 'next up'}
          <li class="project-card-work-line project-card-task" title="{id} {top.name} — {who}">
            <div class="project-card-task-top">
              {#if working.length}
                <span class="project-card-label">Now</span>
                <Icon name="bot" size={13} />
              {:else}
                <span class="project-card-label project-card-next">Next</span>
              {/if}
              <span class="project-card-work-id">{id}</span>
              {#if 'agent' in top}<span class="project-card-task-who">{top.agent}</span>{/if}
              {#if others}<span class="project-card-more">+{others} more</span>{/if}
            </div>
            <div class="project-card-work-name">{top.name}</div>
          </li>
        {/if}
      </ul>
    {/if}

    {#if git}
      <!-- The branch and the latest PR recorded on the tasks (T-208); no CI state. The row keeps its place when there's
           neither, so the cards of a row line up (T-209) -->
      <p class="project-card-git">
        {#if git.branch || git.pr}<Icon name="git-branch" size={13} />{/if}
        {#if git.branch}<span class="project-card-branch" title="Branch {git.branch}">{git.branch}</span>{/if}
        {#if git.pr}
          {#if git.branch}<span class="muted">·</span>{/if}
          {#if prUrl}
            <a
              class="project-card-pr"
              href={prUrl}
              target="_blank"
              rel="noopener noreferrer"
              title="Pull request #{git.pr.number}, {git.pr.state}">PR #{git.pr.number}</a
            >
          {:else}
            <span class="project-card-pr">PR #{git.pr.number}</span>
          {/if}
          <span class="project-card-pr-state {git.pr.state}">{git.pr.state}</span>
        {/if}
      </p>
    {/if}

    <div class="project-card-foot">
      {#if s.agents_working && !byName}
        <span class="project-card-agents"
          ><Icon name="bot" size={13} /> {s.agents_working} agent{s.agents_working === 1 ? '' : 's'} working</span
        >
      {/if}
      {#if lastDone}
        <!-- The task finished last, and when (T-208): replaces the bare "12 m ago" -->
        <span
          class="project-card-last"
          title="Last finished: {lastDone.id} {lastDone.name}, {new Date(lastDone.completed_at).toLocaleString()}"
        >
          <!-- When, in the quiet label style (T-335) -->
          <span class="project-card-label project-card-last-when"
            >{formatAgo(lastDone.completed_at, dashboard.loadedAt)}</span
          >
          <Icon name="check" size={13} />
          <span class="project-card-work-id">{lastDone.id}</span>
          <span class="project-card-last-name">{lastDone.name}</span>
        </span>
      {:else}
        <span class="project-card-last muted">
          {s.last_activity ? formatAgo(s.last_activity, dashboard.loadedAt) : 'No activity yet'}
        </span>
      {/if}
    </div>
  {/if}
</article>
