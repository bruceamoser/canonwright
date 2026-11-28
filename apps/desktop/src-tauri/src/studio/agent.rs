use async_trait::async_trait;
use canonwright_governance::RiskLevel;
use canonwright_tools::{Tool, ToolContext, ToolDefinition, ToolError, ToolRegistry, ToolResult};
use serde_json::Value;

use super::*;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Focus {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) author_sources: Vec<author::Statement>,
    #[serde(default)]
    area_request: bool,
    pub project_id: String,
    #[serde(default)]
    pub(super) job_id: Option<Uuid>,
    pub(super) section_id: Option<Uuid>,
    selection: Option<String>,
    pub(super) record_id: Option<Uuid>,
    area: Option<String>,
}

impl Focus {
    pub(super) fn same_selection(&self, other: &Self) -> bool {
        self.area_request == other.area_request
            && self.author_sources == other.author_sources
            && self.project_id == other.project_id
            && self.section_id == other.section_id
            && self.record_id == other.record_id
            && self.area == other.area
            && self.selection == other.selection
    }
    pub(crate) fn book(project_id: String) -> Self {
        Self {
            project_id,
            ..Self::default()
        }
    }
}

const PROMPT: &str = include_str!("../../skills/chat/AGENTS.md");
pub(crate) fn prompt(root: &Path) -> String {
    let path = local(root, Path::new("AGENTS.md")).ok();
    let project = path.and_then(|path| fs::read_to_string(path).ok());
    match project {
        Some(project) => format!(
            "{PROMPT}\n\n# Book project guidance\n{}",
            project.chars().take(16000).collect::<String>()
        ),
        None => PROMPT.to_owned(),
    }
}

#[derive(Clone, Copy)]
enum Action {
    AssessText,
    AuthorFact,
    AuthorPassages,
    Research,
    CleanResearch,
    RequestAreas,
    AnswerQuestion,
    RequestWriter,
    EditCompanion,
    Dispatch,
    QueueControl,
    SaveSkill,
    ProcessHistory,
    Rollback,
    LocalIssue,
    QueueStatus,
    ResolveIssue,
    Editorial,
    FindEntries,
    ReadTemplate,
    SetTemplate,
    Area,
    #[cfg(test)]
    Record,
    #[cfg(test)]
    Decide,
    Outline,
    Read,
    Canon,
    ReadSkill,
    Finding,
    Build,
    PrepareEdition,
    PublishEdition,
    OutlineEdit,
    Add,
    Edit,
    Show,
    Review,
}

impl Action {
    const fn name(self) -> &'static str {
        match self {
            Self::CleanResearch => "book_clean_research",
            Self::AssessText => "book_assess_ai_text",
            Self::Research => "book_record_research",
            Self::RequestAreas => "book_request_canon_areas",
            Self::AuthorPassages => "book_author_passages",
            Self::AuthorFact => "book_record_author_fact",
            Self::AnswerQuestion => "book_answer_question",
            Self::RequestWriter => "book_request_writer",
            Self::EditCompanion => "book_edit_story_bible",
            Self::Dispatch => "book_queue_work",
            Self::QueueControl => "book_control_work",
            Self::ProcessHistory => "book_process_history",
            Self::Rollback => "book_rollback_process_change",
            Self::SaveSkill => "book_save_project_skill",
            Self::LocalIssue => "book_create_manuscript_issue",
            Self::QueueStatus => "book_queue_status",
            Self::ResolveIssue => "book_resolve_manuscript_issue",
            Self::Editorial => "book_editorial_review",
            Self::FindEntries => "book_find_entries",
            Self::ReadTemplate => "book_read_template",
            Self::SetTemplate => "book_set_template",
            Self::Area => "book_manage_area",
            #[cfg(test)]
            Self::Record => "book_manage_record",
            #[cfg(test)]
            Self::Decide => "book_decide_finding",
            Self::Outline => "book_outline",
            Self::Read => "book_read_section",
            Self::Canon => "book_canon",
            Self::ReadSkill => "book_read_skill",
            Self::Finding => "book_propose_finding",
            Self::Build => "book_build_manuscript",
            Self::PrepareEdition => "book_prepare_edition",
            Self::PublishEdition => "book_publish_edition",
            Self::OutlineEdit => "book_update_section_outline",
            Self::Add => "book_add_section",
            Self::Edit => "book_edit_section",
            Self::Show => "book_show",
            Self::Review => "book_review",
        }
    }
}

pub(crate) fn tools(focus: &Focus, active: &mut Vec<String>) -> ToolRegistry {
    role_tools("chat", focus, active)
}

pub(crate) fn chat_tools(focus: &Focus, active: &mut Vec<String>, full: bool) -> ToolRegistry {
    let mut registry = tools(focus, active);
    registry.enable_book_questions();
    if full {
        registry.enable_book_host_tools();
        registry.register(BookTool {
            action: Action::PrepareEdition,
            focus: focus.clone(),
        });
    }
    registry
}

pub(crate) fn role_tools(role: &str, focus: &Focus, active: &mut Vec<String>) -> ToolRegistry {
    use Action::*;
    let extra: &[Action] = match role {
        "chat" => &[
            AssessText,
            PublishEdition,
            AuthorPassages,
            Show,
            Dispatch,
            RequestAreas,
            Editorial,
            QueueControl,
        ],
        "writer" => &[
            AssessText,
            PrepareEdition,
            Add,
            Edit,
            OutlineEdit,
            Build,
            ResolveIssue,
            EditCompanion,
        ],
        "canon_keeper" => &[
            Finding,
            AuthorPassages,
            AuthorFact,
            Research,
            CleanResearch,
            RequestAreas,
            AnswerQuestion,
            RequestWriter,
        ],
        "editorial_reviewer" => &[AssessText, LocalIssue],
        "process_manager" if focus.area_request => &[Area, SetTemplate],
        "process_manager" => &[Area, SetTemplate, SaveSkill, Rollback],
        _ => &[],
    };
    let mut registry = ToolRegistry::with_book_reads();
    active.clear();
    for &action in [
        Outline,
        Read,
        ReadTemplate,
        FindEntries,
        Canon,
        Review,
        ReadSkill,
        ProcessHistory,
        QueueStatus,
    ]
    .iter()
    .chain(extra)
    {
        registry.register(BookTool {
            action,
            focus: focus.clone(),
        });
        active.push(action.name().into());
    }
    registry
}

pub(crate) fn reviewer_tools(focus: &Focus, active: &mut Vec<String>) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    active.clear();
    for action in [Action::Read, Action::Canon, Action::FindEntries] {
        active.push(action.name().into());
        registry.register(BookTool {
            action,
            focus: focus.clone(),
        });
    }
    registry
}

struct BookTool {
    action: Action,
    focus: Focus,
}

fn argument<'a>(args: &'a Value, name: &str) -> Result<&'a str, ToolError> {
    args.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidArgument(format!("Supply {name}")))
}

fn section(args: &Value, focus: &Focus) -> Result<Uuid, ToolError> {
    match args.get("section_id").and_then(Value::as_str) {
        Some(value) => value.parse().map_err(|_| {
            ToolError::InvalidArgument("Use a section ID from book_outline".to_owned())
        }),
        None => focus.section_id.ok_or_else(|| {
            ToolError::InvalidArgument(
                "No open section. Use book_outline to choose an ID".to_owned(),
            )
        }),
    }
}

fn as_argument(cause: String) -> ToolError {
    ToolError::InvalidArgument(cause)
}

fn manage_area(book: &mut Book, args: &Value) -> Result<Value, ToolError> {
    let mut names: Vec<String> = book.areas().into_iter().map(str::to_owned).collect();
    let name = argument(args, "area")?.trim();
    let operation = argument(args, "operation")?;
    let index = names.iter().position(|n| n.eq_ignore_ascii_case(name));
    let new = if operation == "rename" {
        argument(args, "new_name")?.trim()
    } else {
        name
    };
    if new.is_empty()
        || new.len() > 120
        || new.chars().any(char::is_control)
        || new.eq_ignore_ascii_case("Review")
    {
        return Err(as_argument(
            "Choose a short, single-line area name other than Review".into(),
        ));
    }
    if operation != "remove"
        && names.iter().enumerate().any(|(i, n)| {
            n.eq_ignore_ascii_case(new) && !(operation == "rename" && Some(i) == index)
        })
    {
        return Err(as_argument("Area already exists".into()));
    }
    if operation == "create" {
        names.push(new.into());
    } else {
        let i = index.ok_or_else(|| as_argument("Unknown area".into()))?;
        let old = names[i].clone();
        let old_template = book.template(&old);
        let destination = match operation {
            "rename" => {
                names[i] = new.into();
                Some(new.to_owned())
            }
            "remove" => {
                names.remove(i);
                match args.get("move_to").and_then(Value::as_str) {
                    Some(to) => Some(
                        names
                            .iter()
                            .find(|n| n.eq_ignore_ascii_case(to))
                            .ok_or_else(|| as_argument("Unknown destination area".into()))?
                            .clone(),
                    ),
                    None => None,
                }
            }
            _ => return Err(as_argument("Unknown area operation".into())),
        };
        book.templates.remove(&old);
        if operation == "rename" {
            book.templates.insert(new.to_owned(), old_template);
        }
        let target_template = destination.as_ref().map(|name| book.template(name));
        for record in book.records.iter_mut().filter(|r| r.finding.area == old) {
            if let Some(template) = &target_template {
                template
                    .check(&record.finding.fields)
                    .map_err(as_argument)?;
            }
            record.finding.area = destination.clone().ok_or_else(|| {
                as_argument(
                    "This area contains records. Supply move_to or remove those records first."
                        .into(),
                )
            })?;
        }
    }
    book.areas = Some(names);
    Ok(json!({"areas":book.areas()}))
}

