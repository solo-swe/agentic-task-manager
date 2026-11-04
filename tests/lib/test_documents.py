"""atm-lib's project files for the document viewer (lib/documents.py, docs/compmd-viewer-design.md §4): which files
it hands out, what each is, and the CompMD document it wraps it in, with a fake version control holding the files:
atm-lib reads the working copy only through the vcs interface (docs/layered-architecture.md §2)."""

import unittest

import support  # noqa: F401 - puts atm-cli on the path

from atm.common.errors import Invalid, NotFound, Refused, TooLarge
from atm.db.memory import MemoryStore
from atm.lib.documents import DOC_MAX_BYTES, document_kind, document_path, document_stamp, read_document, read_image
from atm.lib.project import Project

PNG = b"\x89PNG\r\n\x1a\n" + b"\0" * 20
SHA = "a" * 40


class FakeVcs:
    """The working copy's files in a dict; `ignored` and `tracked` sets of paths; `repository` False for a project
    outside version control."""

    def __init__(self, files, ignored=(), tracked=(), repository=True, submodules=None):
        self.files = files
        self.ignored_paths = set(ignored)
        self.tracked_paths = set(tracked)
        self.repository = repository
        # A top-level folder that is a submodule: what submodule_of gives for its files
        self.submodules = submodules or {}

    def file_stamp(self, relative):
        data = self.files.get(relative)
        return None if data is None else (len(data), 1_793_700_000_000_000_000)

    def read_file(self, relative):
        if relative not in self.files:
            raise NotFound(relative)
        return self.files[relative]

    def ignored(self, relative):
        return self.repository and relative in self.ignored_paths

    def tracked(self, relative):
        return (relative in self.tracked_paths) if self.repository else None

    def head(self):
        return SHA if self.repository else None

    def submodule_of(self, relative):
        return self.submodules.get(relative.split("/", 1)[0]) if self.repository else None


def project(files, **vcs):
    return Project(MemoryStore(), FakeVcs(files, **vcs))


class Paths(unittest.TestCase):
    def test_a_relative_normalized_path(self):
        self.assertEqual(document_path("docs/x.md"), "docs/x.md")
        for bad in ("", "/etc/passwd", "../x.md", "docs/../x.md", "docs//x.md", "./x.md", "docs\\x.md", "a\0b", "x" * 513):
            with self.assertRaises(Invalid, msg=bad):
                document_path(bad)

    def test_hidden_files_and_key_files_are_refused(self):
        for refused in (".env", ".git/config", "docs/.secret.md", ".agentic-task-manager/tasks.json", "certs/server.pem",
                        "deploy/id_rsa", "deploy/id_ed25519.pub", "a.KEY", "store.p12", "app.keystore"):
            with self.assertRaises(Refused, msg=refused):
                document_path(refused)
        # Names that only look alike are fine
        for fine in ("docs/keys.md", "src/monkey.ts", "id_rsa_notes.md"):
            self.assertEqual(document_path(fine), fine)


class Kinds(unittest.TestCase):
    def test_by_name(self):
        self.assertEqual(document_kind("docs/x.md"), ("markdown", None))
        self.assertEqual(document_kind("docs/x.comp.md"), ("markdown", None))
        self.assertEqual(document_kind("README.markdown"), ("markdown", None))
        self.assertEqual(document_kind("src/app.ts"), ("code", "ts"))
        self.assertEqual(document_kind("atm/cli/serve.PY"), ("code", "python"))
        self.assertEqual(document_kind("Dockerfile"), ("code", "dockerfile"))
        self.assertEqual(document_kind("logo.SVG"), ("image", "image/svg+xml"))
        self.assertEqual(document_kind("notes.txt"), ("text", None))
        self.assertEqual(document_kind("LICENSE"), ("text", None))


