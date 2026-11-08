"""atm-lib: installing atm's skills into the agent harnesses (T-442, docs/better-way-to-distribute-atm.md §3, §5).

The skills of this atm's version (common/skills.py, generated into .generated/) are written once per harness into the user's atm folder
(`<ATM_HOME>/skills/<harness>/<skill>/SKILL.md`) and linked from the folder each harness reads its skills from; copied
there instead with `copy`, where a link can't be made, and always for a project (whose copy is committed for a team, so
it can't point into one person's home folder). What install wrote is recorded (`<ATM_HOME>/agents.json`), so uninstall
removes exactly that and status can say what is missing or stale. A folder atm didn't write is never touched.

Harnesses are a table: Claude Code for now; another (Codex, Cursor, … reading ~/.agents/skills) is one more entry
(T-413). Never run by atm on its own: only `atm agents install` and the app's button (which call this) do."""

import os
import shutil

from ..common.core import atm_home, ensure_home, read_json, write_json_atomic
from ..common.errors import Invalid
from ..common.skills import CLAUDE_CODE_ALLOWED_TOOLS, SKILLS

RECORD = "agents.json"

HARNESSES = {
    "claude": {
        "label": "Claude Code",
        # Where it is installed (atm says so; installing doesn't need it)
        "detect": "~/.claude",
        # Where it reads skills: the user's, and a project's (relative to its root)
        "user_dir": "~/.claude/skills",
        "project_dir": ".claude/skills",
        # Frontmatter it reads beyond the portable core
        "allowed_tools": True,
        # It follows symlinked skill folders
        "links": True,
    },
}


def project_folder(project):
    """The project's folder, for `--project`: its working copy (where its task folder is)."""
    return project.store.working_copy()


def harness_ids(harness=None):
    """The harnesses to act on: one by its ID, or all of them."""
    if harness is None:
        return list(HARNESSES)
    if harness not in HARNESSES:
        raise Invalid(f"no harness '{harness}' (atm knows: {', '.join(HARNESSES)})")
    return [harness]


def skill_text(harness, name):
    """A skill's SKILL.md as this harness gets it: the portable core, with Claude Code's pre-approved commands added to
    its frontmatter for Claude Code."""
    text = SKILLS[name]
    tools = CLAUDE_CODE_ALLOWED_TOOLS.get(name)
    if HARNESSES[harness]["allowed_tools"] and tools:
        head, sep, body = text.partition("\n---\n")
        text = f"{head}\nallowed-tools: {tools}{sep}{body}"
    return text


def _home_dir(path):
    return os.path.abspath(os.path.expanduser(path))


def _record_path():
    return os.path.join(atm_home(), RECORD)


def _read_record():
    data = read_json(_record_path()) if os.path.exists(_record_path()) else None
    entries = data.get("installed") if isinstance(data, dict) else None
    return [e for e in entries or [] if isinstance(e, dict) and isinstance(e.get("path"), str)]


def _write_record(entries):
    ensure_home(atm_home())
    write_json_atomic(_record_path(), {"installed": sorted(entries, key=lambda e: e["path"])})


def _write_skill(folder, text):
    os.makedirs(folder, exist_ok=True)
    with open(os.path.join(folder, "SKILL.md"), "w", encoding="utf-8") as f:
        f.write(text)


def _read_skill(folder):
    try:
        with open(os.path.join(folder, "SKILL.md"), encoding="utf-8") as f:
            return f.read()
    except OSError:
        return None


def _targets(harness, project_dir):
    """Where a harness reads skills: (the folder, the scope), the user's or the project's."""
    spec = HARNESSES[harness]
    if project_dir is not None:
        return os.path.join(os.path.abspath(project_dir), spec["project_dir"]), "project"
    return _home_dir(spec["user_dir"]), "user"


