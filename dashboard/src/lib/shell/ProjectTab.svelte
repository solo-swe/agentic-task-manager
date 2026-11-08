<script lang="ts">
  import { formatMinutes } from '../format';
  import { actors, vocabCounts, workflowOf } from '../projectinfo';
  import { projectStats, timeHistogram } from '../projectstats';
  import { repositoryOf } from '../repo';
  import { dashboard } from '../state.svelte';
  import InfoTip from '../ui/InfoTip.svelte';
  import ProjectLogo from '../ui/ProjectLogo.svelte';
  import SprintMarker from '../progress/SprintMarker.svelte';
  import TaskTimeHistogram from '../progress/TaskTimeHistogram.svelte';

  // The project as a whole (docs/project-stats-proposal.md, T-252): who worked on it, how it pushes, and its
  // vocabularies, Information on the left; Stats on the right (T-253). From the history, archived tasks too, and
  // whatever the search and filters: it describes the project, not the slice on screen.
  $effect(() => {
    if (dashboard.archivedCount > 0) dashboard.wantArchive();
  });
  const tasks = $derived(dashboard.history);
  const vocab = $derived(dashboard.vocab);
  // Recomputed with every reload of the tasks
  const now = $derived.by(() => {
    void tasks;
    return Date.now();
  });

  const project = $derived({
    id: dashboard.projectId ?? '',
    name: dashboard.projectName,
    logo: dashboard.projects.find((p) => p.id === dashboard.projectId)?.logo,
  });
  const repository = $derived(repositoryOf(dashboard.meta));
  const workflow = $derived(workflowOf(dashboard.meta));
  const who = $derived(actors(tasks, now, vocab));
  const counts = $derived(vocabCounts(tasks, vocab, dashboard.sprints));
  // Each component with how many tasks (archived ones too) are in it
  const componentCounts = $derived(
    dashboard.components.map((c) => ({
      ...c,
      count: tasks.filter((t) => Array.isArray(t.components) && t.components.includes(c.id)).length,
    })),
  );
  // Closed sprints fold away, like on the Progress tab
  let showClosed = $state(false);
  const open = $derived(counts.sprints.filter((s) => s.state !== 'closed'));
  const closed = $derived(counts.sprints.filter((s) => s.state === 'closed'));
  const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;

  // Stats (T-253): archived tasks not loaded yet count from meta.archived
  const stats = $derived(
    projectStats(
      tasks,
      dashboard.meta,
      now,
      vocab,
      dashboard.archiveLoaded ? 0 : dashboard.archivedCount,
      dashboard.sprints,
    ),
  );
  const hours = (msValue: number) => (msValue ? formatMinutes(msValue / 60_000) : '0 m');
  const day = (t: number) =>
    new Date(t).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' });
  const span = (msValue: number) => {
    const days = Math.round(msValue / 86_400_000);
    return days >= 1 ? plural(days, 'day') : hours(msValue);
  };
  const numbers = $derived([
    {
      key: 'started',
      value: stats.started === null ? '–' : day(stats.started),
      label: 'started',
      title: 'When the first task was created',
    },
    {
      key: 'duration',
      value: stats.duration ? span(stats.duration.to - stats.duration.from) : '–',
      label: stats.duration?.ongoing ? 'so far' : 'duration',
      title: stats.duration
        ? `From the first task started (${day(stats.duration.from)}) to ${stats.duration.ongoing ? 'now: tasks are still open' : `the last one done (${day(stats.duration.to)})`}`
        : 'No task has been started yet',
    },
    {
      key: 'tasks',
      value: String(stats.tasks.total),
      label: 'tasks',
      title: `${stats.tasks.done} done, ${stats.tasks.open} open, ${stats.tasks.skipped} skipped (archived tasks count as done)`,
    },
    {
      key: 'sprints',
      value: String(stats.sprints.total),
      label: `sprints · ${stats.sprints.closed} closed`,
      title: 'The project’s sprints, and how many of them are closed',
    },
    {
      key: 'commits',
      value: String(stats.taskCommits),
      label: 'task commits',
      title: 'One per task: the commit atm made when finishing it. Fix-ups and other commits are not counted',
    },
    {
      key: 'prs',
      value: stats.pullRequests === null ? '–' : String(stats.pullRequests),
      label: 'pull requests',
      title:
        stats.pullRequests === null
          ? 'This project pushes straight to its branch'
          : 'Pull requests recorded on tasks (atm pr)',
    },
    {
      key: 'agent-time',
      value: hours(stats.agentMs),
      label: 'agent time',
      title: `Parallel agents add up: ${hours(stats.aloneMs)} on tasks agents did alone, ${hours(stats.withYouMs)} on interactive tasks with you`,
    },
    {
      key: 'most',
      value: String(stats.maxAgents),
      label: 'most at once',
      title: 'The most agents working at the same moment',
    },
  ]);
</script>

