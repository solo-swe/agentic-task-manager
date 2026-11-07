// A project's summary as the server sends it (api.schema.json's ProjectSummary, T-382): every field, for a project
// with nothing going on; a test changes what it is about.
import type { BoardColumn, ProjectBoard, ProjectSummary } from '$generated/types';

const column = (): BoardColumn => ({ count: 0, cards: [] });

export const emptyBoard = (): ProjectBoard => ({
  todo: column(),
  active: column(),
  review: column(),
  blocked: column(),
  done: column(),
});

export function summary(fields: Partial<ProjectSummary> = {}): ProjectSummary {
  return {
    total: 0,
    done: 0,
    skipped: 0,
    percent: 0,
    active_sprint: null,
    recent_sprints: null,
    agents_working: 0,
    working: [],
    next: [],
    last_done: null,
    needs_you: { review: 0, blocked_on_user: 0, together: 0 },
    needs_you_tasks: [],
    git: { branch: null, pr: null },
    description: null,
    last_activity: null,
    completions_14d: [],
    board: emptyBoard(),
    board_active_sprint: null,
    stats: { started: null, tasks: 0, agent_minutes: 0 },
    ...fields,
  };
}

/** A summary from an older server, without the fields it didn't send yet (the dashboard still copes with one) */
export function olderServer(numbers: ProjectSummary, ...without: (keyof ProjectSummary)[]): ProjectSummary {
  const older: Partial<ProjectSummary> = { ...numbers };
  for (const key of without) delete older[key];
  return older as ProjectSummary;
}
