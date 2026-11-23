use std::io::Cursor;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::{Block, BlockKind, Document, Format, ImportError};

// ---------------------------------------------------------------------------
// Markdown and plain text
// ---------------------------------------------------------------------------

/// Read Markdown: ATX headings, blank-line paragraphs, and thematic breaks.
///
/// Setext headings (`Title` underlined with `===`) are treated as a break rather
/// than a heading. That is a deliberate simplification: `---` is the most common
/// scene break in the world, and guessing between "underline" and "cut" from
/// context produces silent mis-segmentation, which is worse than a missing
/// heading.
#[must_use]
pub fn markdown(text: &str, _name: &str) -> Document {
    let (title, body) = strip_front_matter(text);
    let mut blocks = Vec::new();
    let mut paragraph = String::new();

    for raw in body.lines() {
        let line = raw.trim();
        if line.is_empty() {
            flush(&mut paragraph, &mut blocks);
            continue;
        }
        if let Some((level, heading)) = atx_heading(line) {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block {
                kind: BlockKind::Heading(level),
                text: heading,
            });
            continue;
        }
        if is_thematic_break(line) {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block {
                kind: BlockKind::Break,
                text: String::new(),
            });
            continue;
        }
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(line);
    }
    flush(&mut paragraph, &mut blocks);

    let mut document = Document::from_blocks(Format::Markdown, blocks);
    document.title = title;
    document
}

/// Read plain text: blank-line paragraphs, with chapter lines promoted to
/// headings so a `.txt` export still segments somewhere sensible.
#[must_use]
pub fn plain_text(text: &str, _name: &str) -> Document {
    let mut blocks = Vec::new();
    let mut paragraph = String::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            flush(&mut paragraph, &mut blocks);
            continue;
        }
        if looks_like_chapter(line) {
            flush(&mut paragraph, &mut blocks);
            blocks.push(Block {
                kind: BlockKind::Heading(1),
                text: line.to_owned(),
            });
            continue;
        }
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(line);
    }
    flush(&mut paragraph, &mut blocks);

    Document::from_blocks(Format::PlainText, blocks)
}

fn flush(paragraph: &mut String, blocks: &mut Vec<Block>) {
    let text = paragraph.trim();
    if !text.is_empty() {
        blocks.push(Block {
            kind: BlockKind::Paragraph,
            text: text.to_owned(),
        });
    }
    paragraph.clear();
}

fn atx_heading(line: &str) -> Option<(u8, String)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = line[level..].trim_start();
    if rest.len() == line.len() - level {
        // No space after the hashes: `#hashtag` is not a heading.
        return None;
    }
    let text = rest.trim_end_matches('#').trim();
    if text.is_empty() {
        return None;
    }
    Some((u8::try_from(level).ok()?, text.to_owned()))
}

fn is_thematic_break(line: &str) -> bool {
    let mut marker = None;
    let mut count = 0;
    for character in line.chars() {
        if character.is_whitespace() {
            continue;
        }
        match marker {
            None if matches!(character, '*' | '-' | '_' | '=') => marker = Some(character),
            Some(expected) if character == expected => {}
            _ => return false,
        }
        count += 1;
    }
    count >= 3 && marker.is_some()
}

fn looks_like_chapter(line: &str) -> bool {
    if line.ends_with('.') || line.len() > 80 {
        return false;
    }
    let lower = line.to_ascii_lowercase();
    for prefix in ["chapter", "part", "book", "act", "prologue", "epilogue"] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            let rest = rest.trim();
            if rest.is_empty()
                || rest
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit() || matches!(c, 'i' | 'v' | 'x' | 'o'))
            {
                return true;
            }
        }
    }
    // A short shouted line is a chapter title far more often than it is prose.
    let words = line.split_whitespace().count();
    words <= 8
        && line.chars().any(|c| c.is_ascii_alphabetic())
        && line == line.to_uppercase()
        && !line.contains('.')
}

