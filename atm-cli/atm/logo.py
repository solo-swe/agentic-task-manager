"""`atm logo`: the project's logo for the dashboard, in meta.logo (a path relative to the project root), or else one
found in the usual places. atm serve serves it at /p/<id>/logo (docs/multi-project-api.md)."""

import os

from .common.core import AtmError, out, to_json, write_json_atomic
from .lock import locked
from .db.jsongit.files import load_tasks
from .paths import find_paths

LOGO_TYPES = {
    ".svg": "image/svg+xml",
    ".png": "image/png",
    ".webp": "image/webp",
    ".ico": "image/x-icon",
}
MAX_LOGO_BYTES = 512 * 1024
# Where a logo usually is, first match wins: logo.svg/logo.png in the root, then .github/logo.*, public/favicon.* and
# assets/logo.*, each in the order of LOGO_TYPES
LOGO_CANDIDATES = ["logo.svg", "logo.png"] + [
    f"{base}{ext}" for base in (".github/logo", "public/favicon", "assets/logo") for ext in LOGO_TYPES
]


def logo_file(root, relative):
    """(absolute path, content type) of the image at `relative` in the project at `root`. AtmError (exit code 2) if
    it isn't a supported image type, is outside the project (symbolic links followed), is missing or too large."""
    if not isinstance(relative, str) or not relative.strip():
        raise AtmError("no logo path given", 2)
    content_type = LOGO_TYPES.get(os.path.splitext(relative)[1].lower())
    if not content_type:
        raise AtmError(f"{relative}: a logo is an SVG, PNG, WebP or ICO image", 2)
    real_root = os.path.realpath(root)
    full = os.path.realpath(os.path.join(real_root, relative))
    if os.path.commonpath([real_root, full]) != real_root:
        raise AtmError(f"{relative}: the logo must be inside the project folder", 2)
    if not os.path.isfile(full):
        raise AtmError(f"{relative}: no such file", 2)
    if os.path.getsize(full) > MAX_LOGO_BYTES:
        raise AtmError(f"{relative}: the logo is larger than {MAX_LOGO_BYTES // 1024} KB", 2)
    return full, content_type


def find_logo(root, meta):
    """(path relative to root, absolute path, content type, how it was found: "set" or "found") of the project's
    logo, or None. meta.logo wins when it is set (a logo it names that is unusable means no logo); otherwise the first
    usable LOGO_CANDIDATES file."""
    if meta.get("logo"):
        try:
            return (meta["logo"], *logo_file(root, meta["logo"]), "set")
        except AtmError:
            return None
    for candidate in LOGO_CANDIDATES:
        try:
            return (candidate, *logo_file(root, candidate), "found")
        except AtmError:
            continue
    return None


def cmd_logo(args):
    paths = find_paths(os.getcwd())
    root = paths["root"]
    if args.action is None:
        if args.path is not None:
            raise AtmError(f"unexpected argument '{args.path}' (atm logo set PATH)", 2)
        meta = load_tasks(paths).get("meta", {})
        logo = find_logo(root, meta)
        if args.json:
            found = {"logo": logo[0], "source": logo[3]} if logo else {"logo": None, "source": None}
            out(to_json({**found, "set": meta.get("logo")}))
        elif logo:
            out(f"{logo[0]}" + (" (found; set one with `atm logo set <path>`)" if logo[3] == "found" else ""))
        elif meta.get("logo"):
            out(f"meta.logo is {meta['logo']}, which isn't a usable logo (missing, too large, or outside the project).")
        else:
            out("No logo. Set one with `atm logo set <path>` (SVG, PNG, WebP or ICO, inside the project).")
        return
    if args.action == "unset":
        with locked(paths["dir"]):
            data = load_tasks(paths)
            removed = data.get("meta", {}).pop("logo", None)
            if removed is not None:
                write_json_atomic(paths["tasks"], data)
        out(to_json({"logo": None}) if args.json else "Logo unset." if removed else "No logo was set.")
        return
    if args.path is None:
        raise AtmError("atm logo set needs a path", 2)
    # The path is relative to where atm runs, like every path atm takes; it is kept relative to the project root
    relative = os.path.relpath(os.path.abspath(args.path), os.path.realpath(root)).replace(os.sep, "/")
    logo_file(root, relative)
    with locked(paths["dir"]):
        data = load_tasks(paths)
        data.setdefault("meta", {})["logo"] = relative
        write_json_atomic(paths["tasks"], data)
    out(to_json({"logo": relative}) if args.json else f"Logo set: {relative}")
