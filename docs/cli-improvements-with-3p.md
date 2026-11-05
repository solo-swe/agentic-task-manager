# CLI improvements with third-party libraries

**Status:** proposal — enabled by T-414 (build modernization allows 3rd-party dev dependencies).

Now that the build can bundle dependencies into `plugin/scripts/atm.py`, the `atm-cli` codebase can stop hand-rolling everything in the Python standard library. This document surveys the CLI codebase (`atm-cli/atm/cli/`, `atm-cli/atm/common/`, `atm-cli/atm/lib/`, `atm-cli/atm/api/`) and identifies concrete improvements with specific third-party libraries.

---

## 1. CLI parsing: `click` or `typer` (replaces `argparse`)

**Current:** `atm/cli/main.py` builds a flat `argparse` parser with ~40 top-level commands, manual subparser dispatch, handwritten `USAGE` string, and ad-hoc option validation.

**Problem:** Inconsistent option names, no nested groups, manual `--json` handling everywhere, no rich help, no type coercion beyond `argparse`'s basics.

**Improvement:** Adopt `click` (or `typer`, see §6) for nested command groups:

```python
@click.group()
@click.option("--dir", help="run as if started in PATH")
@click.option("--json", is_flag=True, help="print result as JSON")
@click.pass_context
def cli(ctx, dir, json):
    ctx.obj = {"dir": dir, "json": json}

@cli.group()
def task():
    """Task lifecycle."""

@task.command()
@click.argument("task_id")
@click.option("--state", type=click.Choice(["todo","in_progress","in_review","done","blocked","skipped"]))
@click.option("--note")
def state(ctx, task_id, state, note):
    ...
```

**Benefits:** nested `task state`, `project config`, `vcs commit`; declarative `click.Choice` validation; auto-generated consistent help; `Context.obj` for passing `--agent`, `--dir`, `--json` down through groups.

---

## 2. Data validation & modeling: `pydantic` (replaces ad-hoc validation)

**Current:** `atm/common/model.py` validates `meta.statuses`, `meta.sprints`, `meta.releases`, task IDs, branch names, notes, etc. with handwritten `_terms()`, `check_sprints()`, `check_releases()`, `valid_task_id()`, `check_note()`, and `STATUS_CHANGES` dict. Tasks are plain `dict`s.

**Problem:** Validation logic is scattered across `model.py`, `build.py` (schema → types), and CLI commands. Task dicts are untyped; invalid data surfaces as `AtmError` at runtime rather than at load time.

**Improvement:** Use `pydantic v2` (Python 3.9 compatible) for:

- **Task model:** `Task(BaseModel)` with `id: constr(pattern=...)`, `status: Literal[...]`, `created_at: datetime`, etc.
- **Meta model:** `Meta(BaseModel)` with `statuses: list[StatusTerm]`, `sprints: list[Sprint]`, validation via `@model_validator`
- **CLI options:** pydantic models for command arguments (e.g. `AddTask` with `--name`, `--type`, `--priority`)

```python
from pydantic import BaseModel, constr, Field, model_validator
from typing import Literal

class TaskBase(BaseModel):
    id: constr(pattern=r"[A-Za-z0-9][A-Za-z0-9._-]{0,63}")
    status: Literal["todo","in_progress","in_review","blocked","done","skipped"]
    priority: str = "P2"
    created_at: datetime
    updated_at: datetime
    started_at: datetime | None = None
    completed_at: datetime | None = None
    notes: list[TaskNote] = Field(default_factory=list)
```

**Benefits:** Validation at load time (fail fast), self-documenting models, automatic JSON serialization/deserialization, cleaner `change_status` (operates on typed models not dicts).

**Tradeoff:** pydantic adds ~1-2 MB to the bundled artifact. The `build.py` bundler must vendor it.

---

## 3. State machine: `transitions` or `python-statemap`

**Current:** `atm/common/model.py` `STATUS_CHANGES` dict + `change_status()` (~150 lines of imperative `if/elif`) implements task state transitions. Guards (dependencies done, review decided) and side effects (notes, timestamps, claims) are mixed together.

**Improvement:** Declare transitions as data:

