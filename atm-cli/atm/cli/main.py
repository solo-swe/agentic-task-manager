"""The command line: one argparse parser, a subparser per command, and main().

Every command takes --json. Usage errors keep atm's format ("atm: <message>", exit code 2) followed by the command's
usage line; with --json, every error is {"error": "...", "code": N} on stderr, with the same exit code.
"""

import argparse
import os
import sys

from ..common.errors import AtmFailure
from ..components import cmd_components
from ..core import AtmError, AtmExit, out, plugin_version, to_json
from ..model import REVIEW_MODES
from ..lock import cmd_lock_test
from ..merge import cmd_init, cmd_merge_driver
from ..migrate import cmd_migrate
from ..paths import cmd_paths
from ..releases import cmd_releases
from ..sprints import cmd_backlog, cmd_move, cmd_sprints
from ..logo import cmd_logo
from ..registry import cmd_projects
from ..repository import cmd_repository
from ..api.server import DEFAULT_PORT, cmd_serve, cmd_serve_child, refresh_server
from ..archive import cmd_archive, cmd_unarchive
from ..edit import cmd_add, cmd_check, cmd_edit
from ..git_ops import cmd_commit, cmd_merged, cmd_pr, cmd_workflow
from .claims import cmd_claims, cmd_heartbeat, cmd_next, cmd_run
from .query import cmd_deps, cmd_list, cmd_status, cmd_summary
from .status import cmd_note, cmd_status_change
from .show import cmd_show

