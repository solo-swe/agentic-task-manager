// Generated from atm-cli/atm/common/schema/ by atm-cli/build.py: don't edit it, change the schema and run
// `npm run build:cli` in dashboard/ (the pre-commit hook does it too).

export interface StatusEntry {
  id: string;
  label?: string;
  /** What the status means */
  kind: 'todo' | 'active' | 'review' | 'blocked' | 'done' | 'skipped';
}

export interface VocabularyTerm {
  id: string;
  label?: string;
}

export interface LabelEntry {
  id: string;
  label?: string;
  description?: string;
}

export interface PhaseEntry {
  id: number;
  name?: string;
}

/** How the project pushes (docs/v4/skills-and-workflows.md §1) */
export interface Workflow {
  /** direct or pull-request */
  type?: string;
  base?: string;
  verify?: boolean;
  max_rounds?: number;
  commit_template?: string;
  /** "forbid" or "none", or {"require": [the lines]}: what commit messages end with */
  trailers?: unknown;
}

/** When atm suggests archiving, and whether agents may do it (DM-5) */
export interface ArchiveSettings {
  threshold_kb?: number;
  threshold_tasks?: number;
  keep_days?: number;
  auto?: boolean;
}

export interface Sprint {
  /** The sprint's name */
  id: string;
  goal?: string;
  state?: 'planned' | 'active' | 'closed';
  release?: string;
  /** ISO 8601 UTC */
  start?: string;
  /** ISO 8601 UTC */
  end?: string;
  /** YYYY-MM-DD */
  due?: string;
  report?: SprintReport;
}

/** What a closed sprint did (SR-2.3) */
export interface SprintReport {
  /** The tasks done in it */
  done?: string[];
  /** The unfinished tasks carried on */
  carried?: string[];
  /** Where the carried tasks went */
  to?: string;
}

export interface Release {
  id: string;
  name?: string;
  state?: 'planned' | 'released' | 'archived';
  /** YYYY-MM-DD */
  target?: string;
  /** ISO 8601 UTC */
  released_at?: string;
}

export interface Component {
  id: string;
  label?: string;
  /** Globs over its files */
  paths?: string[];
  /** Its tasks' default review mode */
  review?: string;
}

/** A project's meta in tasks.json: its settings, vocabularies, sprints, releases and components (docs/v4/data-model.md §2). Every key is optional and falls back to its default. */
export interface Meta {
  /** The project's ID (8 hex characters, set by atm init) */
  id?: string;
  /** The project's name */
  project?: string;
  /** One line about the project */
  description?: string;
  created?: string;
  /** The repository's web address, for commit and PR links */
  repository?: string;
  /** github, gitlab, bitbucket or gitea, for a host whose name doesn't say */
  repository_type?: string;
  /** The project's logo, relative to its root */
  logo?: string;
  /** How long a claim lasts without a heartbeat */
  lease_minutes?: number;
  statuses?: StatusEntry[];
  types?: VocabularyTerm[];
  /** Most urgent first */
  priorities?: VocabularyTerm[];
  /** The priority of tasks without one */
  default_priority?: string;
  labels?: LabelEntry[];
  /** Before sprints (atm migrate turns them into sprints) */
  phases?: PhaseEntry[];
  workflow?: Workflow;
  archive?: ArchiveSettings;
  /** How many tasks tasks-archive.json holds (DM-5.4) */
  archived?: number;
  /** The number the next new task gets */
  next_id?: number;
  /** In order */
  sprints?: Sprint[];
  /** In order */
  releases?: Release[];
  components?: Component[];
  /** How a sprint created without a name is named: people or time */
  sprint_names?: string;
  /** The project's default review mode: blocker, async, batch or none */
  review?: string;
}

export interface Note {
  /** ISO 8601 UTC */
  at: string;
  /** Who wrote it */
  by?: string;
  /** handoff, outcome, review, rejection, failure, answer or note */
  kind: string;
  /** Markdown, at most 500 characters */
  text: string;
}

/** The PR that contains the task (atm pr): the task is In PR until merged_at (DM-4.3) */
export interface PullRequest {
  number: number;
  url?: string;
}

export interface BlockRound {
  /** ISO 8601 UTC */
  from: string;
  /** ISO 8601 UTC */
  to: string;
  /** user or agent */
  on?: string;
}

export interface ReviewRound {
  /** ISO 8601 UTC */
  from: string;
  /** ISO 8601 UTC */
  to: string;
  outcome?: string;
}

export interface SprintHistoryEntry {
  sprint: string;
  /** ISO 8601 UTC */
  from: string;
  /** ISO 8601 UTC; absent while it is in the sprint */
  to?: string;
}

/** Before v4: the last failed attempt (now a failure note; still read) */
export interface LastFailure {
  at?: string;
  reason?: string;
}