<div class="project-view" id="project">
  <!-- The logo and the name; no description line under it (T-267) -->
  <header class="project-head">
    <ProjectLogo {project} size={40} />
    <h2 class="project-title">
      <span id="project-name">{project.name}</span>
      <InfoTip label="the Project tab" id="project-info-tip">
        <p>
          The whole project: every task, archived ones too. The search and filters don't apply here, as they do on the
          other tabs.
        </p>
      </InfoTip>
    </h2>
  </header>

  <div class="project-columns">
    <section class="project-card" id="project-info" aria-labelledby="project-info-title">
      <h3 class="menu-label" id="project-info-title">Information</h3>
      <dl class="project-facts">
        <dt>Repository</dt>
        <dd id="project-repository">
          {#if repository}
            <a href={repository} target="_blank" rel="noopener noreferrer">{repository.replace(/^https?:\/\//, '')}</a>
          {:else}
            <span class="muted">Not set: <code>atm repository set &lt;url&gt;</code></span>
          {/if}
        </dd>
        <dt>Workflow</dt>
        <dd id="project-workflow">
          {#if workflow}
            <div>{workflow.type}{workflow.base ? ` → ${workflow.base}` : ''}</div>
            <div class="muted">{workflow.verify ? 'Verifies before pushing' : 'Pushes without verifying'}</div>
            {#if workflow.commitTemplate}<div><code>{workflow.commitTemplate}</code></div>{/if}
            {#if workflow.trailers.length}
              <div class="muted">
                Required trailers: {#each workflow.trailers as line (line)}<code>{line}</code>
                {/each}
              </div>
            {/if}
          {:else}
            <span class="muted">Not set: <code>/atm-push</code> asks the first time</span>
          {/if}
        </dd>
        <dt>People</dt>
        <dd id="project-people">
          {#each who.people as p (p.name)}
            <div class="project-person" data-name={p.name}>
              {p.name}
              <span class="muted">created {p.created} · started {p.started} · completed {p.completed}</span>
            </div>
          {:else}
            <span class="muted">None recorded</span>
          {/each}
        </dd>
        <dt>Agents</dt>
        <dd id="project-agents">
          {plural(who.agents.sessions, 'session')} · started {who.agents.started} · completed {who.agents.completed}
          · {who.agents.agentMs ? formatMinutes(who.agents.agentMs / 60_000) : '0 m'} agent time
        </dd>
      </dl>

      <h4 class="menu-label">Vocabulary</h4>
      <dl class="project-facts project-vocab" id="project-vocab">
        <!-- The sprints in the planning order, the backlog, then the closed ones, folded (T-323) -->
        <dt>Sprints</dt>
        <dd id="project-sprints">
          {#each open as s (s.id)}
            <span class="vocab-item" data-sprint={s.id} data-state={s.state}
              ><SprintMarker state={s.state} />{s.id} <strong>{s.count}</strong></span
            >
          {/each}
          {#if counts.backlog || counts.sprints.length}
            <span class="vocab-item" data-sprint="backlog">Backlog <strong>{counts.backlog}</strong></span>
          {/if}
          {#if showClosed}
            {#each closed as s (s.id)}
              <span class="vocab-item" data-sprint={s.id} data-state={s.state}
                ><SprintMarker state={s.state} />{s.id} <strong>{s.count}</strong></span
              >
            {/each}
          {/if}
          {#if closed.length}
            <button class="link-button" id="project-closed-toggle" onclick={() => (showClosed = !showClosed)}
              >{showClosed ? 'Hide closed' : `▸ ${closed.length} closed`}</button
            >
          {/if}
        </dd>
        <!-- The parts of the project (atm components), with their files' globs on hover (T-323) -->
        <dt>Components</dt>
        <dd id="project-components">
          {#each componentCounts as c (c.id)}
            <span
              class="vocab-item"
              data-component={c.id}
              title={c.paths.length ? `${c.id}: ${c.paths.join(', ')}` : `${c.id}: no files yet`}
              >{c.label} <strong>{c.count}</strong></span
            >
          {:else}
            <span class="muted">None: <code>atm components add</code></span>
          {/each}
        </dd>
        <dt>Labels</dt>
        <dd id="project-labels">
          {#each counts.labels as l (l.id)}
            <span class="vocab-item" data-label={l.id} title={l.text === l.id ? undefined : l.id}
              >{l.text} <strong>{l.count}</strong></span
            >
          {:else}
            <span class="muted">None</span>
          {/each}
        </dd>
        <dt>Types</dt>
        <dd id="project-types">
          {#each counts.types as t (t.id)}
            <span class="vocab-item" data-type={t.id}>{t.label} <strong>{t.count}</strong></span>
          {/each}
        </dd>
        <dt>Statuses</dt>
        <dd id="project-statuses">
          {#each counts.statuses as s (s.id)}
            <span class="vocab-item" data-status={s.id}
              >{s.label}{s.kind && s.kind !== s.id ? ` (${s.kind})` : ''} <strong>{s.count}</strong></span
            >
          {/each}
        </dd>
        <dt>Priorities</dt>
        <dd id="project-priorities">
          {#each counts.priorities as p (p.id)}
            <span class="vocab-item" data-priority={p.id}
              >{p.id} {p.label}{p.isDefault ? ' (default)' : ''} <strong>{p.count}</strong></span
            >
          {/each}
        </dd>
      </dl>
    </section>

    <section class="project-card" id="project-stats" aria-labelledby="project-stats-title">
      <h3 class="menu-label project-card-title">
        <span id="project-stats-title">Stats</span>
        <InfoTip label="the stats">
          <p>
            The whole project, archived tasks included. <strong>Agent time</strong> is the work time of every task, from its
            start until it asked for a review or was done; agents working in parallel add up. Hover a number for how it is
            counted.
          </p>
          <p>
            <strong>Task commits</strong> and <strong>pull requests</strong> are those atm recorded on tasks: atm doesn't
            read git history here.
          </p>
        </InfoTip>
      </h3>
      <div class="project-numbers">
        {#each numbers as n (n.key)}
          <div class="project-number" data-stat={n.key} title={n.title}>
            <span class="kpi-value">{n.value}</span>
            <span class="kpi-label">{n.label}</span>
          </div>
        {/each}
      </div>
      {#if stats.parallelism !== null}
        <p class="project-parallelism" id="project-parallelism">
          {stats.parallelism.toFixed(1)} agents working on average while any works
        </p>
      {/if}
      <TaskTimeHistogram histogram={timeHistogram(tasks, vocab)} />
    </section>
  </div>
</div>