USAGE = """usage: atm [--dir PATH] <command> [options]

  --dir PATH     run as if started in PATH: on the project there (relative paths in the command count from it)

commands:
  version        print the plugin version
  paths [--json] print where this project's task files are
  migrate [--json]
                 move tasks.json from docs/ or the root into .agentic-task-manager/,
                 and set up the folder (.gitignore, current dashboard)
  serve [start|stop|status] [--open] [--all] [--port N] [--dir PATH] [--refresh]
                 run the dashboard server in the background: one per user, for all registered projects
                 (start registers this project, or the one at PATH, and prints its URL; --all the overview's;
                 --refresh only restarts a running server whose dashboard is older than this plugin's)
  status         the overview: counts, what is in progress, in review or blocked, the next task
  summary        the project's numbers as atm serve gives them to the dashboard (JSON)
  show <id>...   tasks in full (description, dependencies, files, notes)
  list [--status S] [--sprint S | --backlog] [--type T] [--priority P] [--label L] [--agent A] [--ready]
       [--blocked] [--in-pr] [--with-user] [--derived]
                 one line per task: ID, status, priority, labels, name (--derived: each task in full with what atm
                 derives for it, as JSON)
  deps <id>      what a task waits for, and what waits for it
  start <id>     mark a task in_progress and claim it (its dependencies must be done); prints the task
  review <id> [--note TEXT]
                 ask for a review of a finished task: in_review, and its claim is released
  done <id> [--note TEXT] [--commit SHA] [--resolution R]
                 mark an in_progress or in_review task done and drop its claim; the note is a one-line
                 outcome (for a task in review, done means the review passed); prints the tasks now ready
  reject <id> --reason TEXT
                 send a task in review back to todo, with what needs to change
  approve <id> [--note TEXT] | request-changes <id> --reason TEXT
                 decide a review: the task stays in review, approved or with changes asked for, until an agent takes
                 it back (atm next offers it first, to the agent that worked on it for one lease length)
  block <id> --on user|agent (--question TEXT)... [--reason TEXT]
                 a task that can't go on until someone decides: blocked, its claim released
  unblock <id> [--answer TEXT]...
                 back to todo, with the questions and answers kept as a note
  reopen <id> --reason TEXT
                 a done or skipped task back to todo (done by mistake, or not finished after all)
  skip <id> --reason TEXT [--resolution wont_do|obsolete|duplicate] [--duplicate-of ID]
                 a todo or blocked task that isn't needed: skipped, with the reason as a note
  fail <id> [--reason TEXT]
                 put an in_progress task back to todo, recording why (a failure note)
  release <id> [--note TEXT]
                 put an in_progress task back to todo; the note is a handoff for the next agent
  note <id> TEXT add a note to a task (at most 500 characters)
  add --name N --type T [--sprint S] [--priority P] [--label L]... [--description D] [--dep ID]... [--file F]...
                 add a task with the next ID (in the backlog without --sprint); add --json-file F adds many
  edit <id> [--name] [--description] [--sprint S | --backlog] [--type] [--priority]
            [--add-dep/--remove-dep ID] [--add-file/--remove-file F] [--add-label/--remove-label L]
                 change a task's definition (its status changes only through the status commands)
  check          check tasks.json: schema, vocabularies, dependencies (exit code 1 on errors)
  archive [--sprint S]... [--before DATE] [--keep-days N] [--dry-run]
                 move finished tasks to tasks-archive.json, to keep tasks.json small
  unarchive <id>...
                 bring archived tasks back into tasks.json
  workflow [set direct|pull-request] [--base B] [--verify true|false] [--max-rounds N] ...
                 print the project's workflow (meta.workflow), or set it
  commit <id> --file F [--file F]... [--message TEXT]
                 commit exactly those files plus tasks.json (message from meta.workflow.commit_template),
                 and record the commit on the task
  pr <id>... --number N --url URL
                 record the PR that contains the tasks
  merged --pr N [--at TIME]
                 record that PR N was merged, on its tasks
  next [--sprint S | --tasks T-1,T-5 | --all | --backlog] [--type T] [--with-user] [--claim] [--json]
                 the task /atm:loop-tasks works on next: of the active sprint by default (every task without
                 sprints), tasks back from review first, then the most urgent priority, then lowest ID; --claim
                 also starts and claims it, atomically (exit code 3 if there is none)
  heartbeat <id> extend your claim's lease (heartbeat --auto: the plugin's hook renews your claims for you)
  run <id> [--every SECONDS] -- <command> [args...]
                 run a long command (tests, a build, a wait on CI) while keeping your claim on <id> alive; the
                 hook renews claims only between tool calls. Exits with the command's exit code
  init [--meta-file F]
                 give tasks.json a meta.id if it has none, and register the tasks.json merge driver in this
                 clone (migrate registers the driver too); --meta-file sets meta (project, vocabularies)
                 from a JSON object, creating tasks.json if there is none
  merge-driver <base> <ours> <theirs>
                 git merge driver for tasks.json (see .agentic-task-manager/.gitattributes)
  claims [--json]
                 list live claims, shared by all worktrees of this clone
  repository [set URL [--from OLD]]
                 print or set the repository's web address (meta.repository), for commit and PR links; a git
                 remote is turned into one; atm init fills it from origin. Setting a new one moves the tasks'
                 recorded PR links from the old address (--from, else the one it replaces)
  logo [set PATH | unset]
                 print, set or unset the project's logo for the dashboard (meta.logo: an SVG, PNG, WebP or ICO
                 file inside the project, at most 512 KB); without one, a logo.svg, .github/logo.png, ... is used
  sprints [--all] | sprints add ["<name>"] [--goal T] [--release R] [--due DATE] [--after S]
          | sprints edit <S> [--goal] [--release] [--due] | sprints start <S>
          | sprints close <S> [--carry-to <S2>|new [--name N]|backlog]
                 the sprints in order with their task counts (closed ones with --all); add one (a name is
                 generated without one), change one, make one active (one at a time), or close one: its
                 unfinished tasks go to --carry-to (needed when there are any), keeping their status, and it
                 keeps a report of what was done, carried and added mid-sprint
  releases [--all] | releases add <R> [--name N] [--target DATE] [--pr N]... [--released-at TIME]
          | releases edit <R> [--name] [--target] | releases archive <R> | releases notes <R>
                 the releases in order with their tasks by kind and stage (archived ones with --all); add one
                 (--pr gives it every task of those pull requests, --released-at records one that already shipped),
                 change one, archive it, or print a Markdown
                 draft of its changes (done tasks by type, with their outcome notes; what isn't done yet). A
                 sprint's or a task's release must be one of them
  components | components add <id> [--label T] [--path GLOB]... [--review MODE] [--from-label L] [--assign]
             | components edit <id> [--label T] [--add-path GLOB]... [--review MODE] [--from-label L] [--assign]
                 the components (meta.components) with their globs and task counts; add one, or change one:
                 --from-label moves a label onto it on every task (archived ones too) and drops the label,
                 --assign gives it to every task whose files its globs match
  move <id>... --to <sprint>|next|active|backlog
                 move to-do, blocked or in-review tasks into a sprint, the next planned one, the active one, or
                 the backlog (claimed and finished tasks stay where they are)
  backlog [--all]
                 the tasks with no sprint (with --all, finished ones too)
  projects [add [PATH] | remove <id|PATH> | prune]
                 the projects the dashboard shows (~/.agentic-task-manager/projects.json, or $ATM_HOME):
                 list them, add one, remove one, or drop the ones whose folder or tasks.json is gone
                 (status commands and heartbeat take --agent NAME and --json; status commands and next --claim
                 take --status ID to pick a status when the project has several of that kind)
  help           show this help

Every command takes --json, and `atm <command> --help` describes it.
"""