class Documents(unittest.TestCase):
    def test_markdown_as_it_is(self):
        text = "---\nmd:\n  version: 1\n---\n# Title\n\nBody.\n"
        doc = read_document(project({"docs/x.md": text.encode()}, tracked={"docs/x.md"}), "docs/x.md")
        self.assertEqual(doc["kind"], "markdown")
        self.assertEqual(doc["document"], text)
        self.assertEqual(doc["text"], text)
        self.assertEqual(doc["line_offset"], 0)
        self.assertEqual(doc["lines"], 7)
        self.assertEqual(doc["tracked"], True)
        self.assertEqual(doc["commit"], SHA)
        self.assertEqual(doc["modified_at"], "2026-11-03T10:00:00Z")

    def test_code_in_a_fence_of_its_language_longer_than_its_backticks(self):
        text = "const s = `x`;\n// ```js inside\n"
        doc = read_document(project({"src/app.ts": text.encode()}), "src/app.ts")
        self.assertEqual(doc["kind"], "code")
        self.assertEqual(doc["language"], "ts")
        self.assertEqual(doc["document"], "````ts\n" + text + "````\n")
        self.assertEqual(doc["line_offset"], 1)
        self.assertEqual(doc["lines"], 2)
        self.assertEqual(doc["tracked"], False)

    def test_text_in_a_plain_fence_its_last_line_closed(self):
        doc = read_document(project({"NOTES": b"one\ntwo"}), "NOTES")
        self.assertEqual((doc["kind"], doc["language"]), ("text", None))
        self.assertEqual(doc["document"], "```\none\ntwo\n```\n")
        self.assertEqual(doc["lines"], 2)

    def test_binary_says_so(self):
        for data in (b"\x00\x01\x02", b"\xff\xfe not utf-8"):
            doc = read_document(project({"build/app.bin": data}), "build/app.bin")
            self.assertEqual(doc["kind"], "binary")
            self.assertIsNone(doc["text"])
            self.assertEqual(doc["lines"], 0)
            self.assertIn("binary file", doc["document"])

    def test_an_image_is_a_markdown_image_of_its_own_name(self):
        doc = read_document(project({"docs/img/a diagram.png": PNG}), "docs/img/a diagram.png")
        self.assertEqual(doc["kind"], "image")
        self.assertEqual(doc["document"], "![a diagram.png](<a diagram.png>)\n")
        svg = read_document(project({"logo.svg": b'<?xml version="1.0"?><svg xmlns="x"/>'}), "logo.svg")
        self.assertEqual(svg["document"], "![logo.svg](logo.svg)\n")
        # Named .png, but isn't one: shown as a binary file, never as an image
        fake = read_document(project({"x.png": b"<script>alert(1)</script>"}), "x.png")
        self.assertEqual(fake["kind"], "binary")

    def test_outside_version_control(self):
        doc = read_document(project({"x.md": b"# x\n"}, repository=False), "x.md")
        self.assertIsNone(doc["tracked"])
        self.assertIsNone(doc["commit"])
        self.assertIsNone(doc["submodule"])

    def test_a_file_in_a_submodule_says_which_and_where_it_lives(self):
        sub = {"path": "vendor/lib", "commit": "b" * 40, "repository": "https://github.com/o/lib"}
        files = {"vendor/lib/README.md": b"# lib\n", "README.md": b"# top\n"}
        p = project(files, submodules={"vendor": sub})
        self.assertEqual(read_document(p, "vendor/lib/README.md")["submodule"], sub)
        self.assertIsNone(read_document(p, "README.md")["submodule"])


class Refusals(unittest.TestCase):
    def test_missing_ignored_and_too_large(self):
        with self.assertRaises(NotFound):
            read_document(project({}), "docs/x.md")
        with self.assertRaises(Refused):
            read_document(project({"config/credentials.json": b"{}"}, ignored={"config/credentials.json"}), "config/credentials.json")
        with self.assertRaises(TooLarge):
            read_document(project({"big.md": b"x" * (DOC_MAX_BYTES + 1)}), "big.md")
        # An image may be larger than a document
        self.assertEqual(read_image(project({"big.png": PNG + b"\0" * DOC_MAX_BYTES}), "big.png")[1], "image/png")

    def test_outside_version_control_nothing_is_ignored(self):
        doc = read_document(project({"x.md": b"x"}, ignored={"x.md"}, repository=False), "x.md")
        self.assertEqual(doc["kind"], "markdown")

    def test_without_a_working_copy(self):
        with self.assertRaises(NotFound):
            read_document(Project(MemoryStore(), None), "x.md")

    def test_the_stamp_checks_the_same(self):
        self.assertEqual(document_stamp(project({"x.md": b"abc"}), "x.md"), (3, 1_793_700_000_000_000_000))
        with self.assertRaises(Refused):
            document_stamp(project({"x.md": b"abc"}, ignored={"x.md"}), "x.md")


class Images(unittest.TestCase):
    def test_only_images_whose_bytes_match(self):
        self.assertEqual(read_image(project({"a.png": PNG}), "a.png"), (PNG, "image/png"))
        with self.assertRaises(Refused):
            read_image(project({"a.md": b"# x"}), "a.md")
        with self.assertRaises(Refused):
            read_image(project({"a.gif": b"<svg onload=alert(1)>"}), "a.gif")


if __name__ == "__main__":
    unittest.main()
