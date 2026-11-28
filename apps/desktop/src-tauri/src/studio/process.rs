//! Changes to process definitions share the same atomic manifest publication as canon migration.
//! Each published Markdown revision has its own SKILL.md. Abandoned revision files are inactive.
use super::*;
use canonwright_tools::ToolContext;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use templates::{Field, Format, Template};

const TEST: &str = include_str!("../../skills/process-manager/test-skill.md");
pub(crate) fn directive(task: &str) -> Option<&'static str> {
    task.starts_with(TEST).then_some(TEST)
}
const ROLES: [&str; 5] = [
    "writer",
    "canon_keeper",
    "canon_reviewer",
    "editorial_reviewer",
    "process_manager",
];

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Retained {
    version: Uuid,
    pub key: String,
    pub label: String,
    pub value: Value,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct SkillDoc {
    pub title: String,
    pub role: Option<String>,
    pub markdown: String,
}
impl SkillDoc {
    pub fn fingerprint(&self) -> String {
        hash_bytes(json!(self).to_string().as_bytes())
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Example {
    input: String,
    required: Vec<String>,
    #[serde(default)]
    forbidden: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Definition {
    Template {
        before: Template,
        after: Template,
        renames: BTreeMap<String, String>,
        had_area: bool,
    },
    Skill {
        before: Option<SkillDoc>,
        after: Option<SkillDoc>,
    },
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Version {
    id: Uuid,
    target: String,
    job: Uuid,
    created: i64,
    definition: Definition,
    tests: Value,
    undoes: Option<Uuid>,
    undone_by: Option<Uuid>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Draft {
    id: Uuid,
    target: String,
    baseline: String,
    definition: Definition,
    examples: Vec<Example>,
    undoes: Option<Uuid>,
}

impl Draft {
    pub(super) fn target(&self) -> &str {
        &self.target
    }
    pub(super) fn matches_failure(&self, error: &str) -> bool {
        let Definition::Skill {
            after: Some(doc), ..
        } = &self.definition
        else {
            return false;
        };
        let message = error.strip_prefix("tool failed: ").unwrap_or(error);
        message.starts_with(&format!("{} example ", doc.title))
            && (message.contains(" failed: missing ")
                || message.contains(" failed: returned forbidden "))
            && message.ends_with("Live version preserved.")
    }
}
fn skill_file(root: &Path, version: Uuid) -> Result<PathBuf, String> {
    local(
        root,
        Path::new(&format!(".canonwright/skill-versions/{version}/SKILL.md")),
    )
}
fn latest<'a>(book: &'a Book, target: &str, skill: bool) -> Option<&'a Version> {
    book.process_history
        .iter()
        .rev()
        .find(|v| v.target == target && matches!(v.definition, Definition::Skill { .. }) == skill)
}
pub(super) fn read_skill(root: &Path, book: &Book, id: &str) -> Result<Option<SkillDoc>, String> {
    if let Some(v) = latest(book, id, true)
        && let Definition::Skill { after, .. } = &v.definition
    {
        if let Some(doc) = after {
            let file = fs::read_to_string(skill_file(root, v.id)?).map_err(error)?;
            if file != doc.markdown {
                return Err("Published skill file differs from its tested version".into());
            }
        }
        return Ok(after.clone());
    }
    if id == "create-skill" {
        return Ok(Some(SkillDoc {
            title: "Create skill".into(),
            role: None,
            markdown: include_str!("../../skills/create-skill/SKILL.md").into(),
        }));
    }
    if let Some(s) = SKILLS.iter().find(|s| s.id == id && available(book, s)) {
        return Ok(Some(SkillDoc {
            title: s.title.into(),
            role: None,
            markdown: s.instructions.into(),
        }));
    }
    let Ok(uuid) = id.parse() else {
        return Ok(None);
    };
    let path = custom_path(root, uuid)?;
    match fs::read_to_string(path) {
        Ok(markdown) => Ok(Some(SkillDoc {
            title: markdown
                .lines()
                .find_map(|l| l.strip_prefix("# "))
                .unwrap_or(id)
                .into(),
            role: None,
            markdown,
        })),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(error(e)),
    }
}
pub(super) fn merge_catalog(book: &Book, skills: &mut Vec<SkillCard>) {
    for v in &book.process_history {
        if let Definition::Skill { after, .. } = &v.definition {
            skills.retain(|s| s.id != v.target);
            if let Some(doc) = after {
                skills.push(SkillCard {
                    id: v.target.clone(),
                    title: doc.title.clone(),
                });
            }
        }
    }
}
pub(crate) fn role_guidance(root: &Path, role: &str) -> String {
    let mut result = if role == "process_manager" {
        format!(
            "\n{}",
            include_str!("../../skills/process-manager/AGENTS.md")
        )
    } else {
        String::new()
    };
    if role == "canon_keeper" {
        result.push_str("\nFor requested research records, inspect book_outline and book_read_template, then use book_record_research with a specific source locator. For recollections or leads without a consulted source, identify the origin and use unverified_lead. Research is distinct from manuscript evidence and author direction. Verify the saved ID with book_canon. If the requested area is absent use book_request_canon_areas and report its queued job.");
        result.push_str("\nFor premise work read book_author_passages and use its supporting source_refs with book_record_author_fact. The host copies the exact evidence. Summarize the supported fact in your own words. Only record facts the author stated; leave alternatives, suggestions and undecided details open. Omit unknown fields and empty lists. Create link targets first and use their returned IDs. For an author-supplied answer use book_answer_question on the question ID. For requested edits to story-bible Markdown or the manuscript, create a Writer child job with book_request_writer. A canon finding is never a file-edit assignment. Report the returned job ID.");
    }
    if let Ok(book) = load(root) {
        let mut seen = BTreeSet::new();
        for v in book.process_history.iter().rev() {
            if let Definition::Skill { after, .. } = &v.definition
                && seen.insert(&v.target)
                && let Some(doc) = after
                && doc.role.as_deref() == Some(role)
            {
                result.push_str(&format!(
                    "\nProject skill: {} ({})\n{}",
                    doc.title, v.target, doc.markdown
                ));
            }
        }
    }
    result
}

pub(super) fn history(book: &Book, target: Option<&str>) -> Value {
    json!(book.process_history.iter().rev().filter(|v|target.is_none_or(|t|v.target==t)).map(|v|{
        let skill=matches!(v.definition,Definition::Skill{..});
        let head=book.process_history.iter().rev().find(|entry|entry.target==v.target && matches!(entry.definition,Definition::Skill{..})==skill && entry.undoes.is_none() && entry.undone_by.is_none());
        let resource=match &v.definition{
            Definition::Skill{after,..}=>json!({"kind":"skill","title":after.as_ref().map(|d|&d.title),"role":after.as_ref().and_then(|d|d.role.as_ref())}),
            Definition::Template{..}=>json!({"kind":"template","area":v.target})
        };
        json!({"change_id":v.id,"target":v.target,"job_id":v.job,"created_at":v.created,"undoes":v.undoes,"undone_by":v.undone_by,"tests":v.tests,"resource":resource,"kind":if skill{"skill"}else{"template"},"can_rollback":head.is_some_and(|h|h.id==v.id)})
    }).collect::<Vec<_>>())
}

fn baseline(root: &Path, book: &Book, target: &str, skill: bool) -> Result<String, String> {
    let definition = if skill {
        json!(read_skill(root, book, target)?)
    } else {
        json!([book.areas().contains(&target), book.template(target)])
    };
    Ok(hash_bytes(
        json!([definition, latest(book, target, skill).map(|v| v.id)])
            .to_string()
            .as_bytes(),
    ))
}
fn required<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v[key]
        .as_str()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| format!("Supply {key}"))
}
fn tests_valid(examples: &[Example]) -> Result<(), String> {
    if !(1..=8).contains(&examples.len()) {
        return Err("Supply 1–8 behavioral examples".into());
    }
    for e in examples {
        if e.input.trim().is_empty()
            || e.input.len() > 12000
            || e.required.is_empty()
            || e.required.len() + e.forbidden.len() > 32
            || e.required
                .iter()
                .chain(&e.forbidden)
                .any(|s| s.trim().is_empty() || s.len() > 1000)
        {
            return Err("Each example needs input and nonempty, bounded output assertions".into());
        }
    }
    Ok(())
}
fn template_draft(book: &Book, args: &Value) -> Result<(String, Definition), String> {
    let area = required(args, "area")?.trim();
    if area.len() > 120
        || area.chars().any(char::is_control)
        || area.eq_ignore_ascii_case("Review")
        || book
            .areas()
            .iter()
            .any(|a| a.eq_ignore_ascii_case(area) && *a != area)
    {
        return Err("Invalid or ambiguous area name".into());
    }
    let before = book.template(area);
    if args["revision"].as_u64() != Some(before.revision) {
        return Err("Template revision changed; read it again".into());
    }
    let after = Template {
        revision: before.revision + 1,
        fields: serde_json::from_value(args["fields"].clone()).map_err(error)?,
    };
    after.validate()?;
    let renames: BTreeMap<String, String> = if args["renames"].is_null() {
        BTreeMap::new()
    } else {
        serde_json::from_value(args["renames"].clone()).map_err(error)?
    };
    let mut outputs = BTreeSet::new();
    for (src, dest) in &renames {
        if !before.fields.iter().any(|f| f.key == *src)
            || !after.fields.iter().any(|f| f.key == *dest)
        {
            return Err(format!("Unknown field in rename {src} → {dest}"));
        }
    }
    for field in &before.fields {
        let key = renames.get(&field.key).unwrap_or(&field.key);
        if after.fields.iter().any(|f| &f.key == key) && !outputs.insert(key) {
            return Err(format!("Two fields would map to {key}"));
        }
    }
    Ok((
        area.into(),
        Definition::Template {
            before,
            after,
            renames,
            had_area: book.areas().contains(&area),
        },
    ))
}
fn inverse(v: &Version) -> Definition {
    match &v.definition {
        Definition::Skill { before, after } => Definition::Skill {
            before: after.clone(),
            after: before.clone(),
        },
        Definition::Template {
            before,
            after,
            renames,
            had_area,
        } => Definition::Template {
            before: after.clone(),
            after: before.clone(),
            had_area: *had_area,
            renames: renames
                .iter()
                .map(|(a, b)| (b.clone(), a.clone()))
                .collect(),
        },
    }
}
pub(super) fn stage(root: &Path, job: Uuid, kind: &str, args: &Value) -> Result<Value, String> {
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(root)?;
    let index = book
        .review_jobs
        .iter()
        .position(|j| {
            j.id == job
                && j.state == "running"
                && j.work.as_ref().is_some_and(|w| w.role == "process_manager")
        })
        .ok_or("Requires an active Process Manager job")?;
    if let Some(target) = book.review_jobs[index].repair_target.clone() {
        if kind != "skill" {
            return Err("Repair the failed staged skill before changing other resources".into());
        }
        let current_baseline = baseline(root, &book, &target, true)?;
        let current_doc = read_skill(root, &book, &target)?;
        let draft = book.review_jobs[index]
            .process_drafts
            .iter_mut()
            .find(|d| d.target == target)
            .ok_or("Missing staged skill")?;
        let Definition::Skill {
            before,
            after: Some(doc),
        } = &mut draft.definition
        else {
            return Err("Expected a staged skill".into());
        };
        let skill = SkillDraft {
            title: required(args, "title")?.into(),
            instructions: required(args, "instructions")?.into(),
        };
        validate_skill(&skill)?;
        if doc.title != skill.title || doc.role.as_deref() != args["role"].as_str() {
            return Err("Keep the staged title and role".into());
        }
        let markdown = format!(
            "---\nname: {target}\n---\n\n# {}\n\n{}\n",
            doc.title,
            skill.instructions.trim()
        );
        if doc.markdown == markdown {
            return Err("Revise the staged instructions to address the failed example".into());
        }
        if current_baseline != draft.baseline {
            if current_doc
                .as_ref()
                .is_none_or(|d| Some(d.fingerprint().as_str()) != args["fingerprint"].as_str())
            {
                return Err("The live skill changed. Read it with inspect=true, integrate its current instructions and supply its fingerprint.".into());
            }
            draft.baseline = current_baseline;
            *before = current_doc;
        }
        doc.markdown = markdown;
        let attempt = book.review_jobs[index].process_repairs;
        book.review_jobs[index]
            .receipts
            .push(json!({"corrected_attempt":attempt,"target":target}));
        save(root, &book)?;
        return Ok(json!({"repaired":target,"original_examples_retained":true}));
    }
    let id = Uuid::now_v7();
    let mut examples = vec![];
    let mut undoes = None;
    let (target, definition) = match kind {
        "template" => template_draft(&book, args)?,
        "skill" => {
            let skill = SkillDraft {
                title: required(args, "title")?.into(),
                instructions: required(args, "instructions")?.into(),
            };
            validate_skill(&skill)?;
            let role = required(args, "role")?;
            if !ROLES.contains(&role) {
                return Err("Unknown agent role".into());
            }
            examples = serde_json::from_value(args["examples"].clone()).map_err(error)?;
            tests_valid(&examples)?;
            let target = args["skill_id"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| id.to_string());
            let before = read_skill(root, &book, &target)?;
            if args["skill_id"].is_string()
                && before
                    .as_ref()
                    .is_none_or(|d| Some(d.fingerprint().as_str()) != args["fingerprint"].as_str())
            {
                return Err("Read the existing skill and supply its current fingerprint".into());
            }
            let markdown = format!(
                "---\nname: {target}\n---\n\n# {}\n\n{}\n",
                skill.title.trim(),
                skill.instructions.trim()
            );
            let doc = SkillDoc {
                title: skill.title.trim().into(),
                role: Some(role.into()),
                markdown,
            };
            (
                target,
                Definition::Skill {
                    before,
                    after: Some(doc),
                },
            )
        }
        "rollback" => {
            let change = required(args, "change_id")?
                .parse::<Uuid>()
                .map_err(error)?;
            let v = book
                .process_history
                .iter()
                .find(|v| v.id == change)
                .ok_or("Unknown process version")?;
            let skill = matches!(v.definition, Definition::Skill { .. });
            let head = book.process_history.iter().rev().find(|entry| {
                entry.target == v.target
                    && matches!(entry.definition, Definition::Skill { .. }) == skill
                    && entry.undoes.is_none()
                    && entry.undone_by.is_none()
            });
            if head.is_none_or(|h| h.id != change) {
                return Err("Roll back the latest active change for this resource first".into());
            }
            if let Definition::Template { after, .. } = &v.definition
                && json!(book.template(&v.target).fields) != json!(after.fields)
            {
                return Err("Template has diverged from the published version".into());
            }
            undoes = Some(change);
            (v.target.clone(), inverse(v))
        }
        _ => return Err("Unknown process change".into()),
    };
    let skill = matches!(definition, Definition::Skill { .. });
    let draft = Draft {
        id,
        baseline: baseline(root, &book, &target, skill)?,
        target: target.clone(),
        definition,
        examples,
        undoes,
    };
    let preview = match &draft.definition {
        Definition::Template { .. } => migrate(&mut book.clone(), &draft)?,
        Definition::Skill { .. } => {
            json!({"examples":draft.examples.len(),"restoring_tested_version":undoes.is_some()})
        }
    };
    let drafts = &mut book.review_jobs[index].process_drafts;
    drafts.retain(|d| {
        d.target != target || matches!(d.definition, Definition::Skill { .. }) != skill
    });
    drafts.push(draft);
    save(root, &book)?;
    Ok(json!({"staged":id,"target":target,"preview":preview}))
}

fn link_values(r: &Record) -> Vec<String> {
    let mut links: Vec<_> = r
        .finding
        .fields
        .values()
        .chain(r.retained.iter().map(|f| &f.value))
        .filter_map(Value::as_array)
        .flatten()
        .filter(|v| v.is_object())
        .map(Value::to_string)
        .collect();
    links.sort();
    links
}

fn convert(value: Value, field: &Field) -> Result<Value, String> {
    match (&field.format, value) {
        (Format::List, v @ Value::String(_)) => Ok(json!([v])),
        (Format::Text, Value::Array(mut items)) if items.len() == 1 && items[0].is_string() => {
            Ok(items.remove(0))
        }
        (_, v) => Ok(v),
    }
}
fn migrate(book: &mut Book, draft: &Draft) -> Result<Value, String> {
    let Definition::Template {
        before,
        after,
        renames,
        had_area,
    } = &draft.definition
    else {
        return Err("Expected a template".into());
    };
    let mut next = after.clone();
    next.revision = book.template(&draft.target).revision + 1;
    let mut changed = 0;
    let mut checked = 0;
    let mut links = 0;
    let mut retained = 0;
    for record in book
        .records
        .iter_mut()
        .filter(|r| r.finding.area == draft.target)
    {
        checked += 1;
        let original_links = link_values(record);
        links += original_links.len();
        let initial = json!([record.finding.fields, record.retained]);
        let mut fields = Values::new();
        for (key, value) in std::mem::take(&mut record.finding.fields) {
            let dest = renames.get(&key).unwrap_or(&key);
            if let Some(field) = next.fields.iter().find(|f| &f.key == dest) {
                let converted = convert(value.clone(), field)?;
                if next
                    .check(&Values::from([(dest.clone(), converted.clone())]))
                    .is_ok()
                {
                    if fields.insert(dest.clone(), converted).is_some() {
                        return Err(format!("Field collision on {}", record.finding.title));
                    }
                    continue;
                }
                if draft.undoes.is_none() {
                    return Err(format!(
                        "{}: field {key} cannot be converted losslessly. Retire it or keep its format.",
                        record.finding.title
                    ));
                }
            }
            let label = before
                .fields
                .iter()
                .find(|f| f.key == key)
                .map(|f| f.label.clone())
                .unwrap_or_else(|| key.clone());
            record.retained.push(Retained {
                version: draft.id,
                key,
                label,
                value,
            });
            retained += 1;
        }
        if let Some(undone) = draft.undoes {
            record.retained.retain(|r| {
                if r.version == undone
                    && !fields.contains_key(&r.key)
                    && next
                        .check(&Values::from([(r.key.clone(), r.value.clone())]))
                        .is_ok()
                {
                    fields.insert(r.key.clone(), r.value.clone());
                    false
                } else {
                    true
                }
            });
        }
        next.check(&fields)?;
        record.finding.fields = fields;
        if original_links != link_values(record) {
            return Err("Migration changed a relationship or link target".into());
        }
        changed += usize::from(initial != json!([record.finding.fields, record.retained]));
    }
    let mut names: Vec<String> = book.areas().into_iter().map(str::to_owned).collect();
    if draft.undoes.is_some() && !had_area && checked == 0 {
        names.retain(|n| n != &draft.target);
        book.templates.remove(&draft.target);
    } else {
        if !names.contains(&draft.target) {
            names.push(draft.target.clone());
        }
        book.templates.insert(draft.target.clone(), next);
    }
    book.areas = Some(names);
    Ok(
        json!({"entries_checked":checked,"entries_migrated":changed,"link_objects_verified":links,"values_retained":retained}),
    )
}

pub(super) async fn publish(
    root: &Path,
    context: &ToolContext,
    job: &review_queue::Job,
) -> Result<(), String> {
    let snapshot = load(root)?;
    let mut reports = vec![];
    for draft in &job.process_drafts {
        let skill = matches!(draft.definition, Definition::Skill { .. });
        if baseline(root, &snapshot, &draft.target, skill)? != draft.baseline {
            return Err("Process version changed after staging. Rebase the request.".into());
        }
        let mut results = vec![];
        if let Definition::Skill {
            after: Some(doc), ..
        } = &draft.definition
        {
            for (i, example) in draft.examples.iter().enumerate() {
                let task = format!(
                    "{TEST}\n\nExample data:\n{}",
                    json!({"skill":doc.markdown,"role":doc.role,"input":example.input})
                );
                crate::diagnostics::event(
                    "skill.test_started",
                    json!({"job":job.id,"target":draft.target,"example":i+1}),
                );
                let output = review::read_worker(
                    context,
                    format!("Test {} · example {}", doc.title, i + 1),
                    task,
                )
                .await
                .map_err(error)?;
                let answer = output.to_lowercase();
                if let Some(s) = example
                    .required
                    .iter()
                    .find(|s| !answer.contains(&s.to_lowercase()))
                {
                    return Err(format!(
                        "{} example {} failed: missing {s:?}. Live version preserved.",
                        doc.title,
                        i + 1
                    ));
                }
                if let Some(s) = example
                    .forbidden
                    .iter()
                    .find(|s| answer.contains(&s.to_lowercase()))
                {
                    return Err(format!(
                        "{} example {} failed: returned forbidden {s:?}. Live version preserved.",
                        doc.title,
                        i + 1
                    ));
                }
                crate::diagnostics::event(
                    "skill.test_passed",
                    json!({"job":job.id,"target":draft.target,"example":i+1}),
                );
                results.push(json!({"input":example.input,"required":example.required,"forbidden":example.forbidden,"passed":true}));
            }
        }
        reports.push(results);
    }
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(root)?;
    let index = book
        .review_jobs
        .iter()
        .position(|j| j.id == job.id && j.state == "running" && j.process_ready)
        .ok_or("Process publication stopped")?;
    for (draft, examples) in job.process_drafts.iter().zip(reports) {
        let skill = matches!(draft.definition, Definition::Skill { .. });
        if baseline(root, &book, &draft.target, skill)? != draft.baseline {
            return Err("Target changed during testing. Rebase the request.".into());
        }
        let tests = match &draft.definition {
            Definition::Template { .. } => migrate(&mut book, draft)?,
            Definition::Skill { after, .. } => {
                if let Some(doc) = after {
                    let path = skill_file(root, draft.id)?;
                    write(&path, &doc.markdown)?;
                    if fs::read_to_string(path).map_err(error)? != doc.markdown {
                        return Err("Markdown round-trip failed".into());
                    }
                }
                json!({"examples":examples,"restored_version":draft.undoes})
            }
        };
        if let Some(undo) = draft.undoes {
            book.process_history
                .iter_mut()
                .find(|v| v.id == undo)
                .ok_or("Unknown rollback target")?
                .undone_by = Some(draft.id);
        }
        book.process_history.push(Version {
            id: draft.id,
            job: job.id,
            created: crate::now_millis(),
            target: draft.target.clone(),
            definition: draft.definition.clone(),
            tests: tests.clone(),
            undoes: draft.undoes,
            undone_by: None,
        });
        book.review_jobs[index].receipts.push(
            json!({"published":draft.id,"target":draft.target,"tests":tests,"undoes":draft.undoes}),
        );
    }
    let j = &mut book.review_jobs[index];
    j.state = "done".into();
    j.updated_at = crate::now_millis();
    j.result_id = job.process_drafts.last().map(|d| d.id);
    j.report = Some(format!(
        "Published {} tested process change(s).",
        job.process_drafts.len()
    ));
    j.process_drafts.clear();
    j.process_ready = false;
    save(root, &book)
}

#[cfg(test)]
mod tests {
    use super::*;
    use canonwright_tools::{SubagentInfo, SubagentRuntime, SubagentSpec};
    use canonwright_workspace::WorkspaceService;
    use std::sync::Arc;

    fn claim(root: &Path, request: &str) -> Uuid {
        let id = review_queue::submit(
            root,
            "process_manager",
            request,
            &agent::Focus::book("fixture".into()),
        )
        .unwrap();
        review_queue::update_job(root, id, |j| {
            j.state = "running".into();
            Ok(())
        })
        .unwrap();
        id
    }
    async fn finish(root: &Path, id: Uuid, ctx: &ToolContext) -> Result<(), String> {
        review_queue::update_job(root, id, |j| {
            j.process_ready = true;
            Ok(())
        })?;
        let job = load(root)?
            .review_jobs
            .into_iter()
            .find(|j| j.id == id)
            .unwrap();
        publish(root, ctx, &job).await
    }
    #[tokio::test]
    async fn template_round_trip_keeps_pending_entries_later_edits_and_retired_links() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        initialize(root, Some("novel"), None, None).unwrap();
        let ctx = ToolContext::new(Arc::new(WorkspaceService::new(root)));
        let mut book = load(root).unwrap();
        let section = book.sections[0].id;
        let original = book.template("Characters");
        let mut ids = Vec::new();
        for (name, pending) in [("Aldis", false), ("Rann", true)] {
            let finding=serde_json::from_value(json!({"kind":"entry","area":"Characters","title":name,"detail":"Lives by the lake.","evidence":"The lake."})).unwrap();
            ids.push(insert_finding(&mut book, Some(section), finding, pending));
        }
        let link = json!([{"name":"Aldis","target_id":ids[0],"context":"mother"}]);
        book.records[1].finding.fields =
            serde_json::from_value(json!({"motivation":"Protect her family","relationships":link}))
                .unwrap();
        save(root, &book).unwrap();
        let id = claim(root, "Migrate Characters");
        let mut fields = original.fields.clone();
        fields.retain(|f| f.key != "motivation");
        let field = fields
            .iter_mut()
            .find(|f| f.key == "relationships")
            .unwrap();
        field.key = "connections".into();
        let mut extra = field.clone();
        extra.key = "mentors".into();
        fields.push(extra);
        let draft=stage(root,id,"template",&json!({"area":"Characters","revision":0,"fields":fields,"renames":{"relationships":"connections"}})).unwrap();
        assert_eq!(draft["preview"]["entries_checked"], 2);
        assert_eq!(
            load(root).unwrap().records[1].finding.fields["relationships"],
            link
        );
        finish(root, id, &ctx).await.unwrap();
        let mut book = load(root).unwrap();
        assert_eq!(book.records[1].retained[0].value, "Protect her family");
        assert_eq!(book.records[1].finding.fields["connections"], link);
        let mut edited = link.clone();
        edited[0]["context"] = json!("mother and teacher");
        book.records[1]
            .finding
            .fields
            .insert("connections".into(), edited.clone());
        book.records[1]
            .finding
            .fields
            .insert("mentors".into(), link.clone());
        save(root, &book).unwrap();
        let undo = claim(root, "Undo Characters migration");
        stage(
            root,
            undo,
            "rollback",
            &json!({"change_id":draft["staged"]}),
        )
        .unwrap();
        finish(root, undo, &ctx).await.unwrap();
        let book = load(root).unwrap();
        assert_eq!(book.template("Characters").revision, 2);
        assert_eq!(
            json!(book.template("Characters").fields),
            json!(original.fields)
        );
        let record = &book.records[1];
        assert!(record.pending);
        assert_eq!(record.id, ids[1]);
        assert_eq!(record.section, Some(section));
        assert_eq!(record.finding.evidence, "The lake.");
        assert_eq!(record.finding.fields["relationships"], edited);
        assert_eq!(record.finding.fields["motivation"], "Protect her family");
        assert_eq!(record.retained.len(), 1);
        assert_eq!(record.retained[0].value, link);
    }

    struct ExampleWorker;
    impl ExampleWorker {
        fn result(&self) -> SubagentInfo {
            SubagentInfo {
                id: "example".into(),
                title: String::new(),
                task: String::new(),
                model: "fixture".into(),
                status: "completed".into(),
                log: String::new(),
                result: "the bell rings".into(),
                error: None,
                steps: 1,
                created_at: 0,
                updated_at: 0,
                finished_at: Some(0),
            }
        }
    }
    impl SubagentRuntime for ExampleWorker {
        fn spawn(&self, spec: SubagentSpec) -> Result<SubagentInfo, String> {
            assert!(spec.task.starts_with(TEST));
            assert!(!spec.allow_writes);
            assert!(!spec.task.contains("hidden assertion"));
            Ok(self.result())
        }
        fn get(&self, _: &str) -> Result<Option<SubagentInfo>, String> {
            Ok(Some(self.result()))
        }
        fn list(&self, _: usize) -> Result<Vec<SubagentInfo>, String> {
            Ok(vec![])
        }
        fn cancel(&self, _: &str) -> Result<SubagentInfo, String> {
            Ok(self.result())
        }
    }
    #[tokio::test]
    async fn isolated_skill_tests_gate_whole_job_and_allow_versioned_rollbacks() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        initialize(root, Some("novel"), None, None).unwrap();
        let mut ctx = ToolContext::new(Arc::new(WorkspaceService::new(root)));
        ctx.subagents = Some(Arc::new(ExampleWorker));
        let create = claim(root, "Create bell skill");
        let mut args = json!({"title":"Bells","role":"writer","instructions":"Describe the sound of the object.","examples":[{"input":"A bronze bell swings.","required":["bell"],"forbidden":["hidden assertion"]}]});
        let first = stage(root, create, "skill", &args).unwrap();
        finish(root, create, &ctx).await.unwrap();
        let target = first["target"].as_str().unwrap();
        let book = load(root).unwrap();
        let doc = read_skill(root, &book, target).unwrap().unwrap();
        assert!(role_guidance(root, "writer").contains("Describe the sound"));
        args["skill_id"] = json!(target);
        args["fingerprint"] = json!("stale");
        args["instructions"] = json!("Identify the source of the sound.");
        args["role"] = json!("canon_keeper");
        let edit = claim(root, "Update bell skill");
        assert!(stage(root, edit, "skill", &args).is_err());
        args["fingerprint"] = json!(doc.fingerprint());
        args["examples"][0]["required"] = json!(["hidden assertion"]);
        let template = book.template("Instruments");
        stage(
            root,
            edit,
            "template",
            &json!({"area":"Instruments","fields":template.fields,"revision":0}),
        )
        .unwrap();
        stage(root, edit, "skill", &args).unwrap();
        assert!(
            finish(root, edit, &ctx)
                .await
                .unwrap_err()
                .contains("missing")
        );
        let book = load(root).unwrap();
        assert!(!book.areas().contains(&"Instruments"));
        assert_eq!(book.process_history.len(), 1);
        assert_eq!(
            read_skill(root, &book, target)
                .unwrap()
                .unwrap()
                .fingerprint(),
            doc.fingerprint()
        );
        args["examples"][0]["required"] = json!(["rings"]);
        let second = stage(root, edit, "skill", &args).unwrap();
        finish(root, edit, &ctx).await.unwrap();
        assert!(role_guidance(root, "canon_keeper").contains("Identify the source"));
        for (name, change) in [
            ("Undo skill edit", second),
            ("Undo skill creation", first.clone()),
        ] {
            let undo = claim(root, name);
            stage(
                root,
                undo,
                "rollback",
                &json!({"change_id":change["staged"]}),
            )
            .unwrap();
            finish(root, undo, &ctx).await.unwrap();
            if name == "Undo skill edit" {
                assert_eq!(
                    read_skill(root, &load(root).unwrap(), target)
                        .unwrap()
                        .unwrap()
                        .fingerprint(),
                    doc.fingerprint()
                );
            }
        }
        let book = load(root).unwrap();
        assert!(read_skill(root, &book, target).unwrap().is_none());
        assert!(
            skill_catalog(root, &book)
                .unwrap()
                .iter()
                .all(|s| s.id != target)
        );
    }
}
