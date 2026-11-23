use std::io::{Cursor, Write};

use canonwright_manuscript::segment::{SegmentKind, scaffold};
use canonwright_manuscript::{
    BlockKind, Format, ImportError, detect, read_bytes, read_str, segment,
};

fn zip_of(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default();
        for (name, content) in entries {
            writer.start_file(*name, options).expect("start file");
            writer.write_all(content.as_bytes()).expect("write entry");
        }
        writer.finish().expect("finish zip");
    }
    buffer.into_inner()
}

const DOCX: &[(&str, &str)] = &[
    (
        "word/document.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>
  <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Chapter One</w:t></w:r></w:p>
  <w:p><w:r><w:t>The road was closed &amp; nobody said why.</w:t></w:r></w:p>
  <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Chapter Two</w:t></w:r></w:p>
  <w:p><w:r><w:t>She waited.</w:t></w:r></w:p>
</w:body></w:document>"#,
    ),
    (
        "docProps/core.xml",
        r#"<cp:coreProperties><dc:title>The Salt Road</dc:title></cp:coreProperties>"#,
    ),
];

fn epub_entries() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "META-INF/container.xml",
            r#"<container><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#,
        ),
        (
            "OEBPS/content.opf",
            r#"<package><metadata><dc:title>The Salt Road</dc:title></metadata>
<manifest>
  <item id="c1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  <item id="c2" href="ch2.xhtml" media-type="application/xhtml+xml"/>
</manifest>
<spine><itemref idref="c1"/><itemref idref="c2"/></spine></package>"#,
        ),
        (
            "OEBPS/ch1.xhtml",
            r#"<html><body><h1>Chapter One</h1><p>The road was closed.</p></body></html>"#,
        ),
        (
            "OEBPS/ch2.xhtml",
            r#"<html><body><h1>Chapter Two</h1><p>She waited&#8212;and counted.</p></body></html>"#,
        ),
    ]
}

#[test]
fn the_extension_is_the_first_guess() {
    assert_eq!(detect("a.md"), Some(Format::Markdown));
    assert_eq!(detect("a.markdown"), Some(Format::Markdown));
    assert_eq!(detect("a.txt"), Some(Format::PlainText));
    assert_eq!(detect("a.docx"), Some(Format::Docx));
    assert_eq!(detect("a.epub"), Some(Format::Epub));
    assert_eq!(detect("a.pdf"), Some(Format::Pdf));
    assert_eq!(detect("a.png"), None);
    assert_eq!(detect("no extension"), None);
}

#[test]
fn a_markdown_manuscript_segments_on_headings_and_breaks() {
    let document = read_str(
        "---\ntitle: The Salt Road\n---\n\n# Chapter One\n\nThe road was closed.\n\n***\n\nShe waited.\n\n# Chapter Two\n\nMorning.\n",
        "book.md",
    );
    assert_eq!(document.title.as_deref(), Some("The Salt Road"));

    let segments = segment(&document);
    assert_eq!(segments.len(), 3, "{segments:#?}");
    assert_eq!(segments[0].title.as_deref(), Some("Chapter One"));
    assert_eq!(segments[0].kind, SegmentKind::Heading);
    assert_eq!(segments[1].kind, SegmentKind::SceneBreak);
    assert_eq!(segments[1].title, None);
    assert!(segments[1].body.contains("She waited."));
    assert_eq!(segments[2].title.as_deref(), Some("Chapter Two"));
}

#[test]
fn a_hashtag_is_not_a_heading() {
    let document = read_str("#notaheading\n\nA tag, not a chapter.\n", "book.md");
    assert!(
        !document
            .blocks
            .iter()
            .any(|block| matches!(block.kind, BlockKind::Heading(_))),
        "{:?}",
        document.blocks
    );
}

#[test]
fn a_plain_text_export_still_finds_its_chapters() {
    let document = read_str(
        "CHAPTER ONE\n\nThe road was closed.\n\nShe waited.\n\nCHAPTER TWO\n\nMorning.\n",
        "book.txt",
    );
    let headings: Vec<&str> = document
        .blocks
        .iter()
        .filter(|block| matches!(block.kind, BlockKind::Heading(_)))
        .map(|block| block.text.as_str())
        .collect();
    assert_eq!(headings, vec!["CHAPTER ONE", "CHAPTER TWO"]);
    assert_eq!(segment(&document).len(), 2);
}

#[test]
fn a_structureless_document_is_split_on_word_count_and_says_so() {
    let paragraph = "word ".repeat(400);
    let body = (0..8)
        .map(|_| paragraph.trim())
        .collect::<Vec<_>>()
        .join("\n\n");
    let document = read_str(&body, "book.txt");
    let segments = segment(&document);

    assert!(segments.len() >= 2, "{segments:#?}");
    assert!(
        segments.iter().all(|s| s.kind == SegmentKind::Heuristic),
        "a guess must be labelled a guess"
    );
}