/// Returns the YAML front-matter title, if there is one, and the body after it.
fn strip_front_matter(text: &str) -> (Option<String>, &str) {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (None, text);
    }
    let mut title = None;
    let mut consumed = 3; // the opening fence plus its newline
    for line in text.lines().skip(1) {
        consumed += line.len() + 1;
        let trimmed = line.trim();
        if trimmed == "---" {
            return (title, text.get(consumed..).unwrap_or(""));
        }
        if let Some(value) = trimmed.strip_prefix("title:") {
            title = Some(value.trim().trim_matches('"').to_owned());
        }
    }
    (None, text)
}

// ---------------------------------------------------------------------------
// DOCX
// ---------------------------------------------------------------------------

pub fn docx(bytes: &[u8], name: &str) -> Result<Document, ImportError> {
    let xml = read_entry(bytes, name, "word/document.xml")?;
    let mut reader = Reader::from_str(&xml);
    let mut blocks = Vec::new();
    let mut current = String::new();
    let mut heading: Option<u8> = None;
    let mut in_text = false;
    let mut in_paragraph = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => match local(element.name().into_inner()) {
                "p" => {
                    in_paragraph = true;
                    current.clear();
                    heading = None;
                }
                "pStyle" => {
                    if let Some(style) = attribute(&element, "val") {
                        heading = heading_level(&style);
                    }
                }
                "t" => in_text = true,
                _ => {}
            },
            Ok(Event::End(element)) => match local(element.name().into_inner()) {
                "t" => in_text = false,
                "p" => {
                    if in_paragraph {
                        let text = current.trim();
                        if !text.is_empty() {
                            blocks.push(Block {
                                kind: match heading {
                                    Some(level) => BlockKind::Heading(level),
                                    None => BlockKind::Paragraph,
                                },
                                text: text.to_owned(),
                            });
                        }
                    }
                    in_paragraph = false;
                }
                _ => {}
            },
            Ok(Event::Empty(element)) => {
                // `<w:pStyle w:val="Heading1"/>` is self-closing, so it never
                // arrives as a Start event.
                if local(element.name().into_inner()) == "pStyle" {
                    if let Some(style) = attribute(&element, "val") {
                        heading = heading_level(&style);
                    }
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if in_text {
                    match entity(reference.as_ref()) {
                        Some(character) => current.push(character),
                        None => {
                            current.push('&');
                            current.push_str(reference.as_ref());
                            current.push(';');
                        }
                    }
                }
            }
            Ok(Event::Text(text)) => {
                if in_text {
                    current.push_str(&unescape(text.as_ref()));
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(ImportError::Xml {
                    path: name.to_owned(),
                    detail: error.to_string(),
                });
            }
            _ => {}
        }
    }

    let mut document = Document::from_blocks(Format::Docx, blocks);
    document.title = docx_title(bytes, name);
    Ok(document)
}