pub(super) fn manage_record(
    root: &Path,
    book: &mut Book,
    args: &Value,
) -> Result<Value, ToolError> {
    let operation = argument(args, "operation")?;
    let index = if operation == "create" {
        None
    } else {
        let id: Uuid = argument(args, "record_id")?
            .parse()
            .map_err(|_| as_argument("Invalid record ID".into()))?;
        Some(
            book.records
                .iter()
                .position(|r| r.id == id)
                .ok_or_else(|| as_argument("Unknown record".into()))?,
        )
    };
    if operation == "answer" {
        let id = book.records[index.unwrap()].id;
        let id = answer_question(root, book, id, argument(args, "answer")?).map_err(as_argument)?;
        return Ok(json!({"record_id":id}));
    }
    if operation == "remove" {
        return Ok(json!({"removed":templates::remove_record(book,index.unwrap())}));
    }
    if !matches!(operation, "create" | "edit") {
        return Err(as_argument("Unknown record operation".into()));
    }
    let mut finding = index
        .map(|i| book.records[i].finding.clone())
        .unwrap_or(Finding {
            fields: Values::new(),
            kind: FindingKind::Entry,
            area: String::new(),
            title: String::new(),
            detail: String::new(),
            evidence: String::new(),
        });
    for (key, field) in [
        ("area", &mut finding.area),
        ("title", &mut finding.title),
        ("detail", &mut finding.detail),
    ] {
        if let Some(value) = args.get(key).and_then(Value::as_str) {
            *field = value.trim().to_owned();
        }
    }
    let mut section = index.and_then(|i| book.records[i].section);
    if args.get("section_id").is_some() || args.get("evidence").is_some() {
        section = Some(
            argument(args, "section_id")?
                .parse()
                .map_err(|_| as_argument("Invalid section ID".into()))?,
        );
        finding.evidence = argument(args, "evidence")?.to_owned();
    }
    if let Some(patch) = args.get("fields") {
        let patch = patch
            .as_object()
            .ok_or_else(|| as_argument("Supply a field object".into()))?;
        for (key, value) in patch {
            if value.is_null() {
                finding.fields.remove(key);
            } else {
                finding.fields.insert(key.clone(), value.clone());
            }
        }
    }
    // Earlier versions saved only a summary. Allow ordinary edits to those records until the author structures them.
    let legacy_edit = index.is_some_and(|i| book.records[i].finding.fields.is_empty())
        && args.get("fields").is_none()
        && finding.fields.is_empty();
    if !legacy_edit {
        check_entry(book, &finding).map_err(as_argument)?;
    }
    validate_fields(&finding, &book.areas()).map_err(as_argument)?;
    if let Some(id) = section {
        let source = read_section(root, book, id).map_err(as_argument)?;
        validate_finding(&finding, &book.areas(), &source.text).map_err(as_argument)?;
    }
    let id = if let Some(i) = index {
        book.records[i].finding = finding;
        book.records[i].section = section;
        book.records[i].id
    } else {
        insert_finding(book, section, finding, false)
    };
    Ok(json!({"record_id":id}))
}

