# Tasks: the fields of a task and of the project, as `--json` gives them

For reading `atm task details --json` and the other commands' JSON, not for editing the file.

- **Definition:** `id`, `name`, `type` (`feature`, `bugfix`, `test`, `infra`, `docs`, `refactor`, `polish` by default),
  `description` (Markdown), `dependencies` (task IDs), `priority` (`P0`–`P3` by default; none counts as `P2`), `labels`,
  `sprint`, `release`, `components`, `review` (its review mode), `with` (`user`: done together with the user),
  `requirements`, `use_cases`, `files`, `related`.
- **Status:** `status` (with its kind), `created_at`, `updated_at`, `started_at`, `completed_at`, `skipped_at`,
  `review_requested_at`, `review_state` (`waiting`, `approved`, `changes_requested`, `rejected`); times are ISO 8601
  UTC.
- **Actors:** `created_by`, `updated_by`, `started_by` (the claim's owner), `completed_by`. An actor starting with
  `claude:` (or another agent prefix) is an agent; anything else is a person.
- **Blocked:** `waiting_on` (`user` or `agent`), `questions`, `blocked_at`.
- **Notes:** `notes`, a list of `{at, by, kind, text}`; the kinds: `handoff`, `outcome`, `review`, `rejection`,
  `failure`, `answer`, `note`.
- **Git:** `commit` (the task's SHA), `pr` (`{number, url}`), `merged_at`. Done, In PR and Merged are derived from them
  and never affect dependencies.
- **The project's `meta`:** `project`, `id`, `description`, `repository` (its web address), `statuses`, `types`,
  `priorities`, `default_priority`, `labels`, `sprints`, `releases`, `components`, `workflow`
  (`{type: direct|pull-request, base, verify, max_rounds, commit_template, trailers}`), `lease_minutes`, `archive`,
  `archived`, `next_id`.

## Task text
Descriptions, notes and questions render on the dashboard as Markdown with CompMD: tables, task lists, ` ```mermaid `
diagrams, and the task's own data as `md.data.task`; never write a frontmatter of your own. Link a file of the project
with a relative link from its root (`docs/x.md#routes`, `src/app.ts#L40`), and a requirement or a place in a document
with a text fragment (`docs/x.md#:~:text=SR%2D2.1`, with `-`, `,` and `&` in the text as `%2D`, `%2C`, `%26`); the
dashboard opens them in its document viewer.