# Commands that don't check for an outdated dashboard server first: serve does it itself, the server process is the
# server, and git runs the merge driver in the middle of a merge
NO_REFRESH = ("serve", "_serve-child", "merge-driver", "_lock-test", "help", "version")


class Parser(argparse.ArgumentParser):
    """Usage errors as AtmError (exit code 2), so they are reported like every other error."""

    def error(self, message):
        raise AtmError(f"{message}\n{self.format_usage().strip()}", 2)


def cmd_version(args):
    out(to_json({"version": plugin_version()}) if args.json else plugin_version())


def cmd_help(args):
    if args.json:
        out(to_json({"usage": USAGE}))
    else:
        sys.stdout.write(USAGE)


def build_parser():
    parser = Parser(prog="atm", add_help=False)
    commands = parser.add_subparsers(dest="command", parser_class=Parser)

    def command(name, run, description, hidden=False):
        sub = commands.add_parser(name, description=description, help=argparse.SUPPRESS if hidden else description)
        sub.add_argument("--json", action="store_true", help="print the result as JSON")
        sub.set_defaults(run=run)
        return sub

    def agent_option(sub):
        sub.add_argument("--agent", metavar="NAME", help="who you are (default: ATM_AGENT, else this session)")

    def status_option(sub):
        sub.add_argument(
            "--status",
            metavar="ID",
            help="the status to move to when the project has several of that kind (default: the first listed)",
        )

    command("version", cmd_version, "print the plugin version")
    command("help", cmd_help, "show the list of commands")
    command("paths", cmd_paths, "print where this project's task files are")
    command("migrate", cmd_migrate, "move an old tasks.json into .agentic-task-manager/ and set the folder up")

    serve = command("serve", cmd_serve, "run the dashboard server, one for all your projects, in the background")
    serve.add_argument("action", nargs="?", default="start", choices=["start", "stop", "status"])
    serve.add_argument("--open", action="store_true", help="also open the dashboard in the browser")
    serve.add_argument("--all", action="store_true", help="show the overview of all projects, not this project")
    serve.add_argument(
        "--refresh",
        action="store_true",
        help="only restart a running server that has an older dashboard than this plugin (the SessionStart hook)",
    )
    serve.add_argument("--port", type=int, default=DEFAULT_PORT, help="the port to try first")
    serve.add_argument("--dir", metavar="PATH", help="the project to show (default: the current directory's)")

    for name, description in (
        ("start", "mark a task in_progress and claim it (its dependencies must be done)"),
        ("review", "ask for a review of a finished task: in_review, its claim released"),
        ("done", "mark an in_progress or in_review task done and drop its claim"),
        ("fail", "put an in_progress task back to todo, recording why"),
        ("release", "put an in_progress task back to todo"),
        ("reject", "send a task in review back to todo, recording why"),
        ("approve", "approve a task in review: it stays in review until an agent takes it back to finish"),
        ("request-changes", "ask for changes to a task in review: its agent takes it back first (a rejection note)"),
        ("block", "a task that can't go on until someone decides: blocked, with the questions; claim released"),
        ("unblock", "a blocked task goes back to todo, with the answers as a note"),
        ("reopen", "a done or skipped task goes back to todo, with the reason as a note"),
        ("skip", "a todo or blocked task that isn't needed: skipped, with the reason as a note"),
    ):
        sub = command(name, cmd_status_change(name), description)
        sub.add_argument("id", help="the task ID")
        agent_option(sub)
        status_option(sub)
        if name == "fail":
            sub.add_argument("--reason", metavar="TEXT", help="why it failed (kept as a failure note)")
        if name == "release":
            sub.add_argument("--note", metavar="TEXT", help="a handoff for whoever picks it up next")
        if name == "review":
            sub.add_argument("--note", metavar="TEXT", help="what the reviewer should look at")
        if name in ("reject", "request-changes"):
            sub.add_argument("--reason", metavar="TEXT", required=True, help="what needs to change (a rejection note)")
        if name == "approve":
            sub.add_argument("--note", metavar="TEXT", help="what is left to do, or a word for the agent")
        if name == "reopen":
            sub.add_argument("--reason", metavar="TEXT", required=True, help="why it isn't done after all (a note)")
        if name == "skip":
            sub.add_argument("--reason", metavar="TEXT", required=True, help="why it isn't needed (a note)")
            sub.add_argument(
                "--resolution", choices=["wont_do", "obsolete", "duplicate"], help="why it ended (default: wont_do)"
            )
            sub.add_argument("--duplicate-of", metavar="ID", help="the task it duplicates (with --resolution duplicate)")
        if name == "block":
            sub.add_argument("--on", required=True, choices=["user", "agent"], help="who has to decide")
            sub.add_argument("--question", action="append", default=[], metavar="TEXT", help="an open question")
            sub.add_argument("--reason", metavar="TEXT", help="what it waits for, when it isn't a question")
        if name == "unblock":
            sub.add_argument("--answer", action="append", default=[], metavar="TEXT", help="an answer, in order")
        if name == "done":
            sub.add_argument("--note", metavar="TEXT", help="a one-line outcome")
            sub.add_argument("--commit", metavar="SHA", help="the task's commit")
            sub.add_argument(
                "--resolution", choices=["done", "wont_do", "duplicate", "obsolete"], help="how it ended (default: done)"
            )

    next_ = command("next", cmd_next, "the task /atm:loop-tasks works on next (exit code 3 if there is none)")
    next_.add_argument("--type", help="only this task type")
    next_.add_argument(
        "--with-user", action="store_true", help="also tasks done together with the user (the user is watching)"
    )
    pick = next_.add_mutually_exclusive_group()
    pick.add_argument(
        "--sprint", action="append", metavar="NAME", help="this sprint's tasks (repeatable; default: the active one)"
    )
    pick.add_argument("--tasks", metavar="ID,ID", help="these tasks, in this order")
    pick.add_argument("--all", action="store_true", help="every task: the active sprint, the planned ones, the backlog")
    pick.add_argument("--backlog", action="store_true", help="the tasks with no sprint")
    next_.add_argument("--claim", action="store_true", help="also start and claim it, atomically")
    agent_option(next_)
    status_option(next_)

    show = command("show", cmd_show, "tasks in full: what an agent needs to work on them")
    show.add_argument("ids", nargs="+", metavar="id", help="task IDs")
    list_ = command("list", cmd_list, "one line per task: ID, status, priority, labels, name")
    list_.add_argument("--status", action="append", metavar="ID", help="only these statuses (repeatable)")
    list_.add_argument("--type", help="only this task type")
    list_.add_argument("--priority", help="only this priority (a missing one counts as the default)")
    list_.add_argument("--label", action="append", help="only tasks with this label (repeatable: all of them)")
    list_.add_argument("--agent", metavar="NAME", help="only tasks started by this agent")
    list_.add_argument("--ready", action="store_true", help="only to-do tasks whose dependencies are done")
    list_.add_argument("--blocked", action="store_true", help="only blocked tasks and to-do tasks still waiting")
    list_.add_argument("--in-pr", action="store_true", help="only tasks in a PR that isn't recorded as merged")
    list_.add_argument("--with-user", action="store_true", help="only tasks done together with the user (with: user)")
    list_.add_argument("--archived", action="store_true", help="list the archive (tasks-archive.json) instead")
    list_.add_argument(
        "--derived",
        action="store_true",
        help="each task in full with what atm derives for it (kind, ready, waiting for, effective review and release, "
        "stage), as JSON: what atm serve's api/v1/projects/<id>/tasks gives the dashboard",
    )
    list_.add_argument(
        "--tolerant",
        action="store_true",
        help="with --derived: read a malformed tasks.json as far as it can be read, as atm serve does for the "
        "dashboard, and print {meta, tasks, problems}",
    )
    status = command(
        "status",
        cmd_status,
        "the overview: counts, what is in progress, in review or blocked, the next task, the file's size",
    )

    command("summary", cmd_summary, "the project's numbers as atm serve gives them to the dashboard (JSON)")

    def scope_options(sub):
        sub.add_argument("--sprint", action="append", metavar="NAME", help="only this sprint's tasks (repeatable)")
        sub.add_argument("--backlog", action="store_true", help="only the tasks with no sprint")
        sub.add_argument("--release", metavar="R", help="only the tasks in this release (their own, or their sprint's)")
        sub.add_argument("--component", metavar="C", help="only the tasks in this component")

    scope_options(list_)
    scope_options(status)
    deps = command("deps", cmd_deps, "what a task waits for, and what waits for it")
    deps.add_argument("id", help="the task ID")

    add = command("add", cmd_add, "add a task with the next ID, or many from a JSON file")
    add.add_argument("--name", help="a short name (about 60 characters)")
    add.add_argument("--type", help="the task type (one of the project's types)")
    add.add_argument("--priority", help="the priority (default: none, which counts as the project's default)")
    add.add_argument("--label", action="append", default=[], help="a label (repeatable)")
    describe = add.add_mutually_exclusive_group()
    describe.add_argument("--description", default="", help="what exactly to do")
    describe.add_argument("--description-file", metavar="FILE", help="read the description from a file")
    add.add_argument("--dep", action="append", default=[], metavar="ID", help="a task it depends on (repeatable)")
    add.add_argument("--file", action="append", default=[], help="a file it will create or change (repeatable)")
    add.add_argument("--requirement", action="append", default=[], help="a requirement ID (repeatable)")
    add.add_argument("--sprint", metavar="NAME", help="the sprint it goes into (default: none, the backlog)")
    add.add_argument("--release", metavar="R", help="the release it ships in (default: its sprint's)")
    add.add_argument("--component", action="append", default=[], metavar="C", help="a component (repeatable)")
    add.add_argument(
        "--components", choices=["auto"], help="auto: the components its files are in (otherwise only suggested)"
    )
    add.add_argument(
        "--review", choices=list(REVIEW_MODES), help="how it is reviewed (default: its component's, else the project's)"
    )
    add.add_argument("--related", action="append", default=[], metavar="ID", help="a related task (repeatable)")
    add.add_argument(
        "--with", dest="with_", choices=["user"], help="user: done together with the user (unattended loops skip it)"
    )
    add.add_argument(
        "--json-file",
        metavar="FILE",
        help='add many: a JSON list of tasks; an "id" there is a placeholder (e.g. "new-1") other tasks\' '
        '"dependencies" can use, replaced by the real ID',
    )
    agent_option(add)

    edit = command("edit", cmd_edit, "change a task's definition (not its status: use the status commands)")
    edit.add_argument("id", help="the task ID")
    edit.add_argument("--name")
    edit_description = edit.add_mutually_exclusive_group()
    edit_description.add_argument("--description")
    edit_description.add_argument("--description-file", metavar="FILE")
    edit_sprint = edit.add_mutually_exclusive_group()
    edit_sprint.add_argument("--sprint", metavar="NAME", help="put it in this sprint (planned or active)")
    edit_sprint.add_argument("--backlog", action="store_true", help="take it out of its sprint, into the backlog")
    edit.add_argument("--type")
    edit.add_argument("--priority")
    edit.add_argument("--release", metavar="R", help="the release it ships in (empty: its sprint's)")
    edit.add_argument(
        "--review", choices=list(REVIEW_MODES) + [""], help="how it is reviewed (empty: its component's, else the project's)"
    )
    for option, help_text in (
        ("file", "a file it creates or changes"),
        ("dep", "a task it depends on"),
        ("label", "a label"),
        ("component", "a component"),
        ("related", "a related task"),
    ):
        edit.add_argument(f"--add-{option}", action="append", default=[], metavar="VALUE", help=f"add {help_text}")
        edit.add_argument(f"--remove-{option}", action="append", default=[], metavar="VALUE", help=f"remove {help_text}")
    edit.add_argument(
        "--components", choices=["auto"], help="auto: add the components its files are in (otherwise only suggested)"
    )
    edit.add_argument(
        "--with", dest="with_", choices=["user", ""], help="user: done together with the user (empty: not any more)"
    )
    agent_option(edit)

    check = command("check", cmd_check, "check tasks.json: the schema, the vocabularies, dependencies; exit 1 on errors")
    check.add_argument("--files", action="store_true", help="also warn about missing files listed by finished tasks")

    archive = command("archive", cmd_archive, "move finished tasks to tasks-archive.json, to keep tasks.json small")
    archive.add_argument("--sprint", action="append", metavar="NAME", help="only this sprint's tasks (repeatable)")
    archive.add_argument("--before", metavar="DATE", help="only tasks finished before this date (2026-05-01)")
    archive.add_argument(
        "--keep-days", type=float, help="keep tasks finished in the last N days (default: meta.archive.keep_days, 14)"
    )
    archive.add_argument("--dry-run", action="store_true", help="only say what would move")
    unarchive = command("unarchive", cmd_unarchive, "bring archived tasks back into tasks.json")
    unarchive.add_argument("ids", nargs="+", metavar="id", help="task IDs")

    workflow = command("workflow", cmd_workflow, "print the project's workflow, or set it (atm workflow set TYPE)")
    workflow.add_argument("action", nargs="?", choices=["set"], help="set: change the workflow")
    workflow.add_argument("type", nargs="?", help="direct or pull-request")
    workflow.add_argument("--base", help="the branch PRs target, or that direct pushes to")
    workflow.add_argument("--verify", choices=["true", "false"], help="run the project's verify before pushing")
    workflow.add_argument("--max-rounds", type=int, help="CI fix rounds before /atm:push stops")
    workflow.add_argument("--commit-template", metavar="TEXT", help='commit messages, e.g. "feat({id}): {name}"')
    workflow.add_argument(
        "--trailers",
        choices=["forbid", "require", "none"],
        help="forbid Co-Authored-By lines, require the --trailer lines, or leave it to the agent",
    )
    workflow.add_argument("--trailer", action="append", default=[], metavar="LINE", help="a required trailer line")

    commit = command("commit", cmd_commit, "commit exactly the named files plus tasks.json, and record the commit")
    commit.add_argument("id", help="the task ID")
    commit.add_argument(
        "--file", action="append", default=[], metavar="F", help="a file the task changed (repeat for each; required)"
    )
    commit.add_argument("--message", metavar="TEXT", help="the message (default: meta.workflow.commit_template)")
    commit.add_argument(
        "--record",
        metavar="REV",
        help="record this existing commit on the current branch instead of committing (e.g. after a rebase)",
    )
    agent_option(commit)

    pr = command("pr", cmd_pr, "record the PR that contains the tasks")
    pr.add_argument("ids", nargs="+", metavar="id", help="task IDs")
    pr.add_argument("--number", type=int, required=True, help="the PR number")
    pr.add_argument("--url", required=True, help="the PR's URL")
    agent_option(pr)

    merged = command("merged", cmd_merged, "record that a PR was merged, on the tasks it contains")
    merged.add_argument("--pr", type=int, required=True, metavar="N", help="the PR number")
    merged.add_argument("--at", metavar="TIME", help="when it was merged (ISO 8601 UTC; default: now)")
    agent_option(merged)

    note = command("note", cmd_note, "add a note to a task (at most 500 characters)")
    note.add_argument("id", help="the task ID")
    note.add_argument("text", help="the note")
    agent_option(note)

    heartbeat = command("heartbeat", cmd_heartbeat, "extend your claim's lease")
    heartbeat.add_argument("id", nargs="?", help="the task ID")
    heartbeat.add_argument(
        "--auto",
        action="store_true",
        help="renew your claims if their last renewal is ten minutes old; silent (the plugin's PostToolUse hook)",
    )
    agent_option(heartbeat)

    run = command("run", cmd_run, "run a long command while keeping your claim on a task alive")
    run.add_argument("id", help="the task ID")
    run.add_argument(
        "--every",
        type=float,
        metavar="SECONDS",
        help="renew this often (default: three times per lease, at least once a minute)",
    )
    agent_option(run)

    init = command("init", cmd_init, "give tasks.json a meta.id, and register the tasks.json merge driver")
    init.add_argument(
        "--meta-file",
        metavar="F",
        help="set tasks.json meta from a JSON object (project, vocabularies); creates tasks.json if needed",
    )
    merge = command("merge-driver", cmd_merge_driver, "git merge driver for tasks.json")
    for side in ("base", "ours", "theirs"):
        merge.add_argument(side)
    command("claims", cmd_claims, "list live claims, shared by all worktrees of this clone")
    repository = command("repository", cmd_repository, "print or set the repository's web address (meta.repository)")
    repository.add_argument("action", nargs="?", choices=["set"], help="set: change it")
    repository.add_argument("url", nargs="?", help="an http(s) URL or a git remote (git@host:owner/repo.git)")
    repository.add_argument(
        "--from",
        dest="old",
        metavar="OLD",
        help="with set: move recorded PR links from this address (default: the address being replaced)",
    )
    logo = command("logo", cmd_logo, "print, set or unset the project's logo for the dashboard (meta.logo)")
    logo.add_argument("action", nargs="?", choices=["set", "unset"], help="set or unset it")
    logo.add_argument("path", nargs="?", help="set: the image (SVG, PNG, WebP or ICO), relative to here")
    sprints = command("sprints", cmd_sprints, "the sprints with their task counts; add, edit, start or delete one")
    sprints.add_argument(
        "action", nargs="?", choices=["list", "add", "edit", "start", "close", "delete"], help="what to do"
    )
    sprints.add_argument("name", nargs="?", help="the sprint's name (add: generated when left out)")
    sprints.add_argument(
        "--carry-to", metavar="WHERE", help="close: where unfinished tasks go: a sprint's name, new, or backlog"
    )
    sprints.add_argument(
        "--name", dest="new_name", metavar="NAME", help="close --carry-to new: the new sprint's name (default: generated)"
    )
    sprints.add_argument(
        "--reviewed", action="store_true", help="close: its batch tasks were reviewed with the user (the sprint review)"
    )
    agent_option(sprints)
    sprints.add_argument("--all", action="store_true", help="list: the closed sprints too")
    sprints.add_argument("--goal", help="add, edit: what the sprint is for (empty: remove it)")
    sprints.add_argument("--release", help="add, edit: the release it delivers to (empty: remove it)")
    sprints.add_argument("--due", metavar="DATE", help="add, edit: the planned end, YYYY-MM-DD (empty: remove it)")
    sprints.add_argument("--after", metavar="NAME", help="add: put it after this sprint (default: last)")
    sprints.add_argument(
        "--move-to", metavar="WHERE", help="delete: where its tasks go: a sprint's name or backlog (default)"
    )
    sprints.add_argument(
        "--force", action="store_true", help="delete: also a sprint tasks finished in or moved on from"
    )
    move = command("move", cmd_move, "move tasks into a sprint, the next sprint, the active one, or the backlog")
    move.add_argument("ids", nargs="+", metavar="id", help="task IDs")
    move.add_argument(
        "--to", required=True, metavar="WHERE", help="a sprint's name, next, active, or backlog"
    )
    agent_option(move)
    backlog = command("backlog", cmd_backlog, "the tasks with no sprint (not finished ones, unless --all)")
    backlog.add_argument("--all", action="store_true", help="also the finished tasks with no sprint")
    releases = command("releases", cmd_releases, "the releases with their tasks; add, edit or archive one")
    releases.add_argument(
        "action", nargs="?", choices=["list", "add", "edit", "archive", "notes"], help="what to do"
    )
    releases.add_argument("id", nargs="?", help="the release's ID (letters, digits, '.', '-', '_')")
    releases.add_argument("--all", action="store_true", help="list: the archived releases too")
    releases.add_argument("--name", dest="title", help="add, edit: a name besides its ID (empty: remove it)")
    releases.add_argument("--target", metavar="DATE", help="add, edit: the planned date, YYYY-MM-DD (empty: remove it)")
    releases.add_argument(
        "--pr", type=int, action="append", metavar="N", help="add: give it the tasks of pull request N (repeatable)"
    )
    releases.add_argument(
        "--released-at", metavar="TIME", help="add: it shipped already, at this time (e.g. its PR's merge)"
    )
    components = command(
        "components", cmd_components, "the project's components; define or change one, from a label or by files"
    )
    components.add_argument("action", nargs="?", choices=["list", "add", "edit"], help="what to do")
    components.add_argument("id", nargs="?", help="the component's ID (letters, digits, '.', '-', '_')")
    components.add_argument(
        "--label", help="add, edit: the name shown (add: default the label's own text, with --from-label; edit: empty "
        "removes it)"
    )
    components.add_argument(
        "--path", action="append", metavar="GLOB", help="add: a glob over its files, e.g. 'dashboard/**' (repeatable)"
    )
    components.add_argument(
        "--add-path", action="append", metavar="GLOB", help="edit: add a glob over its files (repeatable)"
    )
    components.add_argument(
        "--review", help="add, edit: how its tasks are reviewed (blocker, async, batch, none; edit: empty removes it)"
    )
    components.add_argument(
        "--from-label", metavar="LABEL", help="add, edit: every task with this label gets the component instead"
    )
    components.add_argument(
        "--assign", action="store_true", help="add, edit: also give it to every task whose files its globs match"
    )
    projects = command("projects", cmd_projects, "the projects the dashboard shows: list, add, remove or prune them")
    projects.add_argument("action", nargs="?", default="list", choices=["list", "add", "remove", "prune"])
    projects.add_argument("target", nargs="?", help="add: the project's folder (default: here); remove: its ID or path")

    child = command("_serve-child", cmd_serve_child, "the detached server process", hidden=True)
    child.add_argument("home")
    child.add_argument("port", type=int)
    lock_test = command("_lock-test", cmd_lock_test, "test hook: hold the lock", hidden=True)
    lock_test.add_argument("seconds", type=float, nargs="?", default=0.0)
    return parser, commands.choices