/** One task, as tasks.json and tasks-archive.json keep it (docs/v4/data-model.md §4). Unknown fields are kept as they are. */
export interface Task {
  /** Letters, digits, '.', '_' and '-' (T-001) */
  id: string;
  name: string;
  /** A type ID from the vocabulary (meta.types) */
  type: string;
  /** A status ID from the vocabulary (meta.statuses); atm acts on its kind */
  status: string;
  /** Markdown */
  description?: string;
  /** Before sprints: the task's phase (atm migrate turns phases into sprints) */
  phase?: number;
  /** A priority ID from the vocabulary ("P0"…"P3" by default); missing means the default priority */
  priority?: string;
  /** Free-form tags that group tasks across sprints and types, e.g. ["spike-2", "ux"] (DM-4.6) */
  labels?: string[];
  /** Task IDs it waits for */
  dependencies?: string[];
  requirements?: string[];
  use_cases?: string[];
  /** Files it creates or changes */
  files?: string[];
  /** ISO 8601 UTC */
  created_at?: string;
  /** ISO 8601 UTC */
  updated_at?: string;
  /** ISO 8601 UTC; set when the task moves to in progress */
  started_at?: string;
  /** ISO 8601 UTC; set when the task is done */
  completed_at?: string;
  /** ISO 8601 UTC; set when the task is skipped (atm skip) */
  skipped_at?: string;
  /** Before v4: the agent that started the task. Now started_by (readers fall back to it) */
  agent?: string;
  /** Who created it (docs/v4/data-model.md §4.1) */
  created_by?: string;
  /** Who changed it last */
  updated_by?: string;
  /** Who started it: the claim's owner */
  started_by?: string;
  /** Who finished it */
  completed_by?: string;
  /** The SHA of the task's commit */
  commit?: string;
  pr?: PullRequest;
  /** ISO 8601 UTC; when that PR was merged (atm merged) */
  merged_at?: string;
  /** ISO 8601 UTC; when the task asked for a review (atm review). Its work ended here; review time isn't work */
  review_requested_at?: string;
  /** A blocked task waits on a decision by the user or an agent (atm block --on) */
  waiting_on?: string;
  /** The open questions of a blocked task */
  questions?: string[];
  /** ISO 8601 UTC; when it was blocked */
  blocked_at?: string;
  /** Past blocks, oldest first, with who each waited on (T-244) */
  blocks?: BlockRound[];
  /** Past reviews, oldest first */
  reviews?: ReviewRound[];
  /** Handoffs, outcomes, review requests, answers, rejections (docs/v4/data-model.md §4) */
  notes?: Note[];
  last_failure?: LastFailure;
  /** The sprint it is in (a sprint's name, meta.sprints); absent: the backlog */
  sprint?: string;
  /** Each sprint it was in, from when to when; the open entry has no `to` (SR-2.5) */
  sprint_history?: SprintHistoryEntry[];
  /** The release it ships in; when absent, its sprint's (SR-2.6) */
  release?: string;
  /** Component IDs (meta.components) */
  components?: string[];
  /** How it is reviewed: blocker, async, batch or none */
  review?: string;
  /** In the review kind: waiting, changes_requested or approved (SR-6.1) */
  review_state?: string;
  /** ISO 8601 UTC; when the review was decided */
  reviewed_at?: string;
  /** A batch task done by an agent: pending until the end-of-sprint review, then reviewed (SR-4.4) */
  sprint_review?: string;
  /** `user`: the work is done together with the user (SR-6.5) */
  with?: string;
  /** Related tasks (a link that doesn't block), stored on one side and shown on both (SR-2.8) */
  related?: string[];
  /** How a finished task ended, when it isn't the default: wont_do, duplicate, obsolete or done (SR-2.8) */
  resolution?: string;
  /** The task this one duplicates (with the duplicate resolution) */
  duplicate_of?: string;
}

/** atm's suggestion to archive (CLI-5.1), present when the working tasks are over meta.archive's thresholds (T-372) */
export interface ArchiveHint {
  /** How big the working task data is */
  bytes: number;
  /** How many working tasks are finished */
  finished: number;
  /** How many of them `atm archive` would move */
  archivable: number;
  /** Whether agents may archive on their own (meta.archive.auto) */
  auto: boolean;
  /** The hint as atm says it */
  text: string;
}

/** A task as stored, and what atm derives for it */
export interface TaskView {
  task: Task;
  derived: TaskDerived;
}

/** What atm-lib works out for a task, so no front end applies atm's rules itself */
export interface TaskDerived {
  /** The status's kind: todo, active, review, blocked, done or skipped (absent for an unknown status) */
  kind: string;
  /** To do, and every dependency done (an archived one counts as done) */
  ready: boolean;
  /** A to-do task's dependencies that aren't done yet */
  waiting_for: string[];
  /** Who a blocked task waits on: user or agent */
  waiting_on?: string;
  /** Its priority, or the project's default */
  priority: string;
  /** Its review mode: its own, else its component's, else the project's */
  effective_review?: string;
  /** Its release: its own, else its sprint's */
  effective_release?: string;
  /** A done task's stage: done, in_pr or merged (DM-4.3) */
  stage?: string;
  /** From the archive */
  archived: boolean;
  /** What atm finds wrong with the task (T-377); it isn't ready then */
  problems?: string[];
}

export interface DependencyItem {
  id: string;
  /** Its status (", archived" added for an archived task), or missing */
  status: string;
  name: string;
}

/** atm deps: what a task waits for, and what waits for it */
export interface DepsResult {
  id: string;
  waits_for: DependencyItem[];
  waited_on_by: DependencyItem[];
}

/** GET /api/v1/projects/<p>/tasks: the tasks the query's filters keep, each with its derived values (docs/layered-architecture.md §6.2) */
export interface TaskViewList {
  /** ISO 8601 UTC */
  generated_at: string;
  meta: Meta;
  /** How big the project's working task data is (tasks.json), for the archive notice */
  bytes: number;
  tasks: TaskView[];
  /** What atm finds wrong with the task file as a whole: entries left out, vocabularies that fell back to their defaults (T-377) */
  problems?: string[];
  archive_hint?: ArchiveHint;
}
