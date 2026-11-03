"""A project's files for the document viewer (docs/compmd-viewer-design.md §4): which ones atm hands out, what each
is, and the CompMD document the dashboard renders for it. atm renders nothing itself: a code file becomes a fenced
block of its language, an image a Markdown image, and CompMD does the rest.

The rules are here; the reading is the version control's (the project's working copy: vcs `file_stamp`, `read_file`,
`ignored`, `tracked`, `head`).
"""

import os
import re
from datetime import datetime, timezone

from ..common.errors import Invalid, NotFound, Refused, TooLarge

DOC_MAX_PATH = 512
DOC_MAX_BYTES = 1_000_000  # a document, code or text
DOC_MAX_IMAGE_BYTES = 5_000_000
# How much of a file says whether it is text
DOC_SNIFF_BYTES = 8192

DOC_MARKDOWN = (".md", ".markdown")  # .comp.md ends in .md
DOC_IMAGES = {
    ".png": "image/png",
    ".jpg": "image/jpeg",
    ".jpeg": "image/jpeg",
    ".gif": "image/gif",
    ".webp": "image/webp",
    ".svg": "image/svg+xml",
}
# A code file's fence language, by extension (or by whole name, for files without one)
DOC_LANGUAGES = {
    ".py": "python",
    ".pyi": "python",
    ".js": "javascript",
    ".mjs": "javascript",
    ".cjs": "javascript",
    ".jsx": "jsx",
    ".ts": "ts",
    ".mts": "ts",
    ".cts": "ts",
    ".tsx": "tsx",
    ".svelte": "svelte",
    ".vue": "vue",
    ".json": "json",
    ".jsonc": "jsonc",
    ".yaml": "yaml",
    ".yml": "yaml",
    ".toml": "toml",
    ".ini": "ini",
    ".cfg": "ini",
    ".xml": "xml",
    ".html": "html",
    ".htm": "html",
    ".css": "css",
    ".scss": "scss",
    ".sass": "sass",
    ".less": "less",
    ".sh": "sh",
    ".bash": "bash",
    ".zsh": "zsh",
    ".fish": "fish",
    ".ps1": "powershell",
    ".bat": "bat",
    ".rs": "rust",
    ".go": "go",
    ".java": "java",
    ".kt": "kotlin",
    ".kts": "kotlin",
    ".scala": "scala",
    ".groovy": "groovy",
    ".gradle": "groovy",
    ".c": "c",
    ".h": "c",
    ".cc": "cpp",
    ".cpp": "cpp",
    ".cxx": "cpp",
    ".hpp": "cpp",
    ".cs": "csharp",
    ".fs": "fsharp",
    ".swift": "swift",
    ".m": "objectivec",
    ".rb": "ruby",
    ".php": "php",
    ".pl": "perl",
    ".lua": "lua",
    ".r": "r",
    ".jl": "julia",
    ".dart": "dart",
    ".ex": "elixir",
    ".exs": "elixir",
    ".erl": "erlang",
    ".hs": "haskell",
    ".clj": "clojure",
    ".ml": "ocaml",
    ".zig": "zig",
    ".nim": "nim",
    ".sql": "sql",
    ".graphql": "graphql",
    ".proto": "protobuf",
    ".tf": "hcl",
    ".hcl": "hcl",
    ".mermaid": "mermaid",
    ".mmd": "mermaid",
    ".diff": "diff",
    ".patch": "diff",
    "Dockerfile": "dockerfile",
    "Makefile": "makefile",
}
# Files that hold keys, refused whatever git says (§4b)
DOC_KEY_FILES = re.compile(r"(\.(pem|key|p12|pfx|keystore|jks)|^id_(rsa|dsa|ecdsa|ed25519)(\..*)?)$", re.I)
# The start of each image type, so a file named .png is one
DOC_IMAGE_MAGIC = {
    "image/png": (b"\x89PNG\r\n\x1a\n",),
    "image/jpeg": (b"\xff\xd8\xff",),
    "image/gif": (b"GIF87a", b"GIF89a"),
}


def document_path(path):
    """The path a request names, checked (§4b): relative to the project's root, `/`-separated, normalized (no `..`,
    `.` or empty segment), no hidden segment (`.git`, `.env`, `.agentic-task-manager`), no NUL or backslash, at most
    DOC_MAX_PATH characters. Invalid otherwise; Refused for a hidden or key file."""
    if not isinstance(path, str) or not path or len(path) > DOC_MAX_PATH:
        raise Invalid("a file path, relative to the project, of at most 512 characters")
    if "\0" in path or "\\" in path or path.startswith("/"):
        raise Invalid(f"{path}: a relative path with / between its parts")
    segments = path.split("/")
    if any(s in ("", ".", "..") for s in segments):
        raise Invalid(f"{path}: a path inside the project, without . or .. parts")
    if any(s.startswith(".") for s in segments):
        raise Refused(f"{path}: hidden files and folders aren't shown")
    if DOC_KEY_FILES.search(segments[-1]):
        raise Refused(f"{path}: key files aren't shown")
    return path


