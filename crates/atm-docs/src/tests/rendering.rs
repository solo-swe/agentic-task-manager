//! Rendering a file for the viewer: Markdown as it is, code and text in a fence, an image as itself, anything else
//! as binary.

use crate::*;

#[test]
fn a_fence_holds_any_text_with_its_own_last_line_end() {
    for (text, language, fenced) in [
        ("", None, "```\n```\n"),
        ("x", Some("rust"), "```rust\nx\n```\n"),
        ("x\n", None, "```\nx\n```\n"),
        // Runs of backticks shorter than three keep the usual fence; three or more get one longer
        ("a `` b\n", None, "```\na `` b\n```\n"),
        ("```\n", None, "````\n```\n````\n"),
        ("` `` ```", None, "````\n` `` ```\n````\n"),
    ] {
        assert_eq!(fence(text, language), fenced, "{text:?}");
    }
}

#[test]
fn an_image_matches_only_the_bytes_its_type_starts_with() {
    let mut far_svg = vec![b' '; DOC_SNIFF_BYTES];
    far_svg.extend_from_slice(b"<svg>");
    let cases: [(&[u8], &str, bool); 15] = [
        (b"\x89PNG\r\n\x1a\nrest", "image/png", true),
        (b"\x89PNG", "image/png", false),
        (b"\xff\xd8\xff\xe0", "image/jpeg", true),
        (b"\xff\xd8", "image/jpeg", false),
        (b"GIF87a", "image/gif", true),
        (b"GIF88a", "image/gif", false),
        (b"RIFF\0\0\0\0WEBPVP8 ", "image/webp", true),
        (b"RIFF\0\0\0\0WAVE", "image/webp", false),
        (b"RIFF", "image/webp", false),
        (
            b"<?xml version=\"1.0\"?>\n<svg xmlns=\"\">",
            "image/svg+xml",
            true,
        ),
        (b"<SVG>", "image/svg+xml", true),
        (b"<html></html>", "image/svg+xml", false),
        (&far_svg, "image/svg+xml", false),
        (b"", "image/png", false),
        // A type the viewer doesn't show matches nothing
        (b"BM", "image/bmp", false),
    ];
    for (data, content_type, matches) in cases {
        assert_eq!(
            image_matches(data, content_type),
            matches,
            "{content_type} {:?}",
            String::from_utf8_lossy(&data[..data.len().min(24)])
        );
    }
}

#[test]
fn a_link_target_is_bracketed_only_when_markdown_would_misread_it() {
    for (name, target) in [
        ("a.png", "a.png"),
        ("a-b_c.png", "a-b_c.png"),
        ("a b.png", "<a b.png>"),
        ("a\tb.png", "<a\tb.png>"),
        ("a(1).png", "<a(1).png>"),
        ("a)b.png", "<a)b.png>"),
        ("a<b>.png", "<a<b>.png>"),
    ] {
        assert_eq!(link_target(name), target, "{name}");
    }
}

#[test]
fn markdown_is_rendered_as_it_is_and_code_and_text_in_a_fence() {
    let markdown = render("docs/a.md", "markdown", None, b"# A\n\nText.\n");
    assert_eq!(
        (
            markdown.kind,
            markdown.language,
            markdown.document.as_str(),
            markdown.line_offset,
            markdown.lines
        ),
        ("markdown", None, "# A\n\nText.\n", 0, 3)
    );
    assert_eq!(markdown.text.as_deref(), Some("# A\n\nText.\n"));
    let code = render("a.rs", "code", Some("rust"), b"fn a() {}\nfn b() {}");
    assert_eq!(
        (
            code.kind,
            code.language,
            code.document.as_str(),
            code.line_offset,
            code.lines
        ),
        (
            "code",
            Some("rust"),
            "```rust\nfn a() {}\nfn b() {}\n```\n",
            1,
            2
        )
    );
    // Text has no language; an empty file no lines
    let text = render("notes.txt", "text", None, b"");
    assert_eq!(
        (text.kind, text.language, text.document.as_str(), text.lines),
        ("text", None, "```\n```\n", 0)
    );
}

#[test]
fn a_file_that_isnt_utf8_text_is_rendered_as_a_line_saying_its_binary() {
    let nul = render("a.rs", "code", Some("rust"), b"fn\0");
    assert_eq!(
        (nul.kind, nul.language, nul.text, nul.document.as_str()),
        ("binary", None, None, "*a.rs is a binary file: 3 bytes.*\n")
    );
    assert_eq!((nul.lines, nul.line_offset), (0, 0));
    let latin1 = render("dir/b.md", "markdown", None, b"caf\xe9");
    assert_eq!(
        (latin1.kind, latin1.document.as_str()),
        ("binary", "*b.md is a binary file: 4 bytes.*\n")
    );
    // A NUL past what is sniffed doesn't make a file binary
    let mut late = vec![b'a'; DOC_SNIFF_BYTES];
    late.push(0);
    assert_eq!(render("a.txt", "text", None, &late).kind, "text");
}

#[test]
fn an_image_is_rendered_as_a_markdown_image_of_itself_or_binary_when_it_isnt_one() {
    let png = b"\x89PNG\r\n\x1a\n";
    let image = render("img/a b.png", "image", Some("image/png"), png);
    assert_eq!(
        (
            image.kind,
            image.language,
            image.text,
            image.document.as_str()
        ),
        ("image", None, None, "![a b.png](<a b.png>)\n")
    );
    let fake = render("a.png", "image", Some("image/png"), b"<html>");
    assert_eq!(
        (fake.kind, fake.document.as_str()),
        (
            "binary",
            "*a.png isn't the image its name says: 6 bytes.*\n"
        )
    );
}

#[test]
fn a_file_is_known_by_its_name() {
    assert_eq!(document_kind("docs/a.comp.md"), ("markdown", None));
    assert_eq!(document_kind("x.SVG"), ("image", Some("image/svg+xml")));
    assert_eq!(document_kind("src/main.rs"), ("code", Some("rust")));
    assert_eq!(document_kind("Dockerfile"), ("code", Some("dockerfile")));
    assert_eq!(document_kind("notes.txt"), ("text", None));
}

#[test]
fn code_is_fenced_longer_than_its_backticks() {
    assert_eq!(fence("a ```` b", Some("md")), "`````md\na ```` b\n`````\n");
    assert_eq!(fence("x\n", None), "```\nx\n```\n");
    assert_eq!(link_target("a b.png"), "<a b.png>");
    assert!(image_matches(b"GIF89a...", "image/gif"));
    assert!(!image_matches(b"<html>", "image/png"));
}
