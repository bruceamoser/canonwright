//! Reading a manuscript that already exists into the Story State Graph.
//!
//! A writer arriving with a finished draft is the normal case, not the edge
//! case. This crate turns a document into a **scaffold**: one scene per chapter
//! or scene break, marked `Drafted`, with its prose kept and its deltas empty.
//!
//! There is no model here on purpose. Structure comes from the document, so the
//! only thing a later extraction pass has to invent is meaning. When the
//! extraction pass is wrong — and it will be — you can still tell which mistakes
//! were the parser's and which were the model's.
//!
//! ```
//! use canonwright_manuscript::{detect, read_str, segment};
//!
//! let document = read_str("# Chapter One\n\nThe road was closed.\n\n***\n\nShe waited.\n", "manuscript.md");
//! assert_eq!(detect("manuscript.md"), Some(canonwright_manuscript::Format::Markdown));
//!
//! let segments = segment(&document);
//! assert_eq!(segments.len(), 2, "a scene break splits the chapter in two");
//! ```

pub mod readers;
pub mod segment;

use std::fmt;
use std::io::Cursor;
use std::path::Path;

pub use segment::{Scaffold, Segment, SegmentKind, scaffold, segment};

/// A container a manuscript plausibly arrives in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Format {
    Markdown,
    /// The default: an unknown file is treated as text rather than refused.
    #[default]
    PlainText,
    Docx,
    Epub,
    Pdf,
}

impl Format {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::PlainText => "plain text",
            Self::Docx => "docx",
            Self::Epub => "epub",
            Self::Pdf => "pdf",
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

/// The document, reduced to the two things every format can agree on: a run of
/// blocks in reading order, and whether a block is structurally a heading.
#[derive(Debug, Clone, Default)]
pub struct Document {
    pub format: Format,
    pub title: Option<String>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: BlockKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// A heading and its depth, 1 being a chapter.
    Heading(u8),
    Paragraph,
    /// An explicit scene break: `***`, `* * *`, `---`, or a horizontal rule.
    Break,
}

impl Document {
    #[must_use]
    pub fn from_blocks(format: Format, blocks: Vec<Block>) -> Self {
        Self {
            format,
            title: None,
            blocks,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.blocks.iter().all(|block| block.text.trim().is_empty())
    }

    #[must_use]
    pub fn word_count(&self) -> usize {
        self.blocks
            .iter()
            .map(|block| block.text.split_whitespace().count())
            .sum()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("{path} does not look like a manuscript we can read: {detail}")]
    Unsupported { path: String, detail: String },
    #[error("{path} is a {format} file but {detail}")]
    Malformed {
        path: String,
        format: &'static str,
        detail: String,
    },
    #[error("io error reading {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("zip error in {path}: {source}")]
    Zip {
        path: String,
        #[source]
        source: zip::result::ZipError,
    },
    #[error("could not parse the XML inside {path}: {detail}")]
    Xml { path: String, detail: String },
    #[error("could not extract text from the pdf {path}: {detail}")]
    Pdf { path: String, detail: String },
}

/// Guess the format from the extension, and only then from the bytes.
///
/// Extensions lie — a `.docx` renamed from `.zip`, a `.txt` that is really a
/// PDF — so [`read`] confirms the guess against the file's magic bytes before
/// trusting a parser.
#[must_use]
pub fn detect(path: &str) -> Option<Format> {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())?
        .to_ascii_lowercase();
    match extension.as_str() {
        "md" | "markdown" | "mdown" | "mkd" => Some(Format::Markdown),
        "txt" | "text" | "rtf" => Some(Format::PlainText),
        "docx" | "doc" => Some(Format::Docx),
        "epub" => Some(Format::Epub),
        "pdf" => Some(Format::Pdf),
        _ => None,
    }
}

/// Read a manuscript from disk.
pub fn read(path: &Path) -> Result<Document, ImportError> {
    let bytes = std::fs::read(path).map_err(|source| ImportError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let name = path.display().to_string();
    read_bytes(&bytes, &name, detect(&name))
}

/// Read a manuscript that is already in memory, where the caller supplies the
/// name used for format detection and error messages.
pub fn read_bytes(
    bytes: &[u8],
    name: &str,
    declared: Option<Format>,
) -> Result<Document, ImportError> {
    let format = confirm(bytes, name, declared)?;
    match format {
        Format::Markdown => Ok(readers::markdown(&decode_text(bytes), name)),
        Format::PlainText => Ok(readers::plain_text(&decode_text(bytes), name)),
        Format::Docx => readers::docx(bytes, name),
        Format::Epub => readers::epub(bytes, name),
        Format::Pdf => readers::pdf(bytes, name),
    }
}

/// Read a manuscript from a string, for tests and for pasted text.
#[must_use]
pub fn read_str(text: &str, name: &str) -> Document {
    match detect(name) {
        Some(Format::Markdown) => readers::markdown(text, name),
        _ => readers::plain_text(text, name),
    }
}

/// Trust the bytes over the extension.
fn confirm(bytes: &[u8], name: &str, declared: Option<Format>) -> Result<Format, ImportError> {
    let malformed = |format: &'static str, detail: &str| ImportError::Malformed {
        path: name.to_owned(),
        format,
        detail: detail.to_owned(),
    };

    if bytes.starts_with(b"%PDF") {
        return Ok(Format::Pdf);
    }

    if bytes.starts_with(b"PK\x03\x04") {
        let container = zip_contains(bytes, name)?;
        if container.docx {
            return Ok(Format::Docx);
        }
        if container.epub {
            return Ok(Format::Epub);
        }
        return Err(malformed(
            "zip container",
            "it is a zip archive but holds neither word/document.xml nor META-INF/container.xml",
        ));
    }

    // Not a container and not a pdf: it is text of some kind.
    match declared {
        Some(Format::Markdown) => Ok(Format::Markdown),
        Some(Format::PlainText) | None => Ok(Format::PlainText),
        Some(other) if other == Format::Docx || other == Format::Epub => Err(malformed(
            other.name(),
            "the file is not a zip container; the extension is lying",
        )),
        Some(Format::Pdf) => Err(malformed(
            "pdf",
            "the file does not start with %PDF; the extension is lying",
        )),
        Some(_) => Ok(Format::PlainText),
    }
}

struct Container {
    docx: bool,
    epub: bool,
}

fn zip_contains(bytes: &[u8], name: &str) -> Result<Container, ImportError> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|source| ImportError::Zip {
            path: name.to_owned(),
            source,
        })?;
    let mut container = Container {
        docx: false,
        epub: false,
    };
    for index in 0..archive.len() {
        let Ok(entry) = archive.by_index(index) else {
            continue;
        };
        if entry.name() == "word/document.xml" {
            container.docx = true;
        }
        if entry.name() == "META-INF/container.xml" {
            container.epub = true;
        }
    }
    Ok(container)
}

/// Manuscripts are UTF-8 or they are old. Decode leniently rather than refusing
/// a 90,000-word file over one stray byte from a 1994 word processor.
fn decode_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    }
}