```python
from transitions import Machine

class Task:
    states = ["todo","active","review","blocked","done","skipped"]
    transitions = [
        {"trigger":"start","source":["todo","review"],"dest":"active",
         "conditions":"dependencies_done","before":"create_claim"},
        {"trigger":"done","source":["active","review"],"dest":"done",
         "before":"record_completion"},
        ...
    ]
```

**Benefits:** State graph is inspectable (render as Mermaid), guards and side effects separated via `conditions`/`before`/`after` hooks, invalid transitions are library errors.

---

## 4. Output formatting: `rich` (replaces hand-rolled text rendering)

**Current:** `atm/cli/render.py` builds strings manually with `ljust()`, f-strings, and `\n`. `atm list` output is plain text with manual column width calculation. No colors, no tables, no progress indicators.

**Problem:** Output is hard to extend (colored status badges, dependency trees, tables with wrapping), and agents reading terminal output get no visual structure.

**Improvement:** Use `rich` for:

```python
from rich.table import Table
from rich.console import Console

console = Console()
table = Table(show_header=True, header_style="bold")
table.add_column("ID")
table.add_column("Status")
table.add_column("Priority")
table.add_column("Name")
for row in rows:
    table.add_row(row["id"], row["status"], row["priority"], row["name"])
console.print(table)
```

**Benefits:** Color-coded statuses (`todo`=dim, `in_progress`=yellow, `done`=green, `blocked`=red), tables with auto-width, panels for `atm show`, progress bars for `atm archive`/`unarchive`, markdown rendering for task descriptions.

**Tradeoff:** `rich` is ~1 MB bundled. Only load it when a TTY is detected; fall back to plain text for piped output.

---

## 5. Logging & observability: `loguru` or `structlog` (replaces `print`/`sys.stderr`)

**Current:** Errors go to `sys.stderr` via `render.report_expired()` and `main.report()`. Output goes to `sys.stdout` via `out()`. No structured logging, no log levels, no log files.

**Improvement:** `loguru` for simple structured logging:

```python
from loguru import logger

logger.add("atm.log", rotation="1 MB", level="DEBUG")
logger.info("Starting task {task_id}", task_id=task.id)
logger.error("Lease expired for {task}", task=task.id)
```

Or `structlog` for JSON-structured logs (useful for the server process).

**Benefits:** Consistent log format, levels (DEBUG/INFO/WARNING/ERROR), file rotation, exception backtraces captured cleanly (important since current code deliberately suppresses tracebacks for security).

---

## 6. Date/time handling: `pendulum` or `arrow`

**Current:** `atm/common/core.py` uses `datetime.now(timezone.utc)`, `strptime("%Y-%m-%dT%H:%M:%SZ")`, manual `format_duration()`. `ATM_NOW` env override for tests.

**Problem:** Manual ISO 8601 parsing is fragile; timezone handling is manual; relative time formatting ("3h ago") is hand-rolled.

**Improvement:** `pendulum` (or `arrow`) for:

```python
import pendulum
now = pendulum.now("UTC")
lease_expires = now.add(minutes=30)
human = pendulum.instance(past).diff_for_humans()  # "2 hours ago"
```

**Benefits:** Correct timezone handling, immutable datetimes, human-readable relative time, easy test mocking.

**Tradeoff:** `pendulum` has native extensions (slower install, larger bundle). `arrow` is pure Python. For Python 3.9 compatibility, check versions carefully.

---

## 7. HTTP server & client: `httpx` + `anyio` or `uvicorn`

**Current:** `atm/api/server.py` uses `http.server` (stdlib) with raw `socket`, `threading`, manual URL parsing (`urllib.parse`), manual CSP header construction, manual multipart handling. `cli/serve.py` uses `subprocess` + `time.sleep` polling.

**Problem:** The server is single-threaded-ish with manual routing; no WebSocket support; no connection pooling; manual HTTP parsing is error-prone; `--refresh` polling is naive.

**Improvement:**

- **Server:** `uvicorn` + a lightweight framework (or raw `anyio` streams) for the REST API. `httpx` for the client side (`whoami()`, server health checks).
- **File watching:** `watchdog` for `serve --refresh` (detect dashboard file changes instead of polling).
- **Process management:** `psutil` for `pid_alive()`, zombie detection, cross-platform process checks (currently Linux-specific `/proc` parsing).

```python
# Example: httpx for server health check
import httpx
async def whoami(port):
    async with httpx.AsyncClient() as client:
        resp = await client.get(f"http://localhost:{port}/__atm/whoami", timeout=1.0)
        return resp.json()
```