def document_kind(path):
    """What a file is by its name: (kind, language or content type). `markdown`, `image` (with its content type),
    `code` (with its fence language), or `text`; `binary` is found only by reading it."""
    name = path.rsplit("/", 1)[-1]
    extension = os.path.splitext(name)[1].lower()
    if extension in DOC_MARKDOWN:
        return "markdown", None
    if extension in DOC_IMAGES:
        return "image", DOC_IMAGES[extension]
    language = DOC_LANGUAGES.get(extension) or DOC_LANGUAGES.get(name)
    return ("code", language) if language else ("text", None)


def _checked(project, path):
    """(path, kind, language or content type, stamp) of a file the viewer may show: its path checked, a regular file
    reached through no symlink, not ignored by version control, not too large. NotFound, Refused or TooLarge."""
    path = document_path(path)
    vcs = project.vcs
    if vcs is None:
        raise NotFound("this project has no working copy to read files from")
    stamp = vcs.file_stamp(path)
    if stamp is None:
        raise NotFound(f"{path}: not in the project")
    if vcs.ignored(path):
        raise Refused(f"{path}: git ignores it, so it isn't shown")
    kind, detail = document_kind(path)
    limit = DOC_MAX_IMAGE_BYTES if kind == "image" else DOC_MAX_BYTES
    if stamp[0] > limit:
        raise TooLarge(f"{path}: larger than {limit // 1_000_000} MB")
    return path, kind, detail, stamp


def document_stamp(project, path):
    """(size, nanosecond mtime) of a file the viewer may show, after the same checks as read_document: for the
    REST API's ETag, so an unchanged file is answered without reading it."""
    return _checked(project, path)[3]


def _fence(text, language):
    """`text` as a fenced code block of `language`, the fence longer than any run of backticks in it."""
    longest = max((len(run) for run in re.findall(r"`+", text)), default=0)
    fence = "`" * max(3, longest + 1)
    body = text if text.endswith("\n") or not text else text + "\n"
    return f"{fence}{language or ''}\n{body}{fence}\n"


def _is_binary(data):
    """A NUL in its first bytes, or bytes that aren't UTF-8."""
    if b"\0" in data[:DOC_SNIFF_BYTES]:
        return True
    try:
        data.decode("utf-8")
    except UnicodeDecodeError:
        return True
    return False


def _image_matches(data, content_type):
    if content_type == "image/svg+xml":
        return b"<svg" in data[:DOC_SNIFF_BYTES].lower()
    if content_type == "image/webp":
        return data[:4] == b"RIFF" and data[8:12] == b"WEBP"
    return data.startswith(DOC_IMAGE_MAGIC[content_type])


def _link_target(name):
    """A file name as a Markdown link destination, in angle brackets when it has spaces or parentheses."""
    return f"<{name}>" if re.search(r"[\s()<>]", name) else name


def read_document(project, path):
    """A file of the project for the viewer (§4a): {path, kind, language, text, document, line_offset, lines, bytes,
    modified_at, tracked, commit}. `document` is the CompMD document to render: a Markdown file as it is, code and
    text in a fence (`line_offset` 1: the fence's first line comes before the file's), an image as a Markdown image
    of its own name (relative, which the dashboard points at the files route), a binary file as a line saying so.
    `text` is the file as it is (None for an image or a binary file); `lines` counts its lines. `tracked` and
    `commit` (the current commit) are None outside version control. NotFound, Refused or TooLarge."""
    path, kind, detail, stamp = _checked(project, path)
    data = project.vcs.read_file(path)
    name = path.rsplit("/", 1)[-1]
    text = None
    line_offset = 0
    if kind == "image":
        if not _image_matches(data, detail):
            kind = "binary"
            document = f"*{name} isn't the image its name says: {len(data)} bytes.*\n"
        else:
            document = f"![{name}]({_link_target(name)})\n"
    elif _is_binary(data):
        kind, detail = "binary", None
        document = f"*{name} is a binary file: {len(data)} bytes.*\n"
    else:
        text = data.decode("utf-8")
        if kind == "markdown":
            document = text
        else:
            document = _fence(text, detail)
            line_offset = 1
    lines = 0 if text is None else len(text.splitlines())
    modified = datetime.fromtimestamp(stamp[1] / 1e9, timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    return {
        "path": path,
        "kind": kind,
        "language": detail if kind == "code" else None,
        "text": text,
        "document": document,
        "line_offset": line_offset,
        "lines": lines,
        "bytes": len(data),
        "modified_at": modified,
        "tracked": project.vcs.tracked(path),
        "commit": project.vcs.head(),
    }


def read_image(project, path):
    """(bytes, content type) of an image file of the project, for an <img> (the files route): only the image types
    the viewer shows, whose bytes match their name. Refused for any other file, as well as read_document's errors."""
    path, kind, content_type, _ = _checked(project, path)
    if kind != "image":
        raise Refused(f"{path}: only images are served as files")
    data = project.vcs.read_file(path)
    if not _image_matches(data, content_type):
        raise Refused(f"{path}: isn't the image its name says")
    return data, content_type