#[async_trait]
impl Tool for BookTool {
    fn definition(&self) -> ToolDefinition {
        let string = json!({"type":"string"});
        let values = json!({"type":"object","description":"Use the stable keys returned by book_read_template, with text, string arrays, or links arrays of {name,context,target_id}. Omit target_id for unresolved mentions. Use book_find_entries to resolve names. On edit, null clears a field."});
        let (description, risk, properties, required) = match self.action {
            Action::RequestAreas => (
                "Queue the Process Manager to create, rename, remove or reshape canon areas for this book. Include the requested names, purpose and constraints. Empty areas are supported.",
                RiskLevel::Write,
                json!({"request":string}),
                vec!["request"],
            ),
            Action::CleanResearch => (
                "Clean specified research entries in place. Remove an exactly repeated opening paragraph and unresolved links with an unambiguous resolved twin of identical name and context. Preserves sources and unrelated entries.",
                RiskLevel::Write,
                json!({"record_ids":{"type":"array","items":string}}),
                vec!["record_ids"],
            ),
            Action::Research => (
                "Save a research-only entry to any existing canon area using its template. Provide a source locator or the origin of an unverified lead. Supply checked_source only when you actually consulted the source. Repeat the area and title to enrich the same entry.",
                RiskLevel::Write,
                json!({"area":string,"title":string,"detail":string,"fields":values,"source":string,"basis":{"enum":["checked_source","unverified_lead"]}}),
                vec!["area", "title", "detail", "fields", "source", "basis"],
            ),
            Action::AuthorPassages => (
                "Find exact saved author passages available in this chat or assignment, newest messages first. Search by a word or browse with offset. Cite supporting source_ref IDs for author facts.",
                RiskLevel::Read,
                json!({"query":string,"offset":{"type":"integer","minimum":0}}),
                vec![],
            ),
            Action::AuthorFact => (
                "Save an explicitly confirmed author fact to canon, even before drafting. Use source_refs from book_author_passages or exact quotes from focus.authorSources, use template keys and omit unknowns. Reuse the same title and area when adding links.",
                RiskLevel::Write,
                json!({"area":string,"title":string,"detail":string,"fields":values,"source_refs":{"type":"array","items":string},"quotes":{"type":"array","items":string}}),
                vec!["area", "title", "detail", "fields"],
            ),
            Action::AnswerQuestion => (
                "Resolve an existing question using the author's explicit answer from the queued request. Supply the exact answer excerpt.",
                RiskLevel::Write,
                json!({"record_id":string,"answer":string}),
                vec!["record_id", "answer"],
            ),
            Action::RequestWriter => (
                "Queue an authorized companion-file or manuscript edit for the Writer. Include the workspace-relative file, passage and exact requested change. Returns the child job ID.",
                RiskLevel::Write,
                json!({"request":string}),
                vec!["request"],
            ),
            Action::EditCompanion => (
                "Edit one unique passage in an existing story-bible Markdown file. Read it first. Supply the exact old text and replacement.",
                RiskLevel::Write,
                json!({"path":string,"old":string,"new":string}),
                vec!["path", "old", "new"],
            ),
            Action::Dispatch => (
                "Queue an authorized background task. For canon review of an existing proposal use book_editorial_review. Include source IDs, intended result and the author's constraints. Return the work ID. For author facts look up supporting text with book_author_passages and pass its source_refs; author_quotes also accepts literal excerpts.",
                RiskLevel::Write,
                json!({"role":{"enum":["writer","canon_keeper","editorial_reviewer","process_manager"]},"request":string,"source_refs":{"type":"array","items":string},"author_quotes":{"type":"array","items":string}}),
                vec!["role", "request"],
            ),
            Action::QueueControl => (
                "Cancel waiting/running work or explicitly retry a failed or cancelled job. Inspect any partial results before retry.",
                RiskLevel::Write,
                json!({"job_id":string,"operation":{"enum":["cancel","retry"]}}),
                vec!["job_id", "operation"],
            ),
            Action::ProcessHistory => (
                "Read project process versions, assigned roles, test results and rollback IDs. Optionally filter by target.",
                RiskLevel::Read,
                json!({"target":string}),
                vec![],
            ),
            Action::Rollback => (
                "Stage a lossless rollback of a currently applied process version by change_id.",
                RiskLevel::Write,
                json!({"change_id":string}),
                vec!["change_id"],
            ),
            Action::SaveSkill => (
                "Stage a Markdown skill and its behavioral examples for isolated testing and publication. To update read the skill with inspect=true, then supply its stable skill_id and fingerprint.",
                RiskLevel::Write,
                json!({"title":string,"instructions":string,"skill_id":string,"fingerprint":string,"role":{"enum":["writer","canon_keeper","canon_reviewer","editorial_reviewer","process_manager"]},"examples":{"type":"array","items":{"type":"object","properties":{"input":string,"required":{"type":"array","items":string},"forbidden":{"type":"array","items":string}},"required":["input","required"]}}}),
                vec!["title", "instructions", "role", "examples"],
            ),
            Action::LocalIssue => (
                "Record a manuscript issue with exact evidence in its source section and actionable completion criteria.",
                RiskLevel::Write,
                json!({"section_id":string,"title":string,"evidence":string,"criteria":string}),
                vec!["section_id", "title", "evidence", "criteria"],
            ),
            Action::QueueStatus => (
                "Read work counts and recent jobs for this book, or inspect receipts and the report for one job_id.",
                RiskLevel::Read,
                json!({"job_id":string,"state":string}),
                vec![],
            ),
            Action::ResolveIssue => (
                "Submit a Writer fix for independent verification and closure after editing and checking the cited section. Supply its current revision, exact revised passage and resolution.",
                RiskLevel::Write,
                json!({"record_id":string,"revision":string,"passage":string,"resolution":string}),
                vec!["record_id", "revision", "passage", "resolution"],
            ),
            Action::Editorial => (
                "Queue canon work and return immediately. unchecked requests first reviews. decided executes saved verdicts. all routes each finding by its existing state. A repeated request reuses the job.",
                RiskLevel::Write,
                json!({"record_id":string,"scope":{"enum":["unchecked","decided","all"]}}),
                vec![],
            ),
            Action::FindEntries => (
                "Find link targets in canon and pending review. Search a title or alias or list an area. Returns stable IDs, status and summaries. With links_to, return entries referencing that ID.",
                RiskLevel::Read,
                json!({"query":string,"area":string,"links_to":string}),
                vec![],
            ),
            Action::ReadTemplate => (
                "Read the current area template with stable keys, field types, writing guidance and revision.",
                RiskLevel::Read,
                json!({"area":string}),
                vec!["area"],
            ),
            Action::SetTemplate => (
                "Stage a template for a new or existing area. Supply the current revision (0 for a new area) and full field list. renames maps old keys to new keys. Removed fields retain their data and links.",
                RiskLevel::Write,
                json!({"area":string,"renames":{"type":"object","additionalProperties":{"type":"string"}},"revision":{"type":"integer","minimum":0},"fields":{"type":"array","items":{"type":"object","properties":{"key":string,"label":string,"guidance":string,"format":{"enum":["text","list","links"]}},"required":["key","label","guidance","format"],"additionalProperties":false}}}),
                vec!["area", "revision", "fields"],
            ),
            Action::Area => (
                "Create, rename or remove a reference area. Renaming moves its records. To remove a populated area supply an existing destination area as move_to.",
                RiskLevel::Write,
                json!({"operation":{"enum":["create","rename","remove"]},"area":string,"new_name":string,"move_to":string}),
                vec!["operation", "area"],
            ),
            #[cfg(test)]
            Action::Record => (
                "Create an author-directed canon entry, edit or move a saved record by ID, remove it, or answer a question with the author’s response. For manuscript extraction use book_propose_finding. Author-created entries need no source. To change citations supply section_id and exact evidence together.",
                RiskLevel::Write,
                json!({"fields":values,"operation":{"enum":["create","edit","remove","answer"]},"answer":string,"record_id":string,"area":string,"title":string,"detail":string,"section_id":string,"evidence":string}),
                vec!["operation"],
            ),
            #[cfg(test)]
            Action::Decide => (
                "Accept or dismiss a pending review item by ID at the author's request.",
                RiskLevel::Write,
                json!({"record_id":string,"decision":{"enum":["accept","dismiss"]}}),
                vec!["record_id", "decision"],
            ),
            Action::AssessText => (
                "Quick screens passages using Desklib. For generation use strict: Desklib >=0.8 plus Falcon Binoculars confirmation. Legacy follow_up accepts GPT-2 or Falcon agreement and can overflag human rules. Experimental fast_detectgpt ranks all eligible windows by GPT-Neo/GPT-J analytic discrepancy. Experimental editlens ranks all eligible windows for AI editing intensity using optional noncommercial weights. Neither experimental mode has a validated cutoff. Report mode, coverage, unavailable checks, offsets and exact excerpt. No score proves authorship or the percentage written by AI. Use section_id, text or omit both for the book. Page with next_offset.",
                RiskLevel::Read,
                json!({"text":string,"section_id":string,"mode":{"enum":["quick","follow_up","strict","fast_detectgpt","editlens"]},"threshold":{"type":"number","minimum":0,"maximum":1},"offset":{"type":"integer","minimum":0}}),
                vec![],
            ),
            Action::Outline => (
                "Read the book type, ordered manuscript sections, current selection, reference areas, and available writing skills.",
                RiskLevel::Read,
                json!({}),
                vec![],
            ),
            Action::Read => (
                "Read up to 12000 characters of a manuscript section. Omit section_id for the open section. Use next_offset to read more, or query to find an exact phrase and up to six surrounding excerpts without paging. The result includes the revision for targeted edits.",
                RiskLevel::Read,
                json!({"section_id":string,"query":string,"offset":{"type":"integer","minimum":0}}),
                vec![],
            ),
            Action::Canon => (
                "Read accepted reference entries and answered author questions. Filter by reference area or phrase when useful.",
                RiskLevel::Read,
                json!({"area":string,"query":string}),
                vec![],
            ),
            Action::ReadSkill => (
                "Read the exact instructions of a writing skill. Use a skill ID from book_outline before extracting, performing a specialized review, or creating a skill.",
                RiskLevel::Read,
                json!({"skill_id":string,"inspect":{"type":"boolean"}}),
                vec!["skill_id"],
            ),
            Action::Finding => (
                "Save a cited entry, note, or question to the durable review queue. Returns its ID.",
                RiskLevel::Write,
                json!({"fields":values,"section_id":string,"kind":{"enum":["entry","note","question"]},"area":string,"title":string,"detail":string,"evidence":string}),
                vec!["kind", "area", "title", "detail", "evidence"],
            ),
            Action::Build => (
                "Compile the entire manuscript in the current outline order and save the generated Markdown file.",
                RiskLevel::Write,
                json!({}),
                vec![],
            ),
            Action::PrepareEdition => (
                "Export a named edition to Markdown and a 6 by 9 inch PDF under editions/. Choose section_ids for a sampler. Omitting IDs includes the complete outline. Requires Pandoc and Typst.",
                RiskLevel::Write,
                json!({"edition":string,"title":string,"author":string,"section_ids":{"type":"array","items":string}}),
                vec!["edition", "title"],
            ),
            Action::PublishEdition => (
                "Perform one authorized publishing step for the active book. Commit includes only the named edition PDF and Markdown. For GitHub actions supply account (personal or work) and exact owner/repo. New repositories are private. Supply tag, title and notes for release. Inspect origin and confirm the author's intended destination and selection.",
                RiskLevel::Network,
                json!({"operation":{"enum":["status","commit","create_private_repo","push","release"]},"edition":string,"message":string,"account":{"enum":["personal","work"]},"repository":string,"tag":string,"title":string,"notes":string}),
                vec!["operation"],
            ),
            Action::OutlineEdit => (
                "Rename or move an existing section, preserving its ID, text and links. Read book_outline. Supply title to rename, and/or before_id or after_id to move relative to another section.",
                RiskLevel::Write,
                json!({"section_id":string,"title":string,"before_id":string,"after_id":string}),
                vec!["section_id"],
            ),
            Action::Add => (
                "Add a new manuscript section with the given title and optional Markdown body.",
                RiskLevel::Write,
                json!({"title":string,"text":string}),
                vec!["title"],
            ),
            Action::Show => (
                "Show the complete manuscript, a section, or a reference entry when the author asks to see it. Use manuscript or an ID from book_outline or book_canon.",
                RiskLevel::Read,
                json!({"target":string}),
                vec!["target"],
            ),
            Action::Review => (
                "Read review items. Use compact=true and status=revise or ask_author to find outstanding corrections and author decisions. Supply record_id to inspect a full finding.",
                RiskLevel::Read,
                json!({"compact":{"type":"boolean"},"record_id":string,"status":{"enum":["unchecked","revise","ask_author","source_conflict","referral_closed","manuscript_issue","issue_resolved"]}}),
                vec![],
            ),
            Action::Edit => (
                "Write the first body of an empty section with old set to an empty string, or replace one exact unique passage in an existing section. Read the section first and pass the current revision to avoid overwriting newer edits.",
                RiskLevel::Write,
                json!({"section_id":string,"revision":string,"old":string,"new":string}),
                vec!["revision", "old", "new"],
            ),
        };
        ToolDefinition::new(self.action.name(), description, risk).with_schema(json!({
            "type":"object","properties":properties,"required":required,"additionalProperties":false
        }))
    }