def install_skills(harness=None, project_dir=None, copy=False, version=None):
    """Write the skills for the harnesses (one, or all) and link or copy them where each reads them: the user's
    folder, or `project_dir`'s with a project. Idempotent. Returns {written: [{harness, skill, path, how, action}],
    refused: [{path, reason}]}: `action` is installed, updated or unchanged; a folder in the way that atm didn't
    write is refused, not replaced."""
    record = {e["path"]: e for e in _read_record()}
    written, refused = [], []
    for h in harness_ids(harness):
        target, scope = _targets(h, project_dir)
        source_root = os.path.join(atm_home(), "skills", h)
        for name in sorted(SKILLS):
            text = skill_text(h, name)
            source = os.path.join(source_root, name)
            if scope == "user" and _read_skill(source) != text:
                _write_skill(source, text)
            path = os.path.join(target, name)
            how = "copy" if copy or scope == "project" or not HARNESSES[h]["links"] else "link"
            ours = path in record
            exists = os.path.lexists(path)
            if exists and not ours:
                refused.append({"path": path, "reason": "is there already, and atm didn't put it there"})
                continue
            if how == "link" and os.path.islink(path) and os.path.realpath(path) == os.path.realpath(source):
                action = "unchanged"
            elif how == "copy" and not os.path.islink(path) and _read_skill(path) == text:
                action = "unchanged"
            else:
                action = "updated" if exists else "installed"
                _remove(path)
                os.makedirs(target, exist_ok=True)
                if how == "link":
                    try:
                        os.symlink(source, path, target_is_directory=True)
                    except OSError:
                        how = "copy"  # no links here (a file system or platform without them): a copy does
                if how == "copy":
                    _write_skill(path, text)
            record[path] = {"path": path, "harness": h, "skill": name, "how": how, "scope": scope, "version": version}
            written.append({"harness": h, "skill": name, "path": path, "how": how, "action": action})
    _write_record(record.values())
    return {"written": written, "refused": refused}


def _remove(path):
    if os.path.islink(path) or os.path.isfile(path):
        os.remove(path)
    elif os.path.isdir(path):
        shutil.rmtree(path)


def uninstall_skills(harness=None, project_dir=None):
    """Remove what install wrote for the harnesses (and, with a project, only that project's): the links or copies,
    and the user's copies in atm's folder. Returns the paths removed."""
    wanted = set(harness_ids(harness))
    project = os.path.abspath(project_dir) if project_dir is not None else None
    removed, kept = [], []
    for entry in _read_record():
        in_scope = entry.get("harness") in wanted and (
            entry.get("scope") == "user" if project is None else entry["path"].startswith(project + os.sep)
        )
        if not in_scope:
            kept.append(entry)
            continue
        if os.path.lexists(entry["path"]):
            _remove(entry["path"])
            removed.append(entry["path"])
    if project is None:
        for h in wanted:
            source_root = os.path.join(atm_home(), "skills", h)
            if os.path.isdir(source_root):
                shutil.rmtree(source_root)
    _write_record(kept)
    return removed


def skills_status(project_dir=None, version=None):
    """Per harness: whether it is installed here, and each skill's state where it reads them: linked, copied, missing,
    or stale (a copy of another version, or a link whose skill is gone). `project_dir` adds that project's."""
    record = {e["path"]: e for e in _read_record()}
    report = []
    for h, spec in HARNESSES.items():
        scopes = [(None, "user")] + ([(project_dir, "project")] if project_dir is not None else [])
        for where, scope in scopes:
            target, _ = _targets(h, where)
            skills = []
            for name in sorted(SKILLS):
                path = os.path.join(target, name)
                entry = record.get(path)
                if not os.path.lexists(path):
                    state = "missing"
                elif entry is None:
                    state = "not atm's"
                elif _read_skill(path) != skill_text(h, name):
                    state = "stale"
                else:
                    state = "linked" if os.path.islink(path) else "copied"
                skills.append({"skill": name, "path": path, "state": state, "version": (entry or {}).get("version")})
            report.append(
                {
                    "harness": h,
                    "label": spec["label"],
                    "detected": os.path.isdir(_home_dir(spec["detect"])),
                    "scope": scope,
                    "folder": target,
                    "version": version,
                    "skills": skills,
                }
            )
    return report