fn docx_title(bytes: &[u8], name: &str) -> Option<String> {
    let xml = read_entry(bytes, name, "docProps/core.xml").ok()?;
    let reader = Reader::from_str(&xml);
    let mut reader = reader;
    let mut in_title = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) if local(element.name().into_inner()) == "title" => {
                in_title = true;
            }
            Ok(Event::End(element)) if local(element.name().into_inner()) == "title" => {
                return None;
            }
            Ok(Event::Text(text)) if in_title => {
                return Some(unescape(text.as_ref()).trim().to_owned());
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

fn heading_level(style: &str) -> Option<u8> {
    let lower = style.to_ascii_lowercase();
    let digits: String = lower.chars().filter(char::is_ascii_digit).collect();
    let prefix = lower.trim_end_matches(|c: char| c.is_ascii_digit()).trim();
    let known = matches!(
        prefix,
        "heading" | "titre" | "title" | "überschrift" | "kopfzeile" | "h"
    );
    if known {
        if let Ok(level) = digits.parse::<u8>() {
            return Some(level.clamp(1, 6));
        }
        return Some(1);
    }
    if lower == "title" {
        return Some(1);
    }
    None
}

// ---------------------------------------------------------------------------
// EPUB
// ---------------------------------------------------------------------------

pub fn epub(bytes: &[u8], name: &str) -> Result<Document, ImportError> {
    let container = read_entry(bytes, name, "META-INF/container.xml")?;
    let rootfile = find_attribute(&container, "rootfile", "full-path").ok_or_else(|| {
        ImportError::Malformed {
            path: name.to_owned(),
            format: "epu",
            detail: "META-INF/container.xml names no rootfile".to_owned(),
        }
    })?;
    let package = read_entry(bytes, name, &rootfile)?;
    let base = parent_path(&rootfile);

    let (title, hrefs) = epub_spine(&package, &base);
    let mut blocks = Vec::new();
    for href in hrefs {
        let Ok(chapter) = read_entry(bytes, name, &href) else {
            continue;
        };
        read_xhtml(&chapter, &mut blocks);
    }

    let mut document = Document::from_blocks(Format::Epub, blocks);
    document.title = title;
    Ok(document)
}

/// Returns the package title and the spine's document hrefs, in reading order.
fn epub_spine(package: &str, base: &str) -> (Option<String>, Vec<String>) {
    let mut reader = Reader::from_str(package);
    let mut manifest: Vec<(String, String)> = Vec::new();
    let mut spine: Vec<String> = Vec::new();
    let mut title = None;
    let mut in_title = false;
    let mut in_spine = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => match local(element.name().into_inner()) {
                "item" => {
                    if let (Some(id), Some(href)) =
                        (attribute(&element, "id"), attribute(&element, "href"))
                    {
                        manifest.push((id, href));
                    }
                }
                "spine" => in_spine = true,
                "itemref" if in_spine => {
                    if let Some(idref) = attribute(&element, "idref") {
                        spine.push(idref);
                    }
                }
                "title" => in_title = true,
                _ => {}
            },
            Ok(Event::Empty(element)) => match local(element.name().into_inner()) {
                "item" => {
                    if let (Some(id), Some(href)) =
                        (attribute(&element, "id"), attribute(&element, "href"))
                    {
                        manifest.push((id, href));
                    }
                }
                "itemref" if in_spine => {
                    if let Some(idref) = attribute(&element, "idref") {
                        spine.push(idref);
                    }
                }
                _ => {}
            },
            Ok(Event::End(element)) => match local(element.name().into_inner()) {
                "spine" => in_spine = false,
                "title" => in_title = false,
                _ => {}
            },
            Ok(Event::Text(text)) if in_title => {
                let value = unescape(text.as_ref());
                if !value.trim().is_empty() {
                    title = Some(value.trim().to_owned());
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    let hrefs = spine
        .into_iter()
        .filter_map(|idref| {
            manifest
                .iter()
                .find(|(id, _)| id == &idref)
                .map(|(_, href)| join_path(base, href))
        })
        .collect();
    (title, hrefs)
}

fn read_xhtml(xhtml: &str, blocks: &mut Vec<Block>) {
    let mut reader = Reader::from_str(xhtml);
    let mut current = String::new();
    let mut heading: Option<u8> = None;
    let mut in_block = false;
    let mut skipping = 0_u32;

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                let name = local(element.name().into_inner());
                match name {
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        in_block = true;
                        current.clear();
                        heading = Some(name.as_bytes()[1] - b'0');
                    }
                    "p" | "div" if !in_block => {
                        in_block = true;
                        current.clear();
                        heading = None;
                    }
                    "script" | "style" => skipping += 1,
                    _ => {}
                }
            }
            Ok(Event::End(element)) => {
                let name = local(element.name().into_inner());
                match name {
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "div" => {
                        let text = current.trim();
                        if in_block && !text.is_empty() {
                            blocks.push(Block {
                                kind: match heading {
                                    Some(level) => BlockKind::Heading(level),
                                    None => BlockKind::Paragraph,
                                },
                                text: text.to_owned(),
                            });
                        }
                        in_block = false;
                        current.clear();
                        heading = None;
                    }
                    "script" | "style" => skipping = skipping.saturating_sub(1),
                    _ => {}
                }
            }
            Ok(Event::GeneralRef(reference)) if skipping == 0 && in_block => {
                match entity(reference.as_ref()) {
                    Some(character) => current.push(character),
                    None => {
                        current.push('&');
                        current.push_str(reference.as_ref());
                        current.push(';');
                    }
                }
            }
            Ok(Event::Text(text)) if skipping == 0 && in_block => {
                current.push_str(&unescape(text.as_ref()));
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// PDF
// ---------------------------------------------------------------------------

/// Extract text from a PDF.
///
/// Best effort by nature: a PDF is a page-layout format, and a scanned one is an
/// image. Text that does come out arrives as one stream, so paragraphs are
/// recovered from blank lines and every page becomes prose with no heading
/// structure — which means the segmenter will fall back to heuristic splits.
pub fn pdf(bytes: &[u8], name: &str) -> Result<Document, ImportError> {
    let text = pdf_extract::extract_text_from_mem(bytes).map_err(|error| ImportError::Pdf {
        path: name.to_owned(),
        detail: error.to_string(),
    })?;
    if text.trim().is_empty() {
        return Err(ImportError::Pdf {
            path: name.to_owned(),
            detail: "no text layer was found; a scanned pdf needs OCR before import".to_owned(),
        });
    }
    let mut document = plain_text(&text, name);
    document.format = Format::Pdf;
    Ok(document)
}

// ---------------------------------------------------------------------------
// Shared XML helpers
// ---------------------------------------------------------------------------

fn read_entry(bytes: &[u8], name: &str, entry: &str) -> Result<String, ImportError> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|source| ImportError::Zip {
            path: name.to_owned(),
            source,
        })?;
    let mut file = archive.by_name(entry).map_err(|_| ImportError::Malformed {
        path: name.to_owned(),
        format: "zip container",
        detail: format!("it has no {entry}"),
    })?;
    let mut buffer = Vec::new();
    std::io::Read::read_to_end(&mut file, &mut buffer).map_err(|source| ImportError::Io {
        path: name.to_owned(),
        source,
    })?;
    Ok(String::from_utf8_lossy(&buffer).into_owned())
}

fn local(name: &str) -> &str {
    match name.rfind(':') {
        Some(position) => &name[position + 1..],
        None => name,
    }
}

fn attribute(element: &BytesStart<'_>, name: &str) -> Option<String> {
    element
        .attributes()
        .flatten()
        .find(|attribute| local(attribute.key.as_ref()).eq_ignore_ascii_case(name))
        .map(|attribute| attribute.value.into_owned())
}

/// Find the first `element` tag in an XML string and read one of its attributes.
fn find_attribute(xml: &str, element: &str, name: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(tag)) | Ok(Event::Empty(tag)) => {
                if local(tag.name().into_inner()).eq_ignore_ascii_case(element) {
                    if let Some(value) = attribute(&tag, name) {
                        return Some(value);
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

fn parent_path(path: &str) -> String {
    match path.rfind('/') {
        Some(index) => path[..index].to_owned(),
        None => String::new(),
    }
}

fn join_path(base: &str, href: &str) -> String {
    let joined = if base.is_empty() {
        href.to_owned()
    } else {
        format!("{base}/{href}")
    };
    // Collapse `a/../b` without touching the filesystem.
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// Decode the entities that actually appear in manuscripts.
///
/// A full XML unescape would be the correct thing to do; a writer's curly quotes
/// and em dashes are what actually need decoding, and a numeric character
/// reference is where a naive parser loses them.
fn entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ => name
            .strip_prefix("#x")
            .or_else(|| name.strip_prefix("#X"))
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .or_else(|| name.strip_prefix('#').and_then(|dec| dec.parse().ok()))
            .and_then(char::from_u32),
    }
}

fn unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let Some(end) = after.find(';').filter(|end| *end <= 12) else {
            out.push_str(after);
            return out;
        };
        match entity(&after[1..end]) {
            Some(character) => out.push(character),
            None => out.push_str(&after[..=end]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}