    async fn execute(&self, args: Value, context: &ToolContext) -> Result<ToolResult, ToolError> {
        let Some(id) = self.focus.job_id.filter(|_| {
            matches!(
                self.action,
                Action::SaveSkill
                    | Action::RequestAreas
                    | Action::CleanResearch
                    | Action::Research
                    | Action::AuthorFact
                    | Action::AnswerQuestion
                    | Action::RequestWriter
                    | Action::EditCompanion
                    | Action::Rollback
                    | Action::LocalIssue
                    | Action::SetTemplate
                    | Action::Area
                    | Action::Finding
                    | Action::PrepareEdition
                    | Action::OutlineEdit
                    | Action::Add
                    | Action::Edit
                    | Action::Build
                    | Action::ResolveIssue
            )
        }) else {
            return self.perform(args, context).await;
        };
        let root = context.workspace.root();
        let tool = self.action.name();
        review_queue::update_job(root, id, |j| {
            if j.repair_target.is_some() && !matches!(self.action, Action::SaveSkill) {
                return Err("This repair can only revise the staged skill".into());
            }
            if matches!(j.editorial, Some(editorial::Assignment::Repair { .. }))
                && !matches!(self.action, Action::Edit | Action::ResolveIssue)
            {
                return Err(
                    "Issue assignments may edit the cited section and submit verification".into(),
                );
            }
            if j.state != "running" {
                return Err("Work is no longer running".into());
            }
            if matches!(j.editorial, Some(editorial::Assignment::Repair { .. }))
                && j.result_id.is_some()
                && !matches!(self.action, Action::ResolveIssue)
            {
                return Err("This fix has already been submitted for verification".into());
            }
            j.write_in_progress = Some(tool.into());
            Ok(())
        })
        .map_err(as_argument)?;
        let result = self.perform(args, context).await;
        review_queue::update_job(root,id,|j|{
            j.write_in_progress=None;
            let receipt=match &result{
                Ok(r)=>json!({"tool":tool,"ok":!r.is_error,"result":r.content.chars().take(2500).collect::<String>()}),
                Err(e)=>json!({"tool":tool,"ok":false,"error":e.to_string()})
            };
            j.receipts.push(receipt); Ok(())
        }).map_err(as_argument)?;
        result
    }
}

