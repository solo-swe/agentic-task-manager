import type { Task as StoredTask } from './generated/types';

export type { Note } from './generated/types';

/** The default statuses; a project may define its own in meta.statuses (see vocab.ts). */
export type KnownStatus = 'todo' | 'in_progress' | 'in_review' | 'blocked' | 'done' | 'skipped';
export type Status = KnownStatus | (string & {});

export type KnownTaskType = 'feature' | 'bugfix' | 'bug' | 'test' | 'infra' | 'docs' | 'refactor' | 'polish';
export type TaskType = KnownTaskType | (string & {});

/** A task: its fields come from common's schema (atm-cli/atm/common/schema/task.schema.json, generated into
 * generated/types.ts). The dashboard reads two of them more narrowly than the file allows, as it always has: a task
 * from before sprints has a `phase`, and a missing description reads as empty. */
export interface Task extends StoredTask {
  phase: number;
  description: string;
  type: TaskType;
  status: Status;
}

export interface TasksFile {
  meta?: {
    project?: string;
    id?: string;
    created?: string;
    description?: string;
    /** The repository's web address (atm repository), for commit and PR links (lib/repo.ts) */
    repository?: string;
    /** The host's kind when its name doesn't say (a self-hosted server): github, gitlab, bitbucket or gitea */
    repository_type?: string;
    lease_minutes?: number;
    // Project vocabularies (docs/v4/data-model.md §2); validated by vocab.ts, so typed loosely here
    statuses?: unknown;
    phases?: unknown;
    types?: unknown;
    priorities?: unknown;
    default_priority?: unknown;
    labels?: unknown;
    workflow?: unknown;
    /** How many tasks are in tasks-archive.json (written by atm archive / unarchive) */
    archived?: number;
    /** Archive settings: threshold_kb, threshold_tasks, keep_days, auto (docs/v4/data-model.md §5) */
    archive?: unknown;
    /** The sprints, in order (docs/sprints-and-releases-design.md §2.1); read leniently by lib/sprints.ts */
    sprints?: unknown;
    /** The releases, in order (§2.1); read leniently by lib/releases.ts */
    releases?: unknown;
    /** The components ({id, label, paths, review}); read leniently by lib/review.ts */
    components?: unknown;
    /** The project's default review mode */
    review?: unknown;
  };
  tasks?: Task[];
}
