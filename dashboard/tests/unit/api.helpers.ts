// Answers as atm serve's v1 routes give them (docs/multi-project-api.md §2.5), for tests that play the server.

/** GET api/v1/projects/<id>/tasks for a project holding `data` as its tasks.json: its meta, its size and each task
 * as a view. The derived values are left empty: the dashboard doesn't read them yet (docs/layered-architecture.md
 * §9, step 6). */
export function asTaskViews(data: { meta?: unknown; tasks?: unknown[] }, bytes = JSON.stringify(data).length) {
  return {
    generated_at: '2026-01-01T00:00:00Z',
    meta: data.meta ?? {},
    bytes,
    tasks: (data.tasks ?? []).map((task) => ({ task, derived: {} })),
  };
}

/** A project's v1 tasks URL, as the dashboard asks for it */
export const v1Tasks = (project: string) => `/api/v1/projects/${project}/tasks`;
