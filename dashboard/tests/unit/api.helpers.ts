// Answers as atm serve's v1 routes give them (docs/multi-project-api.md §2.5), for tests that play the server.
import type { Task, TasksFile } from '../../src/lib/types';
import { sprintModel } from './views';

/** GET api/v1/projects/<id>/tasks for a project holding `data` as its tasks.json: its meta, its size, its sprints and
 * each task as a view, in its sprint (ATM-720; the sprints are views.ts's fake of atm's). The other derived values are
 * left out. */
export function asTaskViews(data: { meta?: unknown; tasks?: unknown[] }, bytes = JSON.stringify(data).length) {
  const tasks = (data.tasks ?? []) as Task[];
  const model = sprintModel(data.meta as TasksFile['meta'], tasks);
  return {
    generated_at: '2026-01-01T00:00:00Z',
    meta: data.meta ?? {},
    bytes,
    sprints: { sprints: model.sprints, backlog: model.backlog, from_phases: model.derived },
    tasks: tasks.map((task) => {
      const sprint = model.sprintOf(task);
      return { task, derived: sprint ? { sprint } : {} };
    }),
  };
}

/** A project's v1 tasks URL, as the dashboard asks for it */
export const v1Tasks = (project: string) => `/api/v1/projects/${project}/tasks`;
