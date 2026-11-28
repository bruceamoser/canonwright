use super::*;
use canonwright_tools::ToolContext;
use review::Decision;
use review_queue::{Job, Work};
use serde_json::Value;

const TRIAGE: &str = include_str!("../../skills/manuscript-issues/SKILL.md");
const VERIFY: &str = include_str!("../../skills/manuscript-issues/verify.md");
const REPAIR: &str = include_str!("../../skills/manuscript-issues/repair.md");
pub(crate) fn directive(task: &str) -> Option<&'static str> {
    [TRIAGE, VERIFY].into_iter().find(|s| task.starts_with(s))
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Conflict {
    pub reason: String,
    pub citations: Vec<Citation>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub(super) enum Assignment {
    Triage {
        finding: Option<Uuid>,
        conflict: Conflict,
    },
    Repair {
        issue: Uuid,
        attempt: u8,
    },
    Verify {
        issue: Uuid,
        revision: String,
        passage: String,
        resolution: String,
        changes: Value,
        attempt: u8,
    },
}
impl Assignment {
    pub(super) fn role(&self) -> &str {
        match self {
            Self::Repair { .. } => "writer",
            _ => "editorial_reviewer",
        }
    }
}
fn job(parent: Uuid, task: Assignment, label: String) -> Job {
    Job {
        id: Uuid::now_v7(),
        state: "ready".into(),
        parent_job: Some(parent),
        updated_at: crate::now_millis(),
        instructions: Some(label),
        editorial: Some(task),
        ..Default::default()
    }
}
pub(super) fn from_record(rec: &Record, v: &review::Verdict) -> Result<Conflict, String> {
    let mut citations = vec![Citation {
        section: rec
            .section
            .ok_or("A source conflict requires a cited section")?,
        evidence: rec.finding.evidence.clone(),
    }];
    for c in rec
        .citations
        .iter()
        .chain(v.conflict_citations.iter().flatten())
    {
        if !citations.contains(c) {
            citations.push(c.clone());
        }
    }
    Ok(Conflict {
        reason: v.reason.clone(),
        citations,
    })
}
fn source_packet(root: &Path, book: &Book, citations: &[Citation]) -> Result<Vec<Value>, String> {
    if citations.is_empty() || citations.len() > 16 {
        return Err("Supply 1–16 source citations".into());
    }
    citations
        .iter()
        .map(|c| {
            let src = read_section(root, book, c.section)?;
            if c.evidence.trim().is_empty() || c.evidence.len() > 1800 {
                return Err("Invalid conflict evidence".into());
            }
            let at = src
                .text
                .find(&c.evidence)
                .ok_or("A cited passage changed. Refresh the referral before proceeding.")?;
            let end = at + c.evidence.len();
            let start = src.text[..at]
                .char_indices()
                .rev()
                .nth(799)
                .map_or(0, |(i, _)| i);
            let stop = src.text[end..]
                .char_indices()
                .nth(800)
                .map_or(src.text.len(), |(i, _)| end + i);
            Ok(json!({"citation":c,"context":&src.text[start..stop],"revision":src.revision}))
        })
        .collect()
}
pub(super) fn refer(
    root: &Path,
    book: &mut Book,
    parent: Uuid,
    finding: Option<Uuid>,
    conflict: Conflict,
) -> Result<Uuid, String> {
    if conflict.reason.trim().is_empty() {
        return Err("Describe the manuscript conflict".into());
    }
    source_packet(root, book, &conflict.citations)?;
    if let Some(j)=book.review_jobs.iter().find(|j|j.parent_job==Some(parent)&&matches!(&j.editorial,Some(Assignment::Triage{conflict:c,..}) if c.reason==conflict.reason && c.citations==conflict.citations)){return Ok(j.id);}
    let j = job(
        parent,
        Assignment::Triage { finding, conflict },
        "Assess manuscript conflict".into(),
    );
    let id = j.id;
    book.review_jobs.push(j);
    Ok(id)
}
pub(super) fn assign_writer(
    book: &mut Book,
    parent: Uuid,
    issue: Uuid,
    attempt: u8,
    feedback: &str,
) -> Result<Uuid, String> {
    if let Some(j)=book.review_jobs.iter().find(|j|matches!(j.editorial,Some(Assignment::Repair{issue:i,attempt:a}) if i==issue && a==attempt)){return Ok(j.id);}
    let record = book
        .records
        .iter()
        .find(|r| r.id == issue)
        .ok_or("Missing issue")?;
    let mut j = job(
        parent,
        Assignment::Repair { issue, attempt },
        format!("Fix {}", record.finding.title),
    );
    let mut focus = agent::Focus::default();
    focus.section_id = record.section;
    focus.record_id = Some(issue);
    focus.job_id = Some(j.id);
    j.work = Some(Work {
        role: "writer".into(),
        focus,
        request: format!(
            "{REPAIR}\nIssue: {}\nVerifier feedback: {feedback}",
            json!(record)
        ),
    });
    let id = j.id;
    book.review_jobs.push(j);
    Ok(id)
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Issue,
    NoChange,
    AskAuthor,
}
#[derive(Deserialize)]
struct Assessment {
    decision: Outcome,
    reason: String,
    target: Option<Citation>,
    title: Option<String>,
    criteria: Option<String>,
}
fn checked_index(book: &Book, id: Uuid) -> Result<usize, String> {
    book.review_jobs
        .iter()
        .position(|j| j.id == id && j.state == "running")
        .ok_or_else(|| "Editorial job is no longer running".into())
}
fn apply_assessment(
    root: &Path,
    book: &mut Book,
    id: Uuid,
    finding: Option<Uuid>,
    conflict: &Conflict,
    a: Assessment,
) -> Result<(), String> {
    let index = checked_index(book, id)?;
    if a.reason.trim().is_empty() {
        return Err("Editorial decision needs a reason".into());
    }
    let waiting = matches!(a.decision, Outcome::AskAuthor);
    if matches!(a.decision, Outcome::Issue) {
        let c = a
            .target
            .filter(|c| conflict.citations.contains(c))
            .ok_or("Choose the cited passage to fix")?;
        let src = read_section(root, book, c.section)?;
        let finding = Finding {
            kind: FindingKind::Note,
            area: "Review".into(),
            fields: Values::new(),
            title: a.title.ok_or("Missing issue title")?,
            detail: a.criteria.ok_or("Missing acceptance criteria")?,
            evidence: c.evidence.clone(),
        };
        validate_finding(&finding, &book.areas(), &src.text)?;
        let existing = book
            .records
            .iter()
            .find(|r| {
                r.section == Some(c.section)
                    && r.finding.evidence == finding.evidence
                    && r.finding.detail == finding.detail
                    && r.editorial
                        .as_ref()
                        .is_some_and(|v| v.decision == Decision::ManuscriptIssue)
            })
            .map(|r| r.id);
        let issue = if let Some(existing) = existing {
            existing
        } else {
            let verdict=serde_json::from_value(json!({"decision":"manuscript_issue","reason":finding.detail,"source_revision":src.revision})).map_err(error)?;
            let issue = insert_finding(book, Some(c.section), finding, true);
            let r = book.records.last_mut().unwrap();
            r.editorial = Some(verdict);
            r.citations = conflict
                .citations
                .iter()
                .filter(|other| **other != c)
                .cloned()
                .collect();
            issue
        };
        let writer = assign_writer(book, id, issue, 1, "")?;
        book.review_jobs[index].result_id = Some(issue);
        book.review_jobs[index]
            .receipts
            .push(json!({"issue":issue,"writer_job":writer}));
    }
    if let Some(record) = finding.and_then(|i| book.records.iter_mut().find(|r| r.id == i)) {
        if let Some(v) = record
            .editorial
            .as_mut()
            .filter(|v| v.decision == Decision::SourceConflict)
        {
            v.decision = if waiting {
                Decision::AskAuthor
            } else {
                Decision::ReferralClosed
            };
            v.reason = format!("{}\nEditorial decision: {}", v.reason, a.reason);
        }
    }
    let j = &mut book.review_jobs[index];
    j.state = if waiting {
        "waiting_for_author"
    } else {
        "done"
    }
    .into();
    j.report = Some(a.reason);
    j.updated_at = crate::now_millis();
    Ok(())
}

pub(super) fn submit_fix(
    root: &Path,
    worker: Uuid,
    issue: Uuid,
    revision: &str,
    passage: &str,
    resolution: &str,
) -> Result<Uuid, String> {
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(root)?;
    let idx = checked_index(&book, worker)?;
    if let Some(existing)=book.review_jobs.iter().find(|j|j.parent_job==Some(worker)&&matches!(&j.editorial,Some(Assignment::Verify{issue:i,revision:r,..}) if *i==issue && r==revision)){return Ok(existing.id);}
    let attempt = match book.review_jobs[idx].editorial {
        Some(Assignment::Repair { issue: i, attempt }) if i == issue => attempt,
        None if book.review_jobs[idx]
            .work
            .as_ref()
            .is_some_and(|w| w.role == "writer") =>
        {
            1
        }
        _ => return Err("Wrong Writer assignment".into()),
    };
    let record = book
        .records
        .iter()
        .find(|r| {
            r.id == issue
                && r.editorial
                    .as_ref()
                    .is_some_and(|v| v.decision == Decision::ManuscriptIssue)
        })
        .ok_or("Issue is not open")?;
    let section = record.section.ok_or("Missing issue section")?;
    let src = read_section(root, &book, section)?;
    if src.revision != revision
        || passage.trim().is_empty()
        || passage.len() > 1800
        || src.text.match_indices(passage).take(2).count() != 1
        || resolution.trim().is_empty()
    {
        return Err(
            "Read the current section and submit a unique changed passage and resolution".into(),
        );
    }
    let changes = book
        .review_jobs
        .iter()
        .rev()
        .filter(|j| j.parent_job == Some(worker))
        .filter_map(|j| j.sync.as_ref())
        .find_map(|s| s.issue_changes(section, &src, passage))
        .ok_or("Submitted passage must overlap this Writer job's actual chapter edit")?;
    let next = job(
        worker,
        Assignment::Verify {
            issue,
            revision: revision.into(),
            passage: passage.into(),
            resolution: resolution.into(),
            changes,
            attempt,
        },
        "Verify manuscript fix".into(),
    );
    let id = next.id;
    book.review_jobs.push(next);
    book.review_jobs[idx].result_id = Some(id);
    save(root, &book)?;
    Ok(id)
}

async fn assess<T: serde::de::DeserializeOwned>(
    context: &ToolContext,
    prompt: &str,
    label: &str,
    packet: &Value,
) -> Result<T, String> {
    let answer = review::read_worker(
        context,
        label.into(),
        format!("{prompt}\n\nBook data:\n{packet}"),
    )
    .await
    .map_err(error)?;
    let text = answer.trim();
    let text = text
        .strip_prefix("```json")
        .and_then(|v| v.strip_suffix("```"))
        .unwrap_or(text);
    serde_json::from_str(text.trim()).map_err(error)
}
#[derive(Deserialize)]
struct Verification {
    verified: bool,
    reason: String,
}
pub(super) async fn run(
    root: &Path,
    context: &ToolContext,
    id: Uuid,
    task: &Assignment,
) -> Result<(), String> {
    match task {
        Assignment::Triage { finding, conflict } => {
            let before = source_packet(root, &load(root)?, &conflict.citations)?;
            let answer = assess(
                context,
                TRIAGE,
                "Assess manuscript conflict",
                &json!({"conflict":conflict,"sources":before}),
            )
            .await?;
            let _guard = BOOK_IO.lock().map_err(error)?;
            let mut book = load(root)?;
            if source_packet(root, &book, &conflict.citations)? != before {
                return Err("Sources changed during editorial assessment".into());
            }
            apply_assessment(root, &mut book, id, *finding, conflict, answer)?;
            save(root, &book)
        }
        Assignment::Verify {
            issue,
            revision,
            passage,
            resolution,
            changes,
            attempt,
        } => {
            let packet = |book: &Book| -> Result<Value, String> {
                let record = book
                    .records
                    .iter()
                    .find(|r| {
                        r.id == *issue
                            && r.editorial
                                .as_ref()
                                .is_some_and(|v| v.decision == Decision::ManuscriptIssue)
                    })
                    .ok_or("No open manuscript issue")?;
                let section = record.section.ok_or("Missing source")?;
                let context = source_packet(
                    root,
                    book,
                    &[Citation {
                        section,
                        evidence: passage.clone(),
                    }],
                )?;
                let related = if record.citations.is_empty() {
                    vec![]
                } else {
                    source_packet(root, book, &record.citations)?
                };
                Ok(
                    json!({"issue":record,"changes":changes,"submitted_passage":context,"related_sources":related,"writer_explanation":resolution,"submitted_revision":revision}),
                )
            };
            let before = packet(&load(root)?)?;
            let verdict: Verification =
                assess(context, VERIFY, "Verify manuscript fix", &before).await?;
            if verdict.reason.trim().is_empty() {
                return Err("Verifier must explain its decision".into());
            }
            let _guard = BOOK_IO.lock().map_err(error)?;
            let mut book = load(root)?;
            let idx = checked_index(&book, id)?;
            if packet(&book)? != before {
                return Err("Source or issue changed during verification".into());
            }
            let verified_revision = before["submitted_passage"][0]["revision"]
                .as_str()
                .ok_or("Missing verification revision")?;
            if verdict.verified {
                review::resolve_issue(
                    root,
                    &mut book,
                    *issue,
                    verified_revision,
                    passage,
                    &format!("{resolution}\nVerification: {}", verdict.reason),
                )?;
            } else if *attempt < 2 {
                let writer = assign_writer(&mut book, id, *issue, attempt + 1, &verdict.reason)?;
                book.review_jobs[idx]
                    .receipts
                    .push(json!({"returned_to_writer":writer}));
            }
            let j = &mut book.review_jobs[idx];
            j.state = if !verdict.verified && *attempt >= 2 {
                "waiting_for_author"
            } else {
                "done"
            }
            .into();
            j.report = Some(verdict.reason);
            j.result_id = Some(*issue);
            j.updated_at = crate::now_millis();
            j.receipts.push(
                json!({"verified":verdict.verified,"issue":issue,"revision":verified_revision}),
            );
            save(root, &book)
        }
        Assignment::Repair { .. } => Err("Writer assignment requires a Writer".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, Book, Uuid, Conflict) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        initialize(root, Some("rulebook"), None, None).unwrap();
        let rule = add_at(root, "Mechanics", "Roll once per round.").unwrap();
        let ex = add_at(
            root,
            "Example",
            "Roll once per turn.\nA different sentence.",
        )
        .unwrap();
        let conflict = Conflict {
            reason: "Different timing.".into(),
            citations: vec![
                Citation {
                    section: ex,
                    evidence: "Roll once per turn.".into(),
                },
                Citation {
                    section: rule,
                    evidence: "Roll once per round.".into(),
                },
            ],
        };
        let mut book = load(root).unwrap();
        let id = refer(root, &mut book, Uuid::now_v7(), None, conflict.clone()).unwrap();
        book.review_jobs[0].state = "running".into();
        (dir, book, id, conflict)
    }

    #[test]
    fn editorial_can_close_or_escalate_without_creating_a_writer_job() {
        for (outcome, state) in [
            (Outcome::NoChange, "done"),
            (Outcome::AskAuthor, "waiting_for_author"),
        ] {
            let (dir, mut book, id, c) = fixture();
            apply_assessment(
                dir.path(),
                &mut book,
                id,
                None,
                &c,
                Assessment {
                    decision: outcome,
                    reason: "Deliberate viewpoint difference.".into(),
                    target: None,
                    title: None,
                    criteria: None,
                },
            )
            .unwrap();
            assert_eq!(book.review_jobs.len(), 1);
            assert!(book.records.is_empty());
            assert_eq!(book.review_jobs[0].state, state);
        }
    }

    #[test]
    fn writer_submission_requires_changed_text_and_creates_one_verification_job() {
        let (dir, mut book, id, c) = fixture();
        let root = dir.path();
        let parent = book.review_jobs[0].parent_job.unwrap();
        assert_eq!(refer(root, &mut book, parent, None, c.clone()).unwrap(), id);
        apply_assessment(
            root,
            &mut book,
            id,
            None,
            &c,
            Assessment {
                decision: Outcome::Issue,
                reason: "Example contradicts the rule.".into(),
                target: Some(c.citations[0].clone()),
                title: Some("Timing".into()),
                criteria: Some("Use round timing.".into()),
            },
        )
        .unwrap();
        let issue = book.records[0].id;
        let writer = book.review_jobs[1].id;
        assert_eq!(assign_writer(&mut book, id, issue, 1, "").unwrap(), writer);
        assert_eq!(book.records[0].citations.len(), 1);
        book.review_jobs[1].state = "running".into();
        save(root, &book).unwrap();
        let sec = c.citations[0].section;
        let old = read_section(root, &book, sec).unwrap();
        assert!(
            submit_fix(
                root,
                writer,
                issue,
                &old.revision,
                "Roll once per turn.",
                "Done"
            )
            .is_err()
        );
        let text = old.text.replace("turn", "round");
        chapter_sync::save_edit(root, sec, &old.revision, &text, Some(writer)).unwrap();
        let rev = hash_bytes(text.as_bytes());
        assert!(submit_fix(root, writer, issue, &rev, "A different sentence.", "Done").is_err());
        let verify = submit_fix(
            root,
            writer,
            issue,
            &rev,
            "Roll once per round.",
            "Matched the rule.",
        )
        .unwrap();
        assert_eq!(
            submit_fix(
                root,
                writer,
                issue,
                &rev,
                "Roll once per round.",
                "Matched the rule."
            )
            .unwrap(),
            verify
        );
        let book = load(root).unwrap();
        assert_eq!(
            book.review_jobs
                .iter()
                .filter(|j| matches!(j.editorial, Some(Assignment::Verify { .. })))
                .count(),
            1
        );
        assert!(book.records[0].editorial.as_ref().unwrap().decision == Decision::ManuscriptIssue);
    }
}