**Benefits:** Proper HTTP semantics, async handling, WebSocket for real-time dashboard updates, cross-platform process management.

---

## 8. Cross-platform file locking: `filelock`

**Current:** `atm/db/jsongit/lock.py` uses `fcntl.flock` (Unix-only) with a custom lock directory fallback. `cli/serve.py` `server_lock()` uses `mkdir` as a lock primitive with pid checking.

**Problem:** `fcntl` doesn't work on Windows; the custom `mkdir` lock is fragile (stale locks, pid reuse).

**Improvement:** `filelock` for cross-platform, robust file locking with timeout support:

```python
from filelock import FileLock
lock = FileLock("tasks.json.lock", timeout=10)
with lock:
    # modify tasks.json
```

**Benefits:** Works on Windows/macOS/Linux, automatic stale-lock handling, timeout semantics.

---

## 9. JSON schema validation: `jsonschema`

**Current:** `atm-cli/build.py` generates Python TypedDicts and TypeScript interfaces from JSON Schemas, but runtime validation of `tasks.json` against those schemas is manual (`check()` command, `model.py` functions).

**Improvement:** `jsonschema` to validate `tasks.json` at load time:

```python
from jsonschema import validate, ValidationError
validate(instance=tasks_data, schema=tasks_schema)
```

**Benefits:** Single source of truth (the schema), catch structural errors early, `atm doctor` becomes a schema validation pass.

---

## 10. Testing: `pytest` + `hypothesis`

**Current:** `tests/scripts/` drives atm-cli through the launcher. Unit tests are `unittest`-based.

**Improvement:** `pytest` with fixtures, `pytest-mock`, and `hypothesis` for property-based testing (e.g. "any valid task state transition preserves invariants").

---

## 11. Summary: recommended library set

| Purpose | Library | Why | Bundle impact |
|---------|---------|-----|---------------|
| CLI parsing | `click` (or `typer`) | Nested subcommands, consistent help | ~0.1 MB |
| Data validation | `pydantic` | Typed task/meta models, fail-fast validation | ~1 MB |
| State machine | `transitions` | Declarative transitions, guards, hooks | ~0.05 MB |
| Output formatting | `rich` | Tables, colors, panels, progress bars | ~1 MB |
| Logging | `loguru` | Structured logs, levels, rotation | ~0.2 MB |
| Date/time | `pendulum` or `arrow` | Timezones, relative time, immutability | ~0.1-0.3 MB |
| HTTP server/client | `httpx` + `uvicorn` | Proper HTTP, async, WebSocket | ~0.3 MB |
| File watching | `watchdog` | `serve --refresh` via file events | ~0.1 MB |
| File locking | `filelock` | Cross-platform locks, timeouts | ~0.05 MB |
| JSON schema | `jsonschema` | Validate tasks.json against schema | ~0.1 MB |
| Testing | `pytest` + `hypothesis` | Better fixtures, property-based tests | dev only |

**Total estimated bundle growth:** ~2-4 MB. Still a single `atm.py` file customers run with `python3 >= 3.9` — no user-facing prerequisites added.

---

## 12. Migration approach (per T-414)

Don't rewrite everything at once. Adopt libraries one area at a time, behind the wrapper:

1. **Phase 1:** `click` for one command group (e.g. `task`), `pydantic` for task model loading.
2. **Phase 2:** `transitions` for state machine, `rich` for `atm list`/`atm show` output.
3. **Phase 3:** `loguru` for logging, `filelock` for cross-platform locking.
4. **Phase 4:** `httpx`/`uvicorn` for `atm serve`, `watchdog` for `--refresh`.
5. **Phase 5:** Remove hand-rolled validation, parsing, and rendering code.

Each phase keeps old commands working as wrappers, tests pass, then old code is deleted.

---

## 13. Open questions

- `click` vs `typer` (see `docs/modernizing-build-approach.md` §6)?
- `pydantic` v1 vs v2 (v2 is faster but larger; v1 is EOL)?
- `rich` only when stdout is a TTY, or always?
- `pendulum` (native extensions) vs `arrow` (pure Python) for Python 3.9 compat?
- Keep `http.server` for `atm serve` or switch to `uvicorn` + framework?