#[test]
fn a_real_docx_is_read_with_its_headings_and_entities() {
    let bytes = zip_of(DOCX);
    assert_eq!(detect("book.docx"), Some(Format::Docx));

    let document = read_bytes(&bytes, "book.docx", Some(Format::Docx)).expect("docx reads");
    assert_eq!(document.format, Format::Docx);
    assert_eq!(document.title.as_deref(), Some("The Salt Road"));

    let headings: Vec<&str> = document
        .blocks
        .iter()
        .filter(|block| matches!(block.kind, BlockKind::Heading(_)))
        .map(|block| block.text.as_str())
        .collect();
    assert_eq!(headings, vec!["Chapter One", "Chapter Two"]);
    assert!(
        document
            .blocks
            .iter()
            .any(|block| block.text.contains("closed & nobody")),
        "entities must be decoded: {:?}",
        document.blocks
    );
    assert_eq!(segment(&document).len(), 2);
}

#[test]
fn a_real_epub_is_read_in_spine_order() {
    let bytes = zip_of(&epub_entries());
    assert_eq!(detect("book.epub"), Some(Format::Epub));

    let document = read_bytes(&bytes, "book.epub", Some(Format::Epub)).expect("epub reads");
    assert_eq!(document.format, Format::Epub);
    assert_eq!(document.title.as_deref(), Some("The Salt Road"));

    let segments = segment(&document);
    assert_eq!(segments.len(), 2, "{segments:#?}");
    assert_eq!(segments[0].title.as_deref(), Some("Chapter One"));
    assert!(
        segments[1].body.contains("She waited—and counted."),
        "numeric character references must decode: {:?}",
        segments[1].body
    );
}

#[test]
fn a_zip_that_is_neither_docx_nor_epub_is_refused_by_name() {
    let bytes = zip_of(&[("readme.txt", "not a manuscript")]);
    let error = read_bytes(&bytes, "book.docx", Some(Format::Docx)).expect_err("refused");
    let message = error.to_string();
    assert!(message.contains("word/document.xml"), "{message}");
}

#[test]
fn an_extension_that_lies_is_reported_rather_than_guessed() {
    let error = read_bytes(b"just text", "book.pdf", Some(Format::Pdf)).expect_err("refused");
    assert!(matches!(error, ImportError::Malformed { .. }), "{error}");
    let error = read_bytes(b"just text", "book.docx", Some(Format::Docx)).expect_err("refused");
    assert!(matches!(error, ImportError::Malformed { .. }), "{error}");
}

#[test]
fn a_pdf_with_no_text_layer_fails_cleanly() {
    let error = read_bytes(b"%PDF-1.4\nnot really", "scan.pdf", Some(Format::Pdf))
        .expect_err("an unreadable pdf is an error, not a panic");
    assert!(matches!(error, ImportError::Pdf { .. }), "{error}");
}

#[test]
fn scaffolding_turns_segments_into_drafted_scenes() {
    let document = read_str(
        "# Chapter One\n\nThe road was closed.\n\n# Chapter Two\n\nShe waited.\n",
        "book.md",
    );
    let segments = segment(&document);

    let mut story = canonwright_story::Story::default();
    story.meta.title = "The Salt Road".to_owned();
    let scaffold = scaffold(&mut story, &segments);

    assert_eq!(scaffold.scenes.len(), 2);
    assert_eq!(
        scaffold.narrator.as_ref().map(ToString::to_string),
        Some("narrator".to_owned()),
        "a manuscript with no cast needs a placeholder point of view"
    );
    assert!(story.check().is_empty(), "{:?}", story.check());

    let first = story.scene(&scaffold.scenes[0]).expect("first scene");
    assert_eq!(first.index, 1);
    assert_eq!(first.title, "Chapter One");
    assert_eq!(first.status, canonwright_story::SceneStatus::Drafted);
    assert!(
        first.delta.is_empty(),
        "the prose is known; the state is not"
    );
    assert!(first.summary.contains("The road was closed"));
}

#[test]
fn scaffolding_appends_to_a_story_that_already_has_scenes() {
    let document = read_str("# Chapter Two\n\nShe waited.\n", "book.md");
    let segments = segment(&document);

    let mut story = canonwright_story::Story::default();
    story.meta.title = "The Salt Road".to_owned();
    let first = scaffold(&mut story, &segments);
    let second = scaffold(&mut story, &segments);

    assert_eq!(first.scenes[0].to_string(), "s1");
    assert_eq!(second.scenes[0].to_string(), "s2");
    assert_eq!(story.scenes.len(), 2);
}