impl BookTool {
    async fn perform(&self, args: Value, context: &ToolContext) -> Result<ToolResult, ToolError> {
        let root = context.workspace.root();
        let book = load(root).map_err(ToolError::Failed)?;
        match self.action {
            Action::RequestWriter => {
                let id =
                    review_queue::submit(root, "writer", argument(&args, "request")?, &self.focus)
                        .map_err(as_argument)?;
                review_queue::wake(root, context)?;
                Ok(ToolResult::ok(json!({"writer_job":id}).to_string()))
            }
            Action::AnswerQuestion => {
                let answer = argument(&args, "answer")?;
                let id = argument(&args, "record_id")?
                    .parse()
                    .map_err(|_| as_argument("Invalid question ID".into()))?;
                let _guard = BOOK_IO.lock().map_err(|e| ToolError::Failed(error(e)))?;
                let mut book = load(root).map_err(as_argument)?;
                let authorized = book
                    .review_jobs
                    .iter()
                    .find(|j| Some(j.id) == self.focus.job_id && j.state == "running")
                    .and_then(|j| j.work.as_ref())
                    .is_some_and(|w| w.role == "canon_keeper" && w.request.contains(answer));
                if !authorized {
                    return Err(as_argument(
                        "Use the author's exact answer included in this Keeper assignment.".into(),
                    ));
                }
                let saved = answer_question(root, &mut book, id, answer).map_err(as_argument)?;
                save(root, &book).map_err(as_argument)?;
                Ok(ToolResult::ok(json!({"answered":saved}).to_string()))
            }
            Action::EditCompanion => {
                let relative = Path::new(argument(&args, "path")?);
                if !relative.starts_with("story-bible")
                    || relative.extension().and_then(|s| s.to_str()) != Some("md")
                {
                    return Err(as_argument(
                        "Choose an existing story-bible Markdown file".into(),
                    ));
                }
                let path = local(root, relative).map_err(as_argument)?;
                let old = argument(&args, "old")?;
                let new = args["new"]
                    .as_str()
                    .ok_or_else(|| as_argument("Supply replacement text".into()))?;
                let _guard = BOOK_IO.lock().map_err(|e| ToolError::Failed(error(e)))?;
                let text = fs::read_to_string(&path).map_err(|e| as_argument(error(e)))?;
                if old.is_empty() || text.matches(old).count() != 1 {
                    return Err(ToolError::Stale(
                        "Read the current file and provide one unique passage".into(),
                    ));
                }
                write(&path, &text.replacen(old, new, 1)).map_err(as_argument)?;
                Ok(ToolResult::ok(json!({"edited":relative}).to_string()))
            }
            Action::CleanResearch => {
                let ids: Vec<Uuid> = serde_json::from_value(args["record_ids"].clone())
                    .map_err(|e| as_argument(error(e)))?;
                let receipt = author::clean_research(
                    root,
                    self.focus
                        .job_id
                        .ok_or_else(|| as_argument("Requires a queued Keeper".into()))?,
                    &ids,
                )
                .map_err(as_argument)?;
                Ok(ToolResult::ok(receipt.to_string())
                    .with_metadata(json!({"book_records_changed":true})))
            }
            Action::AuthorPassages => Ok(ToolResult::ok(
                author::list_passages(
                    &self.focus.author_sources,
                    args["query"].as_str().unwrap_or(""),
                    args["offset"].as_u64().unwrap_or(0) as usize,
                )
                .to_string(),
            )),
            Action::AuthorFact | Action::Research => {
                let research = matches!(self.action, Action::Research);
                let id = author::save_entry(
                    root,
                    self.focus
                        .job_id
                        .ok_or_else(|| as_argument("Requires a queued Keeper".into()))?,
                    &args,
                    research,
                )
                .map_err(as_argument)?;
                Ok(ToolResult::ok(
                    json!({"record_id":id,"provenance":if research {"research"} else {"author_statement"}}).to_string(),
                )
                .with_metadata(json!({"book_records_changed":true})))
            }
            Action::RequestAreas | Action::Dispatch => {
                let area_request = matches!(self.action, Action::RequestAreas);
                let mut focus = self.focus.clone();
                focus.area_request = area_request;
                focus.author_sources = if area_request
                    || (args["source_refs"].is_null() && args["author_quotes"].is_null())
                {
                    vec![]
                } else {
                    if args["role"] != "canon_keeper" {
                        return Err(as_argument(
                            "Author fact assignments belong to the Canon Keeper".into(),
                        ));
                    }
                    author::select_sources(&self.focus.author_sources, &args, "author_quotes")
                        .map_err(as_argument)?
                };
                let id = review_queue::submit(
                    root,
                    if area_request {
                        "process_manager"
                    } else {
                        argument(&args, "role")?
                    },
                    argument(&args, "request")?,
                    &focus,
                )
                .map_err(as_argument)?;
                review_queue::wake(root, context)?;
                Ok(
                    ToolResult::ok(json!({"job_id":id,"queued":true}).to_string())
                        .with_metadata(json!({"book_records_changed":true})),
                )
            }
            Action::QueueControl => {
                let id = argument(&args, "job_id")?
                    .parse()
                    .map_err(|_| as_argument("Invalid job ID".into()))?;
                review_queue::control(root, id, argument(&args, "operation")?)
                    .map_err(as_argument)?;
                review_queue::wake(root, context)?;
                Ok(ToolResult::ok("Queue updated.")
                    .with_metadata(json!({"book_records_changed":true})))
            }
            Action::ProcessHistory => Ok(ToolResult::ok(
                process::history(&book, args["target"].as_str()).to_string(),
            )),
            Action::SaveSkill | Action::Rollback => {
                let job = self
                    .focus
                    .job_id
                    .ok_or_else(|| as_argument("Requires a Process Manager job".into()))?;
                let result = process::stage(
                    root,
                    job,
                    if matches!(self.action, Action::SaveSkill) {
                        "skill"
                    } else {
                        "rollback"
                    },
                    &args,
                )
                .map_err(as_argument)?;
                Ok(ToolResult::ok(result.to_string()))
            }
            Action::LocalIssue => {
                let section = section(&args, &self.focus)?;
                let finding = Finding {
                    area: "Review".into(),
                    kind: FindingKind::Note,
                    title: argument(&args, "title")?.into(),
                    detail: argument(&args, "criteria")?.into(),
                    evidence: argument(&args, "evidence")?.into(),
                    fields: Values::new(),
                };
                let _guard = BOOK_IO.lock().map_err(|e| ToolError::Failed(error(e)))?;
                let mut book = load(root).map_err(as_argument)?;
                let src = read_section(root, &book, section).map_err(as_argument)?;
                validate_finding(&finding, &book.areas(), &src.text).map_err(as_argument)?;
                if let Some(record) = book.records.iter().find(|r| {
                    r.section == Some(section)
                        && r.finding == finding
                        && r.editorial
                            .as_ref()
                            .is_some_and(|v| v.decision == review::Decision::ManuscriptIssue)
                }) {
                    let id = record.id;
                    if let Some(parent) = self.focus.job_id {
                        editorial::assign_writer(&mut book, parent, id, 1, "")
                            .map_err(as_argument)?;
                        save(root, &book).map_err(as_argument)?;
                    }
                    return Ok(ToolResult::ok(json!({"issue_id":id}).to_string()));
                }
                let verdict=serde_json::from_value(json!({"decision":"manuscript_issue","reason":finding.detail,"source_revision":src.revision})).map_err(|e|as_argument(error(e)))?;
                let id = insert_finding(&mut book, Some(section), finding, true);
                book.records.last_mut().unwrap().editorial = Some(verdict);
                if let Some(parent) = self.focus.job_id {
                    editorial::assign_writer(&mut book, parent, id, 1, "").map_err(as_argument)?;
                }
                save(root, &book).map_err(as_argument)?;
                Ok(ToolResult::ok(json!({"issue_id":id}).to_string()))
            }
            Action::QueueStatus => {
                let mut counts = std::collections::BTreeMap::new();
                for job in &book.review_jobs {
                    *counts.entry(&job.state).or_insert(0usize) += 1;
                }
                let id = args["job_id"].as_str();
                let mut jobs: Vec<_> = book
                    .review_jobs
                    .iter()
                    .rev()
                    .filter(|j| {
                        id.is_none_or(|id| j.id.to_string() == id)
                            && args["state"].as_str().is_none_or(|state| state == j.state)
                    })
                    .collect();
                jobs.sort_by_key(|j| std::cmp::Reverse(j.updated_at));
                let matched = jobs.len();
                let jobs:Vec<_>=jobs.into_iter().take(30).map(|j|{
                    let record=book.records.iter().find(|r|r.id==j.id || Some(r.id)==j.result_id);
                    let role=review_queue::job_role(&book,j);
                    let mut row=json!({"id":j.id,"role":role,"task":j.work.as_ref().map(|w|&w.request).or_else(||j.sync.as_ref().and_then(|s|book.sections.iter().find(|sec|sec.id==s.section).map(|sec|&sec.title))).or_else(||j.instructions.as_ref()).or_else(||record.map(|r|&r.finding.title)),"state":j.state,"attempts":j.attempts,"error":j.error,"result_id":j.result_id,"parent_job":j.parent_job});
                    if id.is_some(){row["section_change"]=json!(j.sync.as_ref().map(|s|s.section));row["report"]=json!(j.report);row["receipts"]=json!(j.receipts);}
                    row
                }).collect();
                Ok(ToolResult::ok(
                    json!({"counts":counts,"matched":matched,"jobs":jobs}).to_string(),
                ))
            }
            Action::FindEntries => {
                let query = args["query"].as_str().unwrap_or("").to_lowercase();
                let area = args["area"].as_str();
                let target = args["links_to"].as_str();
                let entries: Vec<_>=book.records.iter().filter(|r| r.finding.kind==FindingKind::Entry &&
                    area.is_none_or(|a| r.finding.area.eq_ignore_ascii_case(a)) &&
                    (r.finding.title.to_lowercase().contains(&query) || r.finding.fields.get("aliases").is_some_and(|v| v.to_string().to_lowercase().contains(&query))) &&
                    target.is_none_or(|id| r.finding.fields.values().chain(r.retained.iter().map(|f|&f.value)).filter_map(Value::as_array).flatten().any(|item| item.get("target_id").and_then(Value::as_str)==Some(id))))
                    .map(|r|json!({"id":r.id,"title":r.finding.title,"area":r.finding.area,"pending":r.pending,"summary":r.finding.detail})).collect();
                Ok(ToolResult::ok(json!(entries).to_string()))
            }
            Action::ReadTemplate => {
                let area = argument(&args, "area")?;
                if !book.areas().contains(&area)
                    && !self.focus.job_id.is_some_and(|id| {
                        book.review_jobs.iter().any(|job| {
                            job.id == id
                                && job
                                    .work
                                    .as_ref()
                                    .is_some_and(|w| w.role == "process_manager")
                        })
                    })
                {
                    return Err(as_argument("Unknown area. Use book_outline.".into()));
                }
                Ok(ToolResult::ok(json!(book.template(area)).to_string()))
            }
            Action::AssessText => {
                let path = root.to_owned();
                let report = crate::off_thread(move || detector::assess(&path, &book, &args))
                    .await
                    .map_err(ToolError::Failed)?;
                Ok(ToolResult::ok(report))
            }
            Action::Outline => {
                let skills = skill_catalog(root, &book).map_err(ToolError::Failed)?;
                Ok(ToolResult::ok(
                    json!({"book_type":book.kind,"sections":book.sections,
                    "active_section":self.focus.section_id,"active_area":self.focus.area,"active_record_id":self.focus.record_id,"selected_passage":self.focus.selection.as_ref().map(|text| text.chars().take(12000).collect::<String>()),
                    "manuscript_repo_root":root,"reference_areas":book.areas(),"skills":skills})
                    .to_string(),
                ))
            }
            Action::Read => {
                let id = section(&args, &self.focus)?;
                let source = read_section(root, &book, id).map_err(as_argument)?;
                let offset = args
                    .get("offset")
                    .and_then(Value::as_u64)
                    .and_then(|value| usize::try_from(value).ok())
                    .unwrap_or(0);
                if let Some(query) = args.get("query").and_then(Value::as_str) {
                    if query.trim().is_empty() || query.len() > 1800 {
                        return Err(as_argument(
                            "Provide a short non-empty source phrase.".into(),
                        ));
                    }
                    let byte = source
                        .text
                        .char_indices()
                        .nth(offset)
                        .map(|(i, _)| i)
                        .unwrap_or(source.text.len());
                    let mut hits = source.text[byte..].match_indices(query);
                    let matches:Vec<_>=hits.by_ref().take(6).map(|(at,_)|{
                        let start=at+byte;
                        let before=&source.text[..start];
                        let context_start=before.char_indices().rev().nth(299).map(|(i,_)|i).unwrap_or(0);
                        let after=&source.text[start+query.len()..];
                        let end=after.char_indices().nth(300).map(|(i,_)|i).unwrap_or(after.len());
                        json!({"offset":before.chars().count(),"text":&source.text[context_start..start+query.len()+end]})
                    }).collect();
                    let next = hits
                        .next()
                        .map(|(at, _)| source.text[..byte + at].chars().count());
                    return Ok(ToolResult::ok(json!({"section_id":id,"revision":source.revision,"matches":matches,"next_offset":next}).to_string()));
                }
                let mut chars = source.text.chars().skip(offset);
                let text: String = chars.by_ref().take(12000).collect();
                let next_offset = chars.next().map(|_| offset + 12000);
                Ok(ToolResult::ok(
                    json!({"section_id":id,"revision":source.revision,
                    "text":text,"next_offset":next_offset})
                    .to_string(),
                ))
            }
            Action::Canon => {
                let area = args.get("area").and_then(Value::as_str);
                let query = args
                    .get("query")
                    .and_then(Value::as_str)
                    .map(str::to_lowercase);
                let records: Vec<_> = book
                    .records
                    .iter()
                    .rev()
                    .filter(|record| {
                        !record.pending
                            && (record.finding.kind != FindingKind::Question
                                || record.answer.is_some())
                            && area
                                .is_none_or(|value| record.finding.area.eq_ignore_ascii_case(value))
                            && query.as_ref().is_none_or(|query| {
                                record.finding.title.to_lowercase().contains(query)
                                    || record.finding.detail.to_lowercase().contains(query)
                                    || record
                                        .finding
                                        .fields
                                        .values()
                                        .any(|v| v.to_string().to_lowercase().contains(query))
                            })
                    })
                    .collect();
                Ok(ToolResult::ok(
                    serde_json::to_string(&records)
                        .map_err(|cause| ToolError::Failed(error(cause)))?,
                ))
            }
            Action::ReadSkill => {
                let id = argument(&args, "skill_id")?;
                let doc = process::read_skill(root, &book, id)
                    .map_err(as_argument)?
                    .ok_or_else(|| as_argument("Unknown skill ID".into()))?;
                if args["inspect"] == true {
                    return Ok(ToolResult::ok(
                        json!({"id":id,"fingerprint":doc.fingerprint(),"skill":doc}).to_string(),
                    ));
                }
                Ok(ToolResult::ok(doc.markdown))
            }
            Action::Finding => {
                let id = section(&args, &self.focus)?;
                let finding: Finding =
                    serde_json::from_value(args).map_err(|cause| as_argument(error(cause)))?;
                let (record_id, pending) = {
                    let _guard = BOOK_IO.lock().map_err(|e| ToolError::Failed(error(e)))?;
                    let mut book = load(root).map_err(ToolError::Failed)?;
                    let source = read_section(root, &book, id).map_err(as_argument)?;
                    check_entry(&book, &finding).map_err(as_argument)?;
                    validate_finding(&finding, &book.areas(), &source.text).map_err(as_argument)?;
                    let existing = book
                        .records
                        .iter()
                        .find(|r| r.section == Some(id) && r.finding == finding);
                    let (record_id, pending) = if let Some(record) = existing {
                        (record.id, record.pending)
                    } else {
                        let record_id = insert_finding(&mut book, Some(id), finding, true);
                        (record_id, true)
                    };
                    if pending {
                        review_queue::queue_records(&mut book, Some(record_id), "all");
                        if let Some(job) = book.review_jobs.iter_mut().find(|j| j.id == record_id) {
                            if job.parent_job.is_none() {
                                job.parent_job = self.focus.job_id;
                            }
                        }
                        save(root, &book).map_err(ToolError::Failed)?;
                    }
                    (record_id, pending)
                };
                if pending {
                    review_queue::wake(root, context)?;
                }
                Ok(
                    ToolResult::ok(json!({"record_id":record_id,"pending":pending}).to_string())
                        .with_metadata(json!({"book_records_changed":true})),
                )
            }
            Action::ResolveIssue => {
                let id = argument(&args, "record_id")?
                    .parse()
                    .map_err(|_| as_argument("Invalid issue ID".into()))?;
                let worker = self
                    .focus
                    .job_id
                    .ok_or_else(|| as_argument("Submit from the assigned Writer job".into()))?;
                let verification = editorial::submit_fix(
                    root,
                    worker,
                    id,
                    argument(&args, "revision")?,
                    argument(&args, "passage")?,
                    argument(&args, "resolution")?,
                )
                .map_err(as_argument)?;
                Ok(ToolResult::ok(
                    json!({"verification_job":verification,"issue":"open"}).to_string(),
                )
                .with_metadata(json!({"book_records_changed":true})))
            }
            Action::Editorial => {
                let id = args
                    .get("record_id")
                    .map(|v| serde_json::from_value(v.clone()))
                    .transpose()
                    .map_err(|e| as_argument(error(e)))?;
                let ids = review_queue::enqueue(root, id, args["scope"].as_str().unwrap_or("all"))
                    .map_err(as_argument)?;
                review_queue::wake(root, context)?;
                Ok(ToolResult::ok(json!({"queued_ids":ids}).to_string())
                    .with_metadata(json!({"book_records_changed":true})))
            }
            Action::Area if self.focus.job_id.is_some() && args["operation"] == "create" => {
                let area = argument(&args, "area")?;
                if book.areas().contains(&area) {
                    return Err(as_argument("Area already exists".into()));
                }
                let template = book.template(area);
                let result = process::stage(
                    root,
                    self.focus.job_id.unwrap(),
                    "template",
                    &json!({"area":area,"revision":template.revision,"fields":template.fields}),
                )
                .map_err(as_argument)?;
                Ok(ToolResult::ok(result.to_string()))
            }
            Action::SetTemplate if self.focus.job_id.is_some() => {
                let result = process::stage(root, self.focus.job_id.unwrap(), "template", &args)
                    .map_err(as_argument)?;
                Ok(ToolResult::ok(result.to_string()))
            }
            Action::Area | Action::SetTemplate => {
                let _guard = BOOK_IO.lock().map_err(|e| ToolError::Failed(error(e)))?;
                let mut book = load(root).map_err(ToolError::Failed)?;
                let result = match self.action {
                    Action::SetTemplate => {
                        let area = argument(&args, "area")?;
                        if !book.areas().contains(&area) {
                            return Err(as_argument(
                                "Create the area first with book_manage_area.".into(),
                            ));
                        }
                        let revision = book.template(area).revision;
                        if args["revision"].as_u64() != Some(revision) {
                            return Err(ToolError::Stale(
                                "Read the current template before updating.".into(),
                            ));
                        }
                        let template = templates::Template {
                            revision: revision + 1,
                            fields: serde_json::from_value(args["fields"].clone())
                                .map_err(|e| as_argument(error(e)))?,
                        };
                        template.validate().map_err(as_argument)?;
                        for record in book.records.iter().filter(|r| r.finding.area == area) {
                            template.check(&record.finding.fields).map_err(|e| {
                                as_argument(format!(
                                    "Existing entry '{}': {e}",
                                    record.finding.title
                                ))
                            })?;
                        }
                        let result = json!(&template);
                        book.templates.insert(area.to_owned(), template);
                        result
                    }
                    Action::Area => manage_area(&mut book, &args)?,
                    _ => unreachable!(),
                };
                save(root, &book).map_err(ToolError::Failed)?;
                Ok(ToolResult::ok(result.to_string())
                    .with_metadata(json!({"book_records_changed":true})))
            }
            #[cfg(test)]
            Action::Record => {
                let _guard = BOOK_IO.lock().map_err(|e| ToolError::Failed(error(e)))?;
                let mut book = load(root).map_err(as_argument)?;
                let result = manage_record(root, &mut book, &args)?;
                save(root, &book).map_err(as_argument)?;
                Ok(ToolResult::ok(result.to_string()))
            }
            #[cfg(test)]
            Action::Decide => {
                let id = argument(&args, "record_id")?
                    .parse()
                    .map_err(|_| as_argument("Invalid record ID".into()))?;
                let accept = match argument(&args, "decision")? {
                    "accept" => true,
                    "dismiss" => false,
                    _ => return Err(as_argument("Unknown decision".into())),
                };
                decide(root, id, accept).map_err(as_argument)?;
                Ok(ToolResult::ok("Review updated")
                    .with_metadata(json!({"book_records_changed":true})))
            }
            Action::Build => {
                let manuscript = build_at(root).map_err(ToolError::Failed)?;
                Ok(ToolResult::ok(format!("Built {}.", manuscript.path))
                    .with_metadata(json!({"book_built":true})))
            }
            Action::PrepareEdition | Action::PublishEdition => {
                let prepare = matches!(self.action, Action::PrepareEdition);
                let path = root.to_owned();
                let receipt = crate::off_thread(move || {
                    if prepare {
                        edition::prepare(&path, &args)
                    } else {
                        edition::publish(&path, &args)
                    }
                })
                .await
                .map_err(as_argument)?;
                Ok(ToolResult::ok(receipt.to_string()))
            }
            Action::OutlineEdit => {
                let id = section(&args, &self.focus)?;
                let anchor = |key: &str| {
                    args.get(key)
                        .map(|v| {
                            serde_json::from_value::<Uuid>(v.clone())
                                .map_err(|e| as_argument(error(e)))
                        })
                        .transpose()
                };
                update_outline_section(
                    root,
                    id,
                    args.get("title")
                        .map(|_| argument(&args, "title"))
                        .transpose()?,
                    anchor("before_id")?,
                    anchor("after_id")?,
                )
                .map_err(as_argument)?;
                Ok(ToolResult::ok(json!({"updated_section":id}).to_string())
                    .with_metadata(json!({"book_changed":true})))
            }
            Action::Add => {
                let title = argument(&args, "title")?;
                let text = args.get("text").and_then(Value::as_str).unwrap_or_default();
                let id = add_at_with_parent(root, title, text, self.focus.job_id)
                    .map_err(as_argument)?;
                Ok(ToolResult::ok(format!("Added section {title} ({id})."))
                    .with_metadata(json!({"book_changed":true})))
            }
            Action::Review => {
                let review: Vec<_> = book
                    .records
                    .iter()
                    .rev()
                    .filter(|record| {
                        record.pending
                            || record.finding.area == "Review"
                            || (record.finding.kind == FindingKind::Question
                                && record.answer.is_none())
                    })
                    .filter(|r|args["record_id"].as_str().is_none_or(|id|r.id.to_string()==id))
                    .filter(|r| match args["status"].as_str() {
                        Some("unchecked") => r.pending && r.editorial.is_none(),
                        Some("revise") => r.pending && r.editorial.as_ref().is_some_and(|v|v.decision==review::Decision::Revise),
                        Some("manuscript_issue") => r.editorial.as_ref().is_some_and(|v|v.decision==review::Decision::ManuscriptIssue),
                        Some("source_conflict")=>r.editorial.as_ref().is_some_and(|v|v.decision==review::Decision::SourceConflict),
                        Some("referral_closed")=>r.editorial.as_ref().is_some_and(|v|v.decision==review::Decision::ReferralClosed),
                        Some("issue_resolved") => r.editorial.as_ref().is_some_and(|v|v.decision==review::Decision::IssueResolved),
                        Some("ask_author") => r.pending && r.editorial.as_ref().is_some_and(|v|v.decision==review::Decision::AskAuthor),
                        _=>true
                    })
                    .map(|r| if args["compact"]==true { json!({"id":r.id,"title":r.finding.title,"area":r.finding.area,"editorial":r.editorial}) } else {json!(r)})
                    .collect();
                Ok(ToolResult::ok(
                    json!({"repository":book.repository,"review":review}).to_string(),
                ))
            }
            Action::Show => {
                let target = argument(&args, "target")?;
                if target == "manuscript" {
                    if book.sections.is_empty() {
                        return Err(as_argument(
                            "The manuscript has no sections yet.".to_owned(),
                        ));
                    }
                    return Ok(ToolResult::ok("Opened manuscript preview.")
                        .with_metadata(json!({"book_show":"manuscript"})));
                }
                let id = target
                    .parse::<Uuid>()
                    .map_err(|_| as_argument("Use a section or reference ID.".to_owned()))?;
                if book.sections.iter().any(|section| section.id == id) {
                    return Ok(
                        ToolResult::ok("Opened section.").with_metadata(json!({"book_show":id}))
                    );
                }
                let record = book
                    .records
                    .iter()
                    .find(|record| record.id == id)
                    .ok_or_else(|| {
                        as_argument("Section or reference entry not found.".to_owned())
                    })?;
                Ok(ToolResult::ok("Opened reference entry.")
                    .with_metadata(json!({"book_reference":{"id":id,"area":record.finding.area,"review":record.pending || record.finding.area == "Review" || (record.finding.kind == FindingKind::Question && record.answer.is_none())}})))
            }
            Action::Edit => {
                let id = section(&args, &self.focus)?;
                if let Some(job) = self
                    .focus
                    .job_id
                    .and_then(|id| book.review_jobs.iter().find(|j| j.id == id))
                    && matches!(job.editorial, Some(editorial::Assignment::Repair { .. }))
                    && Some(id) != self.focus.section_id
                {
                    return Err(as_argument("This issue targets a different section".into()));
                }
                let revision = argument(&args, "revision")?;
                let old = argument(&args, "old")?;
                let new = argument(&args, "new")?;
                let source = read_section(root, &book, id).map_err(as_argument)?;
                if revision != source.revision {
                    return Err(ToolError::Stale(
                        "Read the current section before revising.".to_owned(),
                    ));
                }
                if (old.is_empty() && !source.text.is_empty())
                    || (!old.is_empty() && source.text.match_indices(old).take(2).count() != 1)
                {
                    return Err(as_argument(
                        "The old passage must occur exactly once.".to_owned(),
                    ));
                }
                let updated = source.text.replacen(old, new, 1);
                let job = chapter_sync::save_edit(root, id, revision, &updated, self.focus.job_id)
                    .map_err(as_argument)?;
                Ok(ToolResult::ok(json!({"section_id":id,"canon_sync_job":job,"revision":hash_bytes(updated.as_bytes())}).to_string())
                    .with_metadata(json!({"book_changed":true})))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canonwright_workspace::WorkspaceService;
    use std::sync::Arc;

    #[tokio::test]
    async fn findings_survive_reopen_and_follow_the_author_decision() {
        let root = tempfile::tempdir().unwrap();
        initialize(root.path(), Some("novel"), None, None).unwrap();
        let section = add_at(root.path(), "First chapter", "Mara crossed the ford.").unwrap();
        let focus = Focus {
            area_request: false,
            author_sources: vec![],
            job_id: None,
            project_id: "test".into(),
            section_id: Some(section),
            selection: None,
            record_id: None,
            area: None,
        };
        let context = ToolContext::new(Arc::new(WorkspaceService::new(root.path())));
        let run = |action| BookTool {
            action,
            focus: focus.clone(),
        };
        let finding = json!({"kind":"entry","area":"Characters","title":"Mara","fields":{"role":"Traveler"},"detail":"A traveler.","evidence":"Mara crossed the ford."});
        let initial = run(Action::ReadTemplate)
            .execute(json!({"area":"Characters"}), &context)
            .await
            .unwrap();
        let mut spec: Value = serde_json::from_str(&initial.content).unwrap();
        assert!(
            spec["fields"]
                .as_array()
                .unwrap()
                .iter()
                .any(|field| field["key"] == "motivation")
        );
        spec["fields"].as_array_mut().unwrap().push(json!({"key":"promises","label":"Promises","format":"list","guidance":"Promises made or broken."}));
        spec["area"] = json!("Characters");
        run(Action::SetTemplate)
            .execute(spec.clone(), &context)
            .await
            .unwrap();
        assert!(
            run(Action::SetTemplate)
                .execute(spec, &context)
                .await
                .is_err()
        );
        for fields in [
            json!({}),
            json!({"invented":"text"}),
            json!({"promises":"not a list"}),
        ] {
            let mut invalid = finding.clone();
            invalid["fields"] = fields;
            assert!(
                run(Action::Finding)
                    .execute(invalid, &context)
                    .await
                    .is_err()
            );
        }
        let saved = run(Action::Finding)
            .execute(finding.clone(), &context)
            .await
            .unwrap();
        let same = run(Action::Finding)
            .execute(finding, &context)
            .await
            .unwrap();
        assert_eq!(saved.content, same.content);
        let book = load(root.path()).unwrap();
        assert_eq!(book.records.len(), 1);
        assert!(book.records[0].pending);
        let id = book.records[0].id;
        assert_eq!(
            run(Action::Canon)
                .execute(json!({}), &context)
                .await
                .unwrap()
                .content,
            "[]"
        );
        let review = run(Action::Review)
            .execute(json!({}), &context)
            .await
            .unwrap();
        assert!(review.content.contains(&id.to_string()));
        run(Action::Decide)
            .execute(json!({"record_id":id,"decision":"accept"}), &context)
            .await
            .unwrap();
        run(Action::Area)
            .execute(
                json!({"operation":"rename","area":"Characters","new_name":"Travelers"}),
                &context,
            )
            .await
            .unwrap();
        assert!(
            run(Action::Area)
                .execute(json!({"operation":"remove","area":"Travelers"}), &context)
                .await
                .is_err()
        );
        run(Action::Record)
            .execute(
                json!({"operation":"edit","record_id":id,"detail":"She travels on foot."}),
                &context,
            )
            .await
            .unwrap();
        assert!(
            run(Action::Canon)
                .execute(json!({"area":"Travelers"}), &context)
                .await
                .unwrap()
                .content
                .contains("on foot")
        );
        run(Action::Area)
            .execute(json!({"operation":"create","area":"People"}), &context)
            .await
            .unwrap();
        run(Action::Area)
            .execute(
                json!({"operation":"remove","area":"Travelers","move_to":"People"}),
                &context,
            )
            .await
            .unwrap();
        run(Action::Record).execute(json!({"operation":"create","area":"People","title":"Sam","detail":"The ferryman.","fields":{"role":"Ferryman","connections":[{"target_id":id,"name":"Mara","context":"carries her across the ford"}]}}), &context).await.unwrap();
        let incoming = run(Action::FindEntries)
            .execute(json!({"links_to":id}), &context)
            .await
            .unwrap();
        assert!(incoming.content.contains("Sam"));
        run(Action::Record)
            .execute(
                json!({"operation":"edit","record_id":id,"title":"Mara of the ford"}),
                &context,
            )
            .await
            .unwrap();
        assert!(run(Action::Record).execute(json!({"operation":"edit","record_id":id,"fields":{"connections":[{"target_id":Uuid::now_v7(),"name":"Unknown","context":"teacher"}]}}), &context).await.is_err());
        run(Action::Record)
            .execute(json!({"operation":"remove","record_id":id}), &context)
            .await
            .unwrap();
        let book = load(root.path()).unwrap();
        assert_eq!(book.records.len(), 1);
        assert_eq!(book.records[0].finding.title, "Sam");
        assert!(book.records[0].section.is_none());
        let link = &book.records[0].finding.fields["connections"][0];
        assert_eq!(link["name"], "Mara of the ford");
        assert!(link.get("target_id").is_none());
        assert_eq!(link["context"], "carries her across the ford");
    }

    #[tokio::test]
    async fn book_tools_extend_the_engine_and_open_records_by_id() {
        let root = tempfile::tempdir().unwrap();
        initialize(root.path(), Some("narrative"), None, None).unwrap();
        let mut book = load(root.path()).unwrap();
        let section_id = book.sections[0].id;
        let record_id = Uuid::now_v7();
        book.records.push(Record {
            research_sources: vec![],
            author_sources: vec![],
            retained: vec![],
            unverified_sections: vec![],
            citations: vec![],
            id: record_id,
            section: Some(section_id),
            pending: false,
            editorial: None,
            finding: Finding {
                fields: Values::new(),
                kind: FindingKind::Entry,
                area: "Locations".to_owned(),
                title: "The crossing".to_owned(),
                detail: "A ford.".to_owned(),
                evidence: "The crossing was shallow.".to_owned(),
            },
            github_url: None,
            answer: None,
        });
        save(root.path(), &book).unwrap();
        let focus = Focus {
            area_request: false,
            author_sources: vec![],
            job_id: None,
            project_id: Uuid::now_v7().to_string(),
            section_id: None,
            selection: None,
            record_id: None,
            area: None,
        };
        let mut active = canonwright_tools::HOT_TOOLS
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        let registry = tools(&focus, &mut active);
        let advertised = registry.advertised(&active);
        for denied in [
            "tool_search",
            "run_command",
            "write_file",
            "book_edit_section",
            "book_manage_record",
            "mcp_call",
            "spawn_subagent",
        ] {
            assert!(registry.definition_for(denied, &json!({})).is_none());
        }
        for allowed in [
            "book_request_canon_areas",
            "book_queue_work",
            "book_queue_status",
            "book_control_work",
        ] {
            assert!(advertised.iter().any(|t| t.name == allowed));
        }
        for full in [false, true] {
            let mut names = Vec::new();
            let chat = chat_tools(&focus, &mut names, full);
            let exposed = chat.advertised(&names);
            for tool in ["ask_question", "list_questions", "book_assess_ai_text"] {
                assert!(exposed.iter().any(|t| t.name == tool));
            }
            assert_eq!(exposed.iter().any(|t| t.name == "run_command"), full);
        }
        let area_focus = Focus {
            area_request: true,
            ..focus.clone()
        };
        let manager = role_tools("process_manager", &area_focus, &mut Vec::new());
        assert!(
            manager
                .definition_for("book_set_template", &json!({}))
                .is_some()
        );
        assert!(
            manager
                .definition_for("book_save_project_skill", &json!({}))
                .is_none()
        );
        let keeper = role_tools("canon_keeper", &focus, &mut Vec::new());
        assert!(
            keeper
                .definition_for("book_record_research", &json!({}))
                .is_some()
        );
        assert!(
            keeper
                .definition_for("book_request_canon_areas", &json!({}))
                .is_some()
        );
        assert!(
            registry
                .definition_for("book_record_research", &json!({}))
                .is_none()
        );
        assert!(advertised.iter().any(|tool| tool.name == "read_file"));
        assert!(advertised.iter().any(|tool| tool.name == "book_show"));
        let ctx = ToolContext::new(Arc::new(WorkspaceService::new(root.path().to_owned())));
        let request = BookTool {
            action: Action::RequestAreas,
            focus: focus.clone(),
        };
        let args = json!({"request":"Create separate Folklore and People areas"});
        let queued = request.execute(args.clone(), &ctx).await.unwrap();
        let repeated = request.execute(args, &ctx).await.unwrap();
        assert_eq!(queued.content, repeated.content);
        let book = load(root.path()).unwrap();
        let assignment = book.review_jobs.last().unwrap().work.as_ref().unwrap();
        assert_eq!(assignment.role, "process_manager");
        assert!(assignment.focus.area_request);
        let open = BookTool {
            action: Action::Show,
            focus,
        };
        let result = open
            .execute(json!({"target":record_id}), &ctx)
            .await
            .unwrap();
        assert_eq!(
            result.metadata["book_reference"],
            json!({"id":record_id,"area":"Locations","review":false})
        );
    }
    #[tokio::test]
    async fn chat_can_dispatch_and_keeper_can_cite_the_same_saved_passage() {
        let root = tempfile::tempdir().unwrap();
        initialize(root.path(), Some("novel"), None, None).unwrap();
        let ctx = ToolContext::new(Arc::new(WorkspaceService::new(root.path().to_owned())));
        let mut focus = Focus::book("test".into());
        focus.author_sources.push(author::Statement {
            session: "session".into(),
            message: 1,
            quote: "Al’s her father. Other notes.".into(),
        });
        let read = BookTool {
            action: Action::AuthorPassages,
            focus: focus.clone(),
        };
        let result = read.execute(json!({"query":"father"}), &ctx).await.unwrap();
        let sources: Value = serde_json::from_str(&result.content).unwrap();
        let refs = json!([sources["passages"][0]["source_ref"]]);
        let chat = BookTool {
            action: Action::Dispatch,
            focus,
        };
        chat.execute(
            json!({"role":"canon_keeper","request":"Save the relationship","source_refs":refs}),
            &ctx,
        )
        .await
        .unwrap();
        let book = load(root.path()).unwrap();
        let job = &book.review_jobs[0];
        let work = job.work.as_ref().unwrap();
        assert_eq!(work.focus.author_sources.len(), 1);
        let keeper = BookTool {
            action: Action::AuthorFact,
            focus: work.focus.clone(),
        };
        review_queue::update_job(root.path(), job.id, |j| {
            j.state = "running".into();
            Ok(())
        })
        .unwrap();
        keeper.execute(json!({"area":"Characters","title":"Al","detail":"Her father.","fields":{"role":"Father"},"source_refs":refs}), &ctx).await.unwrap();
        let record = &load(root.path()).unwrap().records[0];
        assert_eq!(record.author_sources[0].quote, "Al’s her father.");
        assert_eq!(record.author_sources[0].message, 1);
    }

    #[tokio::test]
    async fn writer_fills_moves_and_renames_existing_chapters() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        initialize(root, Some("novel"), None, None).unwrap();
        let first = load(root).unwrap().sections[0].id;
        let prologue = add_at(root, "Opening", "First scene.").unwrap();
        let mut focus = Focus::book("book".into());
        let id = review_queue::submit(
            root,
            "writer",
            "Fill first chapter and move opening",
            &focus,
        )
        .unwrap();
        review_queue::update_job(root, id, |j| {
            j.state = "running".into();
            Ok(())
        })
        .unwrap();
        focus.job_id = Some(id);
        let ctx = ToolContext::new(Arc::new(WorkspaceService::new(root.to_owned())));
        let edit = BookTool {
            action: Action::Edit,
            focus: focus.clone(),
        };
        let outline = BookTool {
            action: Action::OutlineEdit,
            focus,
        };
        let args =
            json!({"section_id":first,"revision":hash_bytes(b""),"old":"","new":"Second scene."});
        edit.execute(args.clone(), &ctx).await.unwrap();
        assert!(edit.execute(args, &ctx).await.is_err());
        assert!(edit.execute(json!({"section_id":first,"revision":hash_bytes(b"Second scene."),"old":"","new":"Overwrite"}),&ctx).await.is_err());
        assert!(
            outline
                .execute(
                    json!({"section_id":prologue,"before_id":prologue,"title":"Wrong"}),
                    &ctx
                )
                .await
                .is_err()
        );
        outline
            .execute(
                json!({"section_id":prologue,"before_id":first,"title":"Prologue"}),
                &ctx,
            )
            .await
            .unwrap();
        let book = load(root).unwrap();
        assert_eq!(book.sections[0].id, prologue);
        assert_eq!(book.sections[0].title, "Prologue");
        let manuscript = compile(root, &book).unwrap();
        assert!(
            manuscript.find("First scene.").unwrap() < manuscript.find("Second scene.").unwrap()
        );
        assert!(
            book.review_jobs
                .iter()
                .any(|j| j.parent_job == Some(id) && j.sync.is_some())
        );
    }
}