def report(error, as_json):
    if as_json:
        sys.stderr.write(to_json({"error": str(error), "code": error.code}) + "\n")
    else:
        sys.stderr.write(f"atm: {error}\n")


def leading_dir(argv):
    """`atm --dir PATH <command> ...` (or `--dir=PATH`, T-351): the folder to run in, and the rest. Only before the
    command: `atm serve --dir PATH` keeps its own meaning."""
    if argv and argv[0] == "--dir":
        return (argv[1] if len(argv) > 1 else ""), argv[2:]
    if argv and argv[0].startswith("--dir="):
        return argv[0][len("--dir=") :], argv[1:]
    return None, argv


def main(argv):
    target, argv = leading_dir(argv)
    if target is not None:
        # As if started in that folder: the project is found from there, and relative paths count from there
        if not target or not os.path.isdir(target):
            report(AtmError(f"--dir: no folder '{target}'", 2), "--json" in argv)
            return 2
        os.chdir(target)
    if not argv or argv[0] in ("-h", "--help"):
        sys.stdout.write(USAGE)
        return 0
    parser, commands = build_parser()
    # `atm run <id> -- <command>`: what follows -- is the command to run, never atm's own options
    child = None
    if argv[0] == "run" and "--" in argv:
        split = argv.index("--")
        argv, child = argv[:split], argv[split + 1 :]
    as_json = "--json" in argv
    if argv[0] not in commands:
        if as_json:
            report(AtmError(f"unknown command '{argv[0]}'", 2), True)
        else:
            sys.stderr.write(f"atm: unknown command '{argv[0]}'\n" + USAGE)
        return 2
    try:
        args, extra = parser.parse_known_args(argv)
        args.child = child
        if extra:
            # Reported by the command's own parser, so the usage line is the command's
            commands[argv[0]].error(f"unrecognized arguments: {' '.join(extra)}")
        if argv[0] not in NO_REFRESH:
            try:
                refresh_server()  # after a plugin update, the first atm command brings the dashboard server along
            except Exception:
                pass  # never in the way of the command itself
        args.run(args)
    except AtmFailure as e:
        # atm-lib's errors and those of the modules from before the layers (AtmError is one of them)
        report(e, as_json)
        return e.code
    except AtmExit as e:
        return e.code
    except Exception as e:  # noqa: BLE001
        # A file atm didn't expect (a hostile or damaged one) is an error message, not a traceback: the skills and
        # the merge driver read stderr (docs/security-review.md F12)
        report(AtmError(f"unexpected error ({type(e).__name__}: {e}); run atm check, and report it if that is clean"), as_json)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
