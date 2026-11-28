pub(crate) mod agent;
pub(crate) mod chapter_sync;
pub(crate) mod editorial;
mod review;
pub(crate) mod review_queue;
pub(crate) use review::{EDITORIAL_PROMPT, KEEPER_PROMPT};
pub(crate) mod author;
mod detector;
mod edition;
pub(crate) mod process;
mod templates;
use templates::{Templates, Values, check_entry};

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use canonwright_harness::hash_bytes;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::State;
use uuid::Uuid;

use crate::{AppState, WorkspaceId, github_api, off_thread};

const MAX_SECTION: usize = 4 * 1024 * 1024;
static BOOK_IO: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Kind {
    #[default]
    Novel,
    Rulebook,
    ContentBook,
}

impl Kind {
    fn from_request(kind: Option<&str>) -> Self {
        match kind {
            Some("rules" | "rulebook") => Self::Rulebook,
            Some("hybrid" | "content_book") => Self::ContentBook,
            _ => Self::Novel,
        }
    }

    fn areas(self) -> &'static [&'static str] {
        match self {
            Self::Novel => &[
                "Characters",
                "Locations",
                "Factions",
                "Timeline",
                "Story threads",
                "World rules",
            ],
            Self::Rulebook => &[
                "Terms",
                "Procedures",
                "Examples",
                "Player options",
                "Stat blocks",
                "Errata",
            ],
            Self::ContentBook => &[
                "Characters",
                "Locations",
                "Factions",
                "Terms",
                "Procedures",
                "Examples",
                "Encounters",
                "Clues",
                "Tables",
                "Handouts",
            ],
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct Section {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    original_pdf: Option<String>,
    id: Uuid,
    title: String,
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum FindingKind {
    Entry,
    Question,
    Note,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct Finding {
    #[serde(default)]
    fields: Values,
    kind: FindingKind,
    area: String,
    title: String,
    detail: String,
    evidence: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
struct Citation {
    section: Uuid,
    evidence: String,
}

#[derive(Clone, Deserialize, Serialize)]
struct Record {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    research_sources: Vec<author::ResearchSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    author_sources: Vec<author::Statement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    retained: Vec<process::Retained>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    unverified_sections: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    citations: Vec<Citation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    editorial: Option<review::Verdict>,
    #[serde(default)]
    pending: bool,
    id: Uuid,
    section: Option<Uuid>,
    finding: Finding,
    #[serde(default)]
    github_url: Option<String>,
    #[serde(default)]
    answer: Option<String>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct Book {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    process_history: Vec<process::Version>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    review_jobs: Vec<review_queue::Job>,
    #[serde(default)]
    templates: Templates,
    #[serde(default)]
    areas: Option<Vec<String>>,
    #[serde(default)]
    legacy_review_recovered: bool,
    kind: Kind,
    sections: Vec<Section>,
    #[serde(default)]
    records: Vec<Record>,
    #[serde(default)]
    repository: Option<String>,
}

impl Book {
    fn areas(&self) -> Vec<&str> {
        match &self.areas {
            Some(names) => names.iter().map(String::as_str).collect(),
            None => self.kind.areas().to_vec(),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct SkillDraft {
    title: String,
    instructions: String,
}

#[derive(Serialize)]
struct SkillCard {
    id: String,
    title: String,
}

struct Skill {
    id: &'static str,
    title: &'static str,
    areas: &'static [&'static str],
    instructions: &'static str,
}

const SKILLS: &[Skill] = &[
    Skill {
        id: "organize-book",
        title: "Organize book",
        areas: &[],
        instructions: include_str!("../skills/organize-book/SKILL.md"),
    },
    Skill {
        id: "extract-characters",
        title: "Extract characters",
        areas: &["Characters"],
        instructions: include_str!("../skills/extract-characters/SKILL.md"),
    },
    Skill {
        id: "extract-locations",
        title: "Extract locations",
        areas: &["Locations"],
        instructions: include_str!("../skills/extract-locations/SKILL.md"),
    },
    Skill {
        id: "extract-factions",
        title: "Extract factions",
        areas: &["Factions"],
        instructions: include_str!("../skills/extract-factions/SKILL.md"),
    },
    Skill {
        id: "extract-rules",
        title: "Extract rules",
        areas: &["Terms", "Procedures", "Examples"],
        instructions: include_str!("../skills/extract-rules/SKILL.md"),
    },
    Skill {
        id: "extract-adventure",
        title: "Extract adventure content",
        areas: &["Encounters", "Clues", "Tables", "Handouts"],
        instructions: include_str!("../skills/extract-adventure/SKILL.md"),
    },
    Skill {
        id: "review-section",
        title: "Review section",
        areas: &[],
        instructions: include_str!("../skills/review-section/SKILL.md"),
    },
];

#[derive(Serialize)]
pub(crate) struct BookView {
    job_roles: std::collections::HashMap<Uuid, String>,
    templates: Templates,
    book: Book,
    areas: Vec<String>,
    skills: Vec<SkillCard>,
}

#[derive(Serialize)]
pub(crate) struct SectionView {
    text: String,
    revision: String,
}

fn error(value: impl std::fmt::Display) -> String {
    value.to_string()
}

pub(crate) fn registered_root(state: &AppState, id: &str) -> Result<PathBuf, String> {
    let id = id.parse::<WorkspaceId>().map_err(error)?;
    let project = state
        .store
        .lock()
        .map_err(error)?
        .workspace_summary(id)
        .map_err(error)?
        .ok_or_else(|| "Book not found in the library".to_owned())?;
    PathBuf::from(project.path).canonicalize().map_err(error)
}

// A connected repository can provide files of its own. Keep studio reads and
// writes inside the registered directory, including when metadata is a symlink.
fn local(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    let mut full = root.to_owned();
    for component in relative.components() {
        let std::path::Component::Normal(part) = component else {
            return Err("Invalid book path".to_owned());
        };
        full.push(part);
        match fs::symlink_metadata(&full) {
            Ok(metadata) if metadata.is_symlink() => {
                return Err(format!("Book path is a symlink: {}", full.display()));
            }
            Ok(_) => {}
            Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => {}
            Err(cause) => return Err(error(cause)),
        }
    }
    Ok(full)
}

fn manifest(root: &Path) -> Result<PathBuf, String> {
    local(root, Path::new(".canonwright/studio/book.json"))
}

fn section_path(root: &Path, id: Uuid) -> Result<PathBuf, String> {
    local(
        root,
        Path::new(&format!(".canonwright/studio/sections/{id}.md")),
    )
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    canonwright_tools::atomic_write(path, text).map_err(error)
}

pub(crate) static DESKTOP: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();
fn save(root: &Path, book: &Book) -> Result<(), String> {
    use tauri::Emitter;
    write(
        &manifest(root)?,
        &serde_json::to_string_pretty(book).map_err(error)?,
    )?;
    if let Some(app) = DESKTOP.get() {
        let _ = app.emit("canonwright:book-updated", root.to_string_lossy());
    }
    Ok(())
}

fn load(root: &Path) -> Result<Book, String> {
    let path = manifest(root)?;
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(error),
        Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => {
            let prior = local(root, Path::new(".canonwright/story.json"))?;
            let kind = if prior.exists() {
                match canonwright_story::Story::load(&prior)
                    .map_err(error)?
                    .meta
                    .kind
                {
                    canonwright_story::model::WorkKind::Narrative => Kind::Novel,
                    canonwright_story::model::WorkKind::Rules => Kind::Rulebook,
                    canonwright_story::model::WorkKind::Hybrid => Kind::ContentBook,
                }
            } else {
                Kind::Novel
            };
            Ok(Book {
                kind,
                ..Book::default()
            })
        }
        Err(cause) => Err(error(cause)),
    }
}

fn add_section(root: &Path, book: &mut Book, title: String, text: &str) -> Result<Uuid, String> {
    if text.len() > MAX_SECTION {
        return Err("This section exceeds 4 MiB. Split it into smaller files.".to_owned());
    }
    let id = Uuid::now_v7();
    write(&section_path(root, id)?, text)?;
    book.sections.push(Section {
        id,
        title,
        original_pdf: None,
    });
    Ok(id)
}

fn retain_pdf(root: &Path, bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > 64 * 1024 * 1024 || !bytes.windows(5).take(1024).any(|w| w == b"%PDF-") {
        return Err("Choose a PDF up to 64 MiB".into());
    }
    let relative = format!(".canonwright/studio/originals/{}.pdf", hash_bytes(bytes));
    let path = local(root, Path::new(&relative))?;
    let parent = path.parent().ok_or("Missing PDF directory")?;
    fs::create_dir_all(parent).map_err(error)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(error)?;
    std::io::Write::write_all(&mut temp, bytes).map_err(error)?;
    temp.persist(path).map_err(error)?;
    Ok(relative)
}

#[tauri::command]
pub(crate) fn studio_attach_pdf(
    state: State<'_, AppState>,
    project_id: String,
    section_id: Uuid,
    path: String,
) -> Result<(), String> {
    let root = registered_root(&state, &project_id)?;
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(&root)?;
    let section = book
        .sections
        .iter_mut()
        .find(|s| s.id == section_id)
        .ok_or("Unknown section")?;
    if section.original_pdf.is_some() {
        return Err("This section already has an original PDF".into());
    }
    let file = Path::new(&path);
    if fs::metadata(file).map_err(error)?.len() > 64 * 1024 * 1024 {
        return Err("Choose a PDF up to 64 MiB".into());
    }
    section.original_pdf = Some(retain_pdf(&root, &fs::read(file).map_err(error)?)?);
    save(&root, &book)
}

fn supported(path: &Path) -> bool {
    canonwright_manuscript::detect(&path.to_string_lossy()).is_some()
}

fn import_sources(
    root: &Path,
    book: &mut Book,
    input: &Path,
    repository: bool,
) -> Result<usize, String> {
    let mut files = Vec::new();
    if input.is_dir() {
        for entry in ignore::WalkBuilder::new(input).max_depth(Some(12)).build() {
            let entry = entry.map_err(error)?;
            let path = entry.path();
            if entry.file_type().is_some_and(|kind| kind.is_file())
                && supported(path)
                && !(repository
                    && path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .is_some_and(|stem| {
                            [
                                "readme",
                                "changelog",
                                "license",
                                "notice",
                                "agents",
                                "skills",
                            ]
                            .iter()
                            .any(|name| stem.eq_ignore_ascii_case(name))
                        }))
            {
                files.push(path.to_owned());
            }
        }
        files.sort();
    } else if input.is_file() && supported(input) {
        files.push(input.to_owned());
    }
    if files.is_empty() {
        return Err("No supported manuscript files were found".to_owned());
    }
    let count = files.len();
    for path in files {
        let title = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Section")
            .to_owned();
        if fs::metadata(&path).map_err(error)?.len() > 64 * 1024 * 1024 {
            return Err(format!("Source file is too large: {}", path.display()));
        }
        use canonwright_manuscript::{BlockKind, Format, detect, read};
        let pdf_bytes = if matches!(detect(&path.to_string_lossy()), Some(Format::Pdf)) {
            Some(fs::read(&path).map_err(error)?)
        } else {
            None
        };
        let text = match detect(&path.to_string_lossy()) {
            Some(Format::Markdown | Format::PlainText) => {
                fs::read_to_string(&path).map_err(error)?
            }
            _ => (if let Some(bytes) = &pdf_bytes {
                canonwright_manuscript::read_bytes(
                    bytes,
                    &path.to_string_lossy(),
                    Some(Format::Pdf),
                )
            } else {
                read(&path)
            })
            .map_err(error)?
            .blocks
            .into_iter()
            .map(|block| match block.kind {
                BlockKind::Heading(depth) => format!(
                    "{} {}",
                    "#".repeat(usize::from(depth).clamp(1, 6)),
                    block.text
                ),
                BlockKind::Break => "***".to_owned(),
                BlockKind::Paragraph => block.text,
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
        };
        let title = text
            .trim_start()
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("# "))
            .map(str::trim)
            .filter(|heading| !heading.is_empty() && heading.chars().count() <= 120)
            .map_or(title, str::to_owned);
        add_section(root, book, title, &text)?;
        if let Some(bytes) = pdf_bytes {
            book.sections.last_mut().unwrap().original_pdf = Some(retain_pdf(root, &bytes)?);
        }
    }
    Ok(count)
}

pub(crate) fn initialize(
    root: &Path,
    kind: Option<&str>,
    source: Option<&Path>,
    repository: Option<String>,
) -> Result<(), String> {
    let _guard = BOOK_IO.lock().map_err(error)?;
    crate::git::ensure_book_repo(root)?;
    if manifest(root)?.exists() {
        return Ok(());
    }
    let kind = Kind::from_request(kind);
    let mut book = Book {
        kind,
        repository,
        ..Book::default()
    };
    if let Some(input) = source {
        let from_repo = book.repository.is_some();
        import_sources(root, &mut book, input, from_repo)?;
    } else {
        let title = match kind {
            Kind::Novel => "Chapter 1",
            Kind::Rulebook => "Core mechanic",
            Kind::ContentBook => "Introduction",
        };
        add_section(root, &mut book, title.to_owned(), "")?;
    }
    save(root, &book)
}

fn custom_dir(root: &Path) -> Result<PathBuf, String> {
    local(root, Path::new(".canonwright/skills"))
}

fn custom_path(root: &Path, id: Uuid) -> Result<PathBuf, String> {
    local(
        root,
        Path::new(&format!(".canonwright/skills/{id}/SKILL.md")),
    )
}

fn available(book: &Book, skill: &Skill) -> bool {
    skill.areas.iter().all(|area| book.areas().contains(area))
}

#[tauri::command]
pub(crate) fn studio_retry_failed_reviews(
    state: State<'_, AppState>,
    project_id: String,
    ids: Vec<Uuid>,
) -> Result<usize, String> {
    let root = registered_root(&state, &project_id)?;
    let count = review_queue::retry_failed_reviews(&root, &ids)?;
    if count > 0 {
        crate::resume_book_workers();
    }
    Ok(count)
}

#[tauri::command]
pub(crate) fn studio_book(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<BookView, String> {
    let root = registered_root(&state, &project_id)?;
    let _guard = BOOK_IO.lock().map_err(error)?;
    crate::git::ensure_book_repo(&root)?;
    let mut book = load(&root)?;
    if !book.legacy_review_recovered {
        let history = state
            .store
            .lock()
            .map_err(error)?
            .legacy_book_findings(project_id.parse::<WorkspaceId>().map_err(error)?)
            .map_err(error)?;
        recover_review(&root, &mut book, history)?;
        book.legacy_review_recovered = true;
        save(&root, &book)?;
        crate::resume_book_workers();
    }
    let areas = book.areas().into_iter().map(str::to_owned).collect();
    let skills = skill_catalog(&root, &book)?;
    let templates = book
        .areas()
        .into_iter()
        .map(|area| (area.to_owned(), book.template(area)))
        .collect();
    let job_roles = book
        .review_jobs
        .iter()
        .map(|job| {
            let role = if job.state == "ready" {
                None
            } else {
                job.claimed_role.as_deref()
            }
            .unwrap_or_else(|| review_queue::job_role(&book, job));
            (job.id, role.to_owned())
        })
        .collect();
    Ok(BookView {
        job_roles,
        templates,
        book,
        areas,
        skills,
    })
}

fn skill_catalog(root: &Path, book: &Book) -> Result<Vec<SkillCard>, String> {
    let mut skills: Vec<_> = SKILLS
        .iter()
        .filter(|skill| available(book, skill))
        .map(|skill| SkillCard {
            id: skill.id.to_owned(),
            title: skill.title.to_owned(),
        })
        .collect();
    match fs::read_dir(custom_dir(root)?) {
        Ok(entries) => {
            for entry in entries {
                let name = entry.map_err(error)?.file_name();
                let Some(id) = name.to_str().and_then(|name| name.parse::<Uuid>().ok()) else {
                    continue;
                };
                let text = fs::read_to_string(custom_path(root, id)?).map_err(error)?;
                if let Some(title) = text.lines().find_map(|line| line.strip_prefix("# ")) {
                    skills.push(SkillCard {
                        id: id.to_string(),
                        title: title.to_owned(),
                    });
                }
            }
        }
        Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => {}
        Err(cause) => return Err(error(cause)),
    }
    process::merge_catalog(book, &mut skills);
    Ok(skills)
}

fn read_section(root: &Path, book: &Book, id: Uuid) -> Result<SectionView, String> {
    if !book.sections.iter().any(|section| section.id == id) {
        return Err("This section does not belong to the book".to_owned());
    }
    let text = fs::read_to_string(section_path(root, id)?).map_err(error)?;
    Ok(SectionView {
        revision: hash_bytes(text.as_bytes()),
        text,
    })
}

#[tauri::command]
pub(crate) fn studio_section(
    state: State<'_, AppState>,
    project_id: String,
    section_id: Uuid,
) -> Result<SectionView, String> {
    let root = registered_root(&state, &project_id)?;
    read_section(&root, &load(&root)?, section_id)
}

#[tauri::command]
pub(crate) fn studio_save_section(
    state: State<'_, AppState>,
    project_id: String,
    section_id: Uuid,
    revision: String,
    text: String,
) -> Result<SectionView, String> {
    let root = registered_root(&state, &project_id)?;
    let changed = chapter_sync::save_edit(&root, section_id, &revision, &text, None)?;
    if changed.is_some() {
        crate::resume_book_workers();
    }
    Ok(SectionView {
        revision: hash_bytes(text.as_bytes()),
        text,
    })
}

#[tauri::command]
pub(crate) fn studio_add_section(
    state: State<'_, AppState>,
    project_id: String,
    title: String,
) -> Result<Uuid, String> {
    let root = registered_root(&state, &project_id)?;
    add_at(&root, &title, "")
}

fn add_at(root: &Path, title: &str, text: &str) -> Result<Uuid, String> {
    add_at_with_parent(root, title, text, None)
}

fn add_at_with_parent(
    root: &Path,
    title: &str,
    text: &str,
    parent: Option<Uuid>,
) -> Result<Uuid, String> {
    let _guard = BOOK_IO.lock().map_err(error)?;
    let title = section_title(title)?;
    let mut book = load(root)?;
    let id = add_section(root, &mut book, title.to_owned(), text)?;
    if parent.is_some() && !text.is_empty() {
        chapter_sync::stage(
            &mut book,
            id,
            &SectionView {
                text: String::new(),
                revision: hash_bytes(b""),
            },
            text,
            parent,
        )?;
    }
    save(root, &book)?;
    Ok(id)
}

#[tauri::command]
pub(crate) async fn studio_import(
    state: State<'_, AppState>,
    project_id: String,
    path: String,
) -> Result<usize, String> {
    let root = registered_root(&state, &project_id)?;
    off_thread(move || {
        let _guard = BOOK_IO.lock().map_err(error)?;
        let mut book = load(&root)?;
        let count = import_sources(&root, &mut book, Path::new(&path), false)?;
        save(&root, &book)?;
        Ok(count)
    })
    .await
}

fn update_outline_section(
    root: &Path,
    id: Uuid,
    title: Option<&str>,
    before: Option<Uuid>,
    after: Option<Uuid>,
) -> Result<(), String> {
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(root)?;
    let index = book
        .sections
        .iter()
        .position(|s| s.id == id)
        .ok_or("Unknown manuscript section")?;
    if before.is_some() && after.is_some() || title.is_none() && before.is_none() && after.is_none()
    {
        return Err("Supply a title or one placement anchor".into());
    }
    let mut section = book.sections.remove(index);
    if let Some(title) = title {
        section.title = section_title(title)?.to_owned();
    }
    let position = if let Some(anchor) = before.or(after) {
        book.sections
            .iter()
            .position(|s| s.id == anchor)
            .ok_or("Choose a different existing section as the anchor")?
            + usize::from(after.is_some())
    } else {
        index
    };
    book.sections.insert(position, section);
    save(root, &book)
}

fn section_title(title: &str) -> Result<&str, String> {
    let title = title.trim();
    if title.is_empty() || title.contains(['\r', '\n']) || title.chars().count() > 120 {
        return Err("Enter a section title of 1–120 characters on one line".to_owned());
    }
    Ok(title)
}

#[tauri::command]
pub(crate) fn studio_order(
    state: State<'_, AppState>,
    project_id: String,
    section_ids: Vec<Uuid>,
) -> Result<(), String> {
    let root = registered_root(&state, &project_id)?;
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(&root)?;
    let mut sections: std::collections::HashMap<_, _> = book
        .sections
        .into_iter()
        .map(|section| (section.id, section))
        .collect();
    if sections.len() != section_ids.len() {
        return Err("The book outline changed. Reload before reordering.".to_owned());
    }
    book.sections = section_ids
        .into_iter()
        .map(|id| {
            sections
                .remove(&id)
                .ok_or_else(|| "Invalid section in outline".to_owned())
        })
        .collect::<Result<_, _>>()?;
    save(&root, &book)
}

fn compile(root: &Path, book: &Book) -> Result<String, String> {
    if book.sections.is_empty() {
        return Err("Add or import a section before building the manuscript".to_owned());
    }
    let mut manuscript = String::new();
    for section in &book.sections {
        let text = fs::read_to_string(section_path(root, section.id)?).map_err(error)?;
        if !text.trim_start().starts_with("# ") {
            manuscript.push_str(&format!("# {}\n\n", section.title));
        }
        manuscript.push_str(&text);
        manuscript.push_str("\n\n");
    }
    Ok(manuscript)
}

#[derive(Serialize)]
pub(crate) struct Manuscript {
    path: String,
    text: String,
}

#[tauri::command]
pub(crate) async fn studio_build(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Manuscript, String> {
    let root = registered_root(&state, &project_id)?;
    off_thread(move || build_at(&root)).await
}

#[tauri::command]
pub(crate) async fn studio_compiled(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<Manuscript, String> {
    let root = registered_root(&state, &project_id)?;
    off_thread(move || {
        let _guard = BOOK_IO.lock().map_err(error)?;
        let text = compile(&root, &load(&root)?)?;
        let relative = ".canonwright/studio/manuscript.md";
        let path = if fs::read_to_string(local(&root, Path::new(relative))?)
            .ok()
            .as_ref()
            == Some(&text)
        {
            relative.to_owned()
        } else {
            String::new()
        };
        Ok(Manuscript { path, text })
    })
    .await
}

fn build_at(root: &Path) -> Result<Manuscript, String> {
    let _guard = BOOK_IO.lock().map_err(error)?;
    let text = compile(root, &load(root)?)?;
    let relative = ".canonwright/studio/manuscript.md";
    write(&local(root, Path::new(relative))?, &text)?;
    Ok(Manuscript {
        path: relative.to_owned(),
        text,
    })
}

fn validate_finding(finding: &Finding, areas: &[&str], source: &str) -> Result<(), String> {
    validate_fields(finding, areas)?;
    if finding.evidence.trim().is_empty()
        || finding.evidence.len() > 1800
        || !source.contains(&finding.evidence)
    {
        return Err("Quote an exact passage from the source section".into());
    }
    Ok(())
}

fn validate_fields(finding: &Finding, areas: &[&str]) -> Result<(), String> {
    if !areas.contains(&finding.area.as_str())
        && !(finding.kind != FindingKind::Entry && finding.area == "Review")
    {
        return Err(format!("Unknown area: {}", finding.area));
    }
    if finding.title.trim().is_empty()
        || finding.title.chars().count() > 160
        || finding.title.contains(['\r', '\n'])
        || finding.detail.trim().is_empty()
        || finding.detail.len() > 9000
    {
        return Err(
            "A finding has invalid fields or quotes a passage absent from the source".to_owned(),
        );
    }
    Ok(())
}

fn recover_review(root: &Path, book: &mut Book, history: Vec<String>) -> Result<(), String> {
    for raw in history {
        let args: serde_json::Value = serde_json::from_str(&raw).map_err(error)?;
        let Some(section) = args["section_id"].as_str().and_then(|id| id.parse().ok()) else {
            continue;
        };
        let finding: Finding = serde_json::from_value(args).map_err(error)?;
        if book.records.iter().any(|r| {
            r.section == Some(section)
                && r.finding.kind == finding.kind
                && r.finding.area == finding.area
                && r.finding.title == finding.title
        }) {
            continue;
        }
        if !book.sections.iter().any(|s| s.id == section) {
            continue;
        }
        let source = read_section(root, book, section)?;
        if validate_finding(&finding, &book.areas(), &source.text).is_ok() {
            insert_finding(book, Some(section), finding, true);
        }
    }
    Ok(())
}

fn insert_finding(book: &mut Book, section: Option<Uuid>, finding: Finding, pending: bool) -> Uuid {
    let id = Uuid::now_v7();
    book.records.push(Record {
        research_sources: vec![],
        author_sources: vec![],
        retained: vec![],
        unverified_sections: vec![],
        citations: vec![],
        editorial: None,
        id,
        section,
        finding,
        pending,
        github_url: None,
        answer: None,
    });
    id
}

fn approve(root: &Path, book: &mut Book, index: usize) -> Result<Uuid, String> {
    let proposed = book.records[index].clone();
    if let Some(section) = proposed.section {
        validate_finding(
            &proposed.finding,
            &book.areas(),
            &read_section(root, book, section)?.text,
        )?;
    }
    if !proposed.finding.fields.is_empty() {
        check_entry(book, &proposed.finding)?;
    }
    // A shared citation identifies the prior version even for older imports that stored notes as entries.
    let matches: Vec<_> = book
        .records
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            !r.pending
                && r.section == proposed.section
                && r.finding.area == proposed.finding.area
                && r.finding
                    .title
                    .eq_ignore_ascii_case(&proposed.finding.title)
                && r.finding.evidence == proposed.finding.evidence
        })
        .map(|(i, _)| i)
        .collect();
    if matches.len() > 1 {
        return Err(
            "Multiple canon entries match this proposal. Resolve the identity before approving."
                .into(),
        );
    }
    if let Some(&target) = matches.first() {
        return consolidate(book, index, target, true);
    }
    book.records[index].pending = false;
    Ok(proposed.id)
}

// Keep the accepted ID and its original citation. Carry additional sources and redirect
// inbound links in the same manifest write that removes the proposal.
fn consolidate(book: &mut Book, index: usize, target: usize, merge: bool) -> Result<Uuid, String> {
    let proposal = book.records[index].clone();
    if !proposal.pending || book.records[target].pending || index == target {
        return Err("Choose a pending proposal and an accepted canon target".into());
    }
    let canon = &mut book.records[target];
    if canon.finding.area != proposal.finding.area && !proposal.finding.fields.is_empty() {
        return Err("Structured fields require a compatible target area".into());
    }
    for (key, incoming) in &proposal.finding.fields {
        match (canon.finding.fields.get_mut(key), incoming) {
            (None, _) => {
                canon.finding.fields.insert(key.clone(), incoming.clone());
            }
            (Some(serde_json::Value::Array(saved)), serde_json::Value::Array(items)) => {
                for item in items {
                    if !saved.contains(item) {
                        saved.push(item.clone());
                    }
                }
            }
            (Some(serde_json::Value::String(saved)), serde_json::Value::String(incoming)) => {
                if merge && !saved.contains(incoming.as_str()) {
                    saved.push_str("\n");
                    saved.push_str(incoming);
                }
            }
            _ => return Err(format!("Incompatible values for field {key}")),
        }
    }
    if merge && !canon.finding.detail.contains(&proposal.finding.detail) {
        canon.finding.detail.push_str("\n\n");
        canon.finding.detail.push_str(&proposal.finding.detail);
    }
    // Preserve the original type for an established structured entry. Early imports
    // used entry for summary-only questions and notes.
    if merge
        && canon.finding.kind == FindingKind::Entry
        && canon.finding.fields.is_empty()
        && canon
            .finding
            .title
            .eq_ignore_ascii_case(&proposal.finding.title)
    {
        canon.finding.kind = proposal.finding.kind;
    }
    let primary = canon.section.map(|section| Citation {
        section,
        evidence: canon.finding.evidence.clone(),
    });
    for citation in proposal
        .section
        .map(|section| Citation {
            section,
            evidence: proposal.finding.evidence.clone(),
        })
        .into_iter()
        .chain(proposal.citations)
    {
        if Some(&citation) != primary.as_ref() && !canon.citations.contains(&citation) {
            canon.citations.push(citation);
        }
    }
    for source in proposal.author_sources {
        if !canon.author_sources.contains(&source) {
            canon.author_sources.push(source);
        }
    }
    canon.retained.extend(proposal.retained);
    canon.editorial = proposal.editorial;
    if canon.answer.is_none() {
        canon.answer = proposal.answer;
    }
    if canon.github_url.is_none() {
        canon.github_url = proposal.github_url;
    }
    let id = canon.id;
    book.records.remove(index);
    let old = proposal.id.to_string();
    for value in book
        .records
        .iter_mut()
        .flat_map(|r| {
            r.finding
                .fields
                .values_mut()
                .chain(r.retained.iter_mut().map(|f| &mut f.value))
        })
        .filter_map(serde_json::Value::as_array_mut)
    {
        for item in value.iter_mut() {
            if item.get("target_id").and_then(serde_json::Value::as_str) == Some(old.as_str()) {
                item["target_id"] = json!(id);
            }
        }
        // Rewriting IDs can collapse two previously distinct links.
        let mut seen = std::collections::HashSet::new();
        value.retain(|item| !item.is_object() || seen.insert(item.to_string()));
    }
    Ok(id)
}

fn decide(root: &Path, id: Uuid, accept: bool) -> Result<(), String> {
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(root)?;
    let index = book
        .records
        .iter()
        .position(|r| r.id == id && r.pending)
        .ok_or("Pending finding not found")?;
    let title = book.records[index].finding.title.clone();
    let result = if accept {
        book.records[index].editorial = None;
        Some(approve(root, &mut book, index)?)
    } else {
        templates::remove_record(&mut book, index);
        None
    };
    let job = if let Some(index) = book.review_jobs.iter().position(|j| j.id == id) {
        &mut book.review_jobs[index]
    } else {
        book.review_jobs.push(review_queue::Job {
            id,
            ..Default::default()
        });
        book.review_jobs.last_mut().unwrap()
    };
    job.reviewed_title = Some(title);
    job.author_decision = Some(
        if accept {
            "Accepted by author"
        } else {
            "Dismissed by author"
        }
        .into(),
    );
    job.result_id = result;
    job.state = "done".into();
    job.error = None;
    job.updated_at = crate::now_millis();
    save(root, &book)
}

#[tauri::command]
pub(crate) fn studio_decide(
    state: State<'_, AppState>,
    project_id: String,
    record_id: Uuid,
    accept: bool,
) -> Result<(), String> {
    decide(&registered_root(&state, &project_id)?, record_id, accept)
}

#[tauri::command]
pub(crate) fn studio_record(
    state: State<'_, AppState>,
    project_id: String,
    section_id: Uuid,
    revision: String,
    finding: Finding,
) -> Result<(), String> {
    let root = registered_root(&state, &project_id)?;
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(&root)?;
    let section = read_section(&root, &book, section_id)?;
    if section.revision != revision {
        return Err(
            "The source passage changed. Run the skill again before saving this finding."
                .to_owned(),
        );
    }
    check_entry(&book, &finding)?;
    validate_finding(&finding, &book.areas(), &section.text)?;
    if finding.kind == FindingKind::Entry {
        if let Some(record) = book.records.iter_mut().find(|record| {
            !record.pending
                && record.finding.kind == FindingKind::Entry
                && record.finding.area == finding.area
                && record.finding.title.to_lowercase() == finding.title.to_lowercase()
        }) {
            record.section = Some(section_id);
            let mut updated = finding;
            let mut values = record.finding.fields.clone();
            values.extend(updated.fields);
            updated.fields = values;
            record.finding = updated;
            return save(&root, &book);
        }
    }
    insert_finding(&mut book, Some(section_id), finding, false);
    save(&root, &book)
}

#[tauri::command]
pub(crate) fn studio_answer(
    state: State<'_, AppState>,
    project_id: String,
    record_id: Uuid,
    answer: String,
) -> Result<(), String> {
    let root = registered_root(&state, &project_id)?;
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(&root)?;
    answer_question(&root, &mut book, record_id, &answer)?;
    save(&root, &book)
}

fn answer_question(
    root: &Path,
    book: &mut Book,
    record_id: Uuid,
    answer: &str,
) -> Result<Uuid, String> {
    let answer = answer.trim();
    if answer.is_empty() || answer.len() > 4000 {
        return Err("Enter an answer of 1–4000 bytes".to_owned());
    }
    let index = book
        .records
        .iter()
        .position(|record| record.id == record_id && record.finding.kind == FindingKind::Question)
        .ok_or_else(|| "Question not found".to_owned())?;
    book.records[index].answer = Some(answer.to_owned());
    book.records[index].editorial = None;
    if book.records[index].pending {
        let title = book.records[index].finding.title.clone();
        let saved = approve(root, book, index)?;
        review_queue::record_result(book, record_id, saved, review::Decision::Approve, &title);
        Ok(saved)
    } else {
        Ok(record_id)
    }
}

fn validate_skill(draft: &SkillDraft) -> Result<(), String> {
    let title = draft.title.trim();
    let instructions = draft.instructions.trim();
    if title.is_empty()
        || title.contains(['\r', '\n'])
        || title.chars().count() > 100
        || instructions.is_empty()
        || instructions.len() > 16000
    {
        return Err("Use a one-line title and up to 16 KB of Markdown instructions".to_owned());
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn studio_save_skill(
    state: State<'_, AppState>,
    project_id: String,
    draft: SkillDraft,
) -> Result<(), String> {
    let root = registered_root(&state, &project_id)?;
    save_project_skill(&root, Uuid::now_v7(), &draft)
}

fn save_project_skill(root: &Path, id: Uuid, draft: &SkillDraft) -> Result<(), String> {
    validate_skill(draft)?;
    let title = draft.title.trim();
    let instructions = draft.instructions.trim();
    let text = format!(
        "---\nname: {id}\ndescription: {}\n---\n\n# {title}\n\n{instructions}\n",
        title.replace(':', " ")
    );
    write(&custom_path(&root, id)?, &text)
}

#[tauri::command]
pub(crate) async fn studio_open_issue(
    state: State<'_, AppState>,
    project_id: String,
    record_id: Uuid,
) -> Result<String, String> {
    let root = registered_root(&state, &project_id)?;
    off_thread(move || open_issue_at(&root, record_id)).await
}

fn open_issue_at(root: &Path, record_id: Uuid) -> Result<String, String> {
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(root)?;
    let repo = book
        .repository
        .as_deref()
        .ok_or_else(|| "This book has no connected repository".to_owned())?;
    let record = book
        .records
        .iter_mut()
        .find(|record| record.id == record_id && record.finding.kind != FindingKind::Entry)
        .ok_or_else(|| "Review item not found".to_owned())?;
    if let Some(url) = &record.github_url {
        return Ok(url.clone());
    }
    let token = github_api::stored_token()
        .ok_or_else(|| "Connect a GitHub account in the library first".to_owned())?;
    let body = format!(
        "{}\n\nCanonWright section: `{}`",
        record.finding.detail,
        record.section.map(|id| id.to_string()).unwrap_or_default()
    );
    let url = github_api::open_issue(&token, repo, &record.finding.title, &body)?;
    record.github_url = Some(url.clone());
    save(root, &book)?;
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approving_enrichment_preserves_canon_identity_and_retargets_links() {
        let root = tempfile::tempdir().unwrap();
        initialize(root.path(), Some("novel"), None, None).unwrap();
        let section = add_at(root.path(), "Arrival", "Aldis taught Rann.").unwrap();
        let mut book = load(root.path()).unwrap();
        let mut rann: Finding=serde_json::from_value(json!({"kind":"entry","area":"Characters","title":"Rann","evidence":"Aldis taught Rann.","detail":"An apprentice."})).unwrap();
        let canonical = insert_finding(&mut book, Some(section), rann.clone(), false);
        rann.fields.insert("role".into(), json!("Apprentice"));
        let pending = insert_finding(&mut book, Some(section), rann, true);
        let aldis: Finding=serde_json::from_value(json!({"kind":"entry","area":"Characters","title":"Aldis","detail":"Teacher.","evidence":"Aldis taught Rann.","fields":{"relationships":[{"name":"Rann","target_id":pending,"context":"Teaches her."}]}})).unwrap();
        insert_finding(&mut book, Some(section), aldis, true);
        assert_eq!(approve(root.path(), &mut book, 1).unwrap(), canonical);
        assert_eq!(book.records.len(), 2);
        assert_eq!(book.records[0].finding.fields["role"], "Apprentice");
        assert_eq!(
            book.records[1].finding.fields["relationships"][0]["target_id"],
            json!(canonical)
        );
    }

    #[test]
    fn folder_import_compiles_full_markdown_in_saved_order() {
        let input = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        fs::write(
            input.path().join("02-ford.md"),
            "# At the ford\n\nMara paid.",
        )
        .unwrap();
        fs::write(
            input.path().join("01-road.md"),
            "Mara walked.\n\n| A | B |\n|---|---|",
        )
        .unwrap();
        initialize(root.path(), Some("rules"), Some(input.path()), None).unwrap();
        let mut book = load(root.path()).unwrap();
        assert!(book.kind == Kind::Rulebook);
        let compiled = compile(root.path(), &book).unwrap();
        assert!(compiled.find("Mara walked.") < compiled.find("Mara paid."));
        assert!(compiled.contains("| A | B |"));
        book.sections.reverse();
        let reordered = compile(root.path(), &book).unwrap();
        assert!(reordered.find("Mara paid.") < reordered.find("Mara walked."));
        assert_eq!(
            fs::read_to_string(input.path().join("02-ford.md")).unwrap(),
            "# At the ford\n\nMara paid."
        );
    }

    #[test]
    fn invented_evidence_and_out_of_scope_areas_are_rejected() {
        let mut finding = Finding {
            fields: Values::new(),
            kind: FindingKind::Entry,
            area: "Characters".to_owned(),
            title: "Mara".to_owned(),
            detail: "Walks to the ford.".to_owned(),
            evidence: "Mara walked.".to_owned(),
        };
        assert!(validate_finding(&finding, Kind::Novel.areas(), "Mara walked.").is_ok());
        assert!(validate_finding(&finding, Kind::Rulebook.areas(), "Mara walked.").is_err());
        finding.evidence = "Mara ran.".to_owned();
        assert!(validate_finding(&finding, Kind::Novel.areas(), "Mara walked.").is_err());
    }
}
