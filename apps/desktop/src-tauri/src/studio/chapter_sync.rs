use super::*;
use canonwright_tools::ToolContext;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const KEEPER: &str = include_str!("../../skills/sync-chapter/SKILL.md");
const REVIEW: &str = include_str!("../../skills/sync-chapter/review.md");

pub(crate) fn directive(task: &str) -> Option<&'static str> {
    [KEEPER, REVIEW]
        .into_iter()
        .find(|prompt| task.starts_with(prompt))
}

// One reversible edit suffices to recover the old chapter and replay an interrupted file save.
#[derive(Clone, Serialize, Deserialize)]
struct Edit {
    at: usize,
    old: String,
    new: String,
}

impl Edit {
    fn between(old: &str, new: &str) -> Self {
        let at = old
            .chars()
            .zip(new.chars())
            .take_while(|(a, b)| a == b)
            .map(|(c, _)| c.len_utf8())
            .sum::<usize>();
        let tail = old[at..]
            .chars()
            .rev()
            .zip(new[at..].chars().rev())
            .take_while(|(a, b)| a == b)
            .map(|(c, _)| c.len_utf8())
            .sum::<usize>();
        Self {
            at,
            old: old[at..old.len() - tail].into(),
            new: new[at..new.len() - tail].into(),
        }
    }
    fn apply(&self, source: &str, reverse: bool) -> Result<String, String> {
        let (before, after) = if reverse {
            (&self.new, &self.old)
        } else {
            (&self.old, &self.new)
        };
        let end = self
            .at
            .checked_add(before.len())
            .ok_or("Invalid edit offset")?;
        if source.get(self.at..end) != Some(before) {
            return Err("Chapter no longer matches the recorded edit".into());
        }
        let mut text = source.to_owned();
        text.replace_range(self.at..end, after);
        Ok(text)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Change {
    pub section: Uuid,
    base: String,
    previous: String,
    revision: String,
    net: Edit,
    last: Edit,
    plan: Option<Plan>,
    stamp: Option<String>,
    verdict: Option<Disposition>,
}

impl Change {
    pub(super) fn role(&self) -> &'static str {
        if self.plan.is_some()
            && self
                .verdict
                .as_ref()
                .is_none_or(|v| v.approve || v.conflict.is_some())
        {
            "canon_reviewer"
        } else {
            "canon_keeper"
        }
    }

    pub(super) fn issue_changes(
        &self,
        section: Uuid,
        src: &SectionView,
        passage: &str,
    ) -> Option<Value> {
        if self.section != section || self.revision != src.revision {
            return None;
        }
        let end = self.last.at + self.last.new.len();
        let overlaps = src.text.match_indices(passage).any(|(pos, _)| {
            if self.last.new.is_empty() {
                pos <= end && pos + passage.len() >= end
            } else {
                pos < end && pos + passage.len() > self.last.at
            }
        });
        overlaps.then(||json!({"before":self.last.old,"after":self.last.new,"section":section,"revision":self.revision}))
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Update {
    id: Uuid,
    evidence: String,
    title: Option<String>,
    detail: Option<String>,
    #[serde(default)]
    text: BTreeMap<String, String>,
    #[serde(default)]
    add: BTreeMap<String, Vec<Value>>,
    #[serde(default)]
    remove: BTreeMap<String, Vec<Value>>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    updates: Vec<Update>,
    creates: Vec<Finding>,
    questions: Vec<String>,
}
#[derive(Clone, Deserialize, Serialize)]
struct Disposition {
    #[serde(default)]
    conflict: Option<editorial::Conflict>,
    approve: bool,
    reason: String,
}

pub(super) fn save_edit(
    root: &Path,
    section: Uuid,
    revision: &str,
    text: &str,
    parent: Option<Uuid>,
) -> Result<Option<Uuid>, String> {
    if text.len() > MAX_SECTION {
        return Err("This section exceeds 4 MiB".into());
    }
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(root)?;
    let source = read_section(root, &book, section)?;
    if source.revision != revision {
        return Err("Chapter changed. Read its current revision before editing.".into());
    }
    if source.text == text {
        return Ok(None);
    }
    let id = stage(&mut book, section, &source, text, parent)?;
    save(root, &book)?;
    write(&section_path(root, section)?, text)?;
    Ok(Some(id))
}

pub(super) fn stage(
    book: &mut Book,
    section: Uuid,
    source: &SectionView,
    text: &str,
    parent: Option<Uuid>,
) -> Result<Uuid, String> {
    let revision = &source.revision;
    let last = Edit::between(&source.text, text);
    let mut base = source.text.clone();
    // An edit supersedes the outstanding packet, carrying its entire unprocessed delta forward.
    if let Some(prior) = book.review_jobs.iter_mut().rev().find(|j| {
        j.sync
            .as_ref()
            .is_some_and(|s| s.section == section && s.revision == *revision)
            && matches!(j.state.as_str(), "ready" | "running" | "failed")
    }) {
        let change = prior.sync.as_ref().unwrap();
        base = change.net.apply(&source.text, true)?;
        if hash_bytes(base.as_bytes()) != change.base {
            return Err("Invalid pending chapter revision".into());
        }
        prior.state = "superseded".into();
    }
    let id = Uuid::now_v7();
    let change = Change {
        section,
        base: hash_bytes(base.as_bytes()),
        previous: revision.clone(),
        revision: hash_bytes(text.as_bytes()),
        net: Edit::between(&base, text),
        last,
        plan: None,
        stamp: None,
        verdict: None,
    };
    book.review_jobs.push(review_queue::Job {
        id,
        parent_job: parent,
        state: "ready".into(),
        instructions: Some("Sync changed chapter passages".into()),
        updated_at: crate::now_millis(),
        sync: Some(change),
        ..Default::default()
    });
    Ok(id)
}

#[derive(Serialize)]
struct Passage {
    before: String,
    after: String,
}

fn passages(old: &str, new: &str) -> Vec<Passage> {
    use similar::{DiffTag, TextDiff};
    let lines = |s: &str| {
        let mut offsets = vec![0];
        for line in s.split_inclusive('\n') {
            offsets.push(offsets.last().unwrap() + line.len());
        }
        offsets
    };
    let left = lines(old);
    let right = lines(new);
    fn excerpt(text: &str, start: usize, end: usize) -> String {
        let pre = &text[..start];
        let post = &text[end..];
        let a = pre
            .char_indices()
            .rev()
            .take(220)
            .take_while(|(_, c)| *c != '\n')
            .last()
            .map_or(start, |(i, _)| i);
        let b = post
            .chars()
            .take(220)
            .take_while(|c| *c != '\n')
            .map(char::len_utf8)
            .sum::<usize>();
        text[a..end + b].into()
    }
    TextDiff::from_lines(old, new)
        .ops()
        .iter()
        .filter(|op| op.tag() != DiffTag::Equal)
        .map(|op| {
            let l = op.old_range();
            let r = op.new_range();
            let diff = Edit::between(
                &old[left[l.start]..left[l.end]],
                &new[right[r.start]..right[r.end]],
            );
            let a = left[l.start] + diff.at;
            let b = right[r.start] + diff.at;
            Passage {
                before: excerpt(old, a, a + diff.old.len()),
                after: excerpt(new, b, b + diff.new.len()),
            }
        })
        .collect()
}

fn cited<'a>(r: &'a Record, section: Uuid) -> impl Iterator<Item = &'a str> {
    r.section
        .filter(|id| *id == section)
        .map(|_| r.finding.evidence.as_str())
        .into_iter()
        .chain(
            r.citations
                .iter()
                .filter(move |c| c.section == section)
                .map(|c| c.evidence.as_str()),
        )
}
fn links(record: &Record) -> impl Iterator<Item = Uuid> + '_ {
    record
        .finding
        .fields
        .values()
        .chain(record.retained.iter().map(|f| &f.value))
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(|v| v.get("target_id")?.as_str()?.parse().ok())
}
struct Scope {
    passages: Vec<Passage>,
    ids: BTreeSet<Uuid>,
    packet: Value,
    stamp: String,
}

fn scope(book: &Book, change: &Change, new: &str) -> Result<Scope, String> {
    if hash_bytes(new.as_bytes()) != change.revision {
        return Err("Chapter revision changed during sync".into());
    }
    let old = change.net.apply(new, true)?;
    if hash_bytes(old.as_bytes()) != change.base {
        return Err("Chapter base revision changed".into());
    }
    let passages = passages(&old, new);
    let excerpts: Vec<_> = passages
        .iter()
        .flat_map(|p| [p.before.as_str(), p.after.as_str()])
        .collect();
    let words = excerpts
        .iter()
        .map(|s| s.to_lowercase())
        .collect::<Vec<_>>();
    let named = |name: &str| {
        let name = name.trim().to_lowercase();
        !name.is_empty()
            && words.iter().any(|s| {
                s.match_indices(&name).any(|(at, _)| {
                    !s[..at]
                        .chars()
                        .next_back()
                        .is_some_and(char::is_alphanumeric)
                        && !s[at + name.len()..]
                            .chars()
                            .next()
                            .is_some_and(char::is_alphanumeric)
                })
            })
    };
    let direct: BTreeSet<_> = book
        .records
        .iter()
        .filter(|r| !r.pending)
        .filter(|r| {
            cited(r, change.section).any(|quote| {
                !quote.is_empty()
                    && ((old.contains(quote) && !new.contains(quote))
                        || excerpts.iter().any(|s| {
                            !s.trim().is_empty() && (s.contains(quote) || quote.contains(s))
                        }))
            }) || named(&r.finding.title)
                || r.finding
                    .fields
                    .get("aliases")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .any(named)
        })
        .map(|r| r.id)
        .collect();
    let mut ids = direct.clone();
    for r in book.records.iter().filter(|r| !r.pending) {
        if direct.contains(&r.id) {
            ids.extend(links(r));
        } else if links(r).any(|id| direct.contains(&id)) {
            ids.insert(r.id);
        }
    }
    let records: Vec<_> = book
        .records
        .iter()
        .filter(|r| !r.pending && ids.contains(&r.id))
        .collect();
    let templates: BTreeMap<_, _> = book
        .areas()
        .into_iter()
        .map(|a| (a, book.template(a)))
        .collect();
    let packet = json!({"section":change.section,"passages":passages,"direct_ids":direct,"records":records,"templates":templates});
    let stamp = hash_bytes(packet.to_string().as_bytes());
    Ok(Scope {
        passages,
        ids,
        packet,
        stamp,
    })
}

fn validate_and_apply(
    book: &mut Book,
    change: &Change,
    scope: &Scope,
    source: &str,
    plan: &Plan,
) -> Result<Vec<Uuid>, String> {
    let supported = |s: &str| {
        !s.trim().is_empty()
            && s.len() <= 1800
            && source.contains(s)
            && scope.passages.iter().any(|p| p.after.contains(s))
    };
    let mut touched = BTreeSet::new();
    for patch in &plan.updates {
        if !scope.ids.contains(&patch.id)
            || !touched.insert(patch.id)
            || !supported(&patch.evidence)
        {
            return Err(
                "Patch needs a unique scoped target and verbatim changed-passage evidence".into(),
            );
        }
        let index = book
            .records
            .iter()
            .position(|r| r.id == patch.id && !r.pending)
            .ok_or("Canon entry changed")?;
        let mut r = book.records[index].clone();
        if let Some(title) = &patch.title {
            r.finding.title = title.clone();
        }
        if let Some(detail) = &patch.detail {
            r.finding.detail = detail.clone();
        }
        for (k, v) in &patch.text {
            r.finding.fields.insert(k.clone(), json!(v));
        }
        for (key, items) in &patch.remove {
            let values = r
                .finding
                .fields
                .get_mut(key)
                .and_then(Value::as_array_mut)
                .ok_or("Missing list to amend")?;
            for item in items {
                let i = values
                    .iter()
                    .position(|v| v == item)
                    .ok_or("List item changed")?;
                values.remove(i);
            }
            if values.is_empty() {
                r.finding.fields.remove(key);
            }
        }
        for (key, items) in &patch.add {
            let values = r
                .finding
                .fields
                .entry(key.clone())
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .ok_or("Expected a list")?;
            for item in items {
                if !values.contains(item) {
                    values.push(item.clone());
                }
            }
        }
        validate_fields(&r.finding, &book.areas())?;
        if !r.finding.fields.is_empty() {
            check_entry(book, &r.finding)?;
        }
        r.citations
            .retain(|c| c.section != change.section || source.contains(&c.evidence));
        if r.section == Some(change.section) && !source.contains(&r.finding.evidence) {
            r.finding.evidence = patch.evidence.clone();
        } else if !(r.section == Some(change.section) && r.finding.evidence == patch.evidence) {
            let cite = Citation {
                section: change.section,
                evidence: patch.evidence.clone(),
            };
            if !r.citations.contains(&cite) {
                r.citations.push(cite);
            }
        }
        book.records[index] = r;
    }
    for f in &plan.creates {
        if !supported(&f.evidence) {
            return Err("New entry needs changed-passage evidence".into());
        }
        validate_finding(f, &book.areas(), source)?;
        check_entry(book, f)?;
        if book
            .records
            .iter()
            .any(|r| r.finding.area == f.area && r.finding.title.eq_ignore_ascii_case(&f.title))
        {
            return Err("A matching entry already exists; update its stable ID".into());
        }
        touched.insert(insert_finding(book, Some(change.section), f.clone(), false));
    }
    let original = change.net.apply(source, true)?;
    for r in book
        .records
        .iter_mut()
        .filter(|r| scope.ids.contains(&r.id))
    {
        let stale = cited(r, change.section)
            .any(|s| !s.is_empty() && original.contains(s) && !source.contains(s));
        if stale {
            if plan.questions.iter().all(|q| q.trim().is_empty()) {
                return Err(format!(
                    "Unresolved source for {} needs a correction or an author question",
                    r.finding.title
                ));
            }
            if !r.unverified_sections.contains(&change.section) {
                r.unverified_sections.push(change.section);
            }
        } else if cited(r, change.section).all(|s| source.contains(s)) {
            r.unverified_sections.retain(|id| *id != change.section);
        }
    }
    Ok(touched.into_iter().collect())
}

fn parse<T: serde::de::DeserializeOwned>(s: &str) -> Result<T, String> {
    let s = s.trim();
    let json = s
        .strip_prefix("```json")
        .and_then(|v| v.strip_suffix("```"))
        .unwrap_or(s);
    serde_json::from_str(json.trim()).map_err(error)
}

pub(super) async fn run(root: &Path, context: &ToolContext, id: Uuid) -> Result<(), String> {
    let (change, scope) = {
        let _guard = BOOK_IO.lock().map_err(error)?;
        let book = load(root)?;
        let change = book
            .review_jobs
            .iter()
            .find(|j| j.id == id && j.state == "running")
            .and_then(|j| j.sync.clone())
            .ok_or("Missing sync job")?;
        let mut section = read_section(root, &book, change.section)?;
        if section.revision == change.previous && section.revision != change.revision {
            section.text = change.last.apply(&section.text, false)?;
            if hash_bytes(section.text.as_bytes()) != change.revision {
                return Err("Cannot recover chapter edit".into());
            }
            write(&section_path(root, change.section)?, &section.text)?;
        }
        let scope = scope(&book, &change, &section.text)?;
        (change, scope)
    };
    async fn ask<T: serde::de::DeserializeOwned>(
        cx: &ToolContext,
        prompt: &str,
        title: &str,
        data: &Value,
    ) -> Result<T, String> {
        let answer =
            review::read_worker(cx, title.into(), format!("{prompt}\n\nBook data:\n{data}"))
                .await
                .map_err(error)?;
        match parse(&answer) {
            Ok(parsed) => Ok(parsed),
            Err(reason) => {
                let task = format!("{prompt}\nYour previous JSON failed validation: {reason}. Correct the response using the same source data.\nPrevious response:\n{answer}\nBook data:\n{data}");
                let corrected = review::read_worker(cx, title.into(), task).await.map_err(error)?;
                parse(&corrected)
            }
        }
    }
    let fresh = change.stamp.as_ref() == Some(&scope.stamp);
    if !fresh && change.role() == "canon_reviewer" {
        return review_queue::update_job(root, id, |j| {
            if j.state == "running" {
                let s = j.sync.as_mut().ok_or("Missing sync")?;
                s.plan = None;
                s.verdict = None;
                j.state = "ready".into();
            }
            Ok(())
        });
    }
    let rejected = change
        .verdict
        .as_ref()
        .filter(|v| !v.approve && v.conflict.is_none() && fresh);
    let plan = if let Some(p) = change.plan.clone().filter(|_| fresh && rejected.is_none()) {
        p
    } else if scope.passages.is_empty() {
        Plan {
            updates: vec![],
            creates: vec![],
            questions: vec![],
        }
    } else {
        let mut packet = scope.packet.clone();
        if let Some(feedback) = rejected {
            packet["returned_patch"] = json!(change.plan);
            packet["correction_requested"] = json!(feedback.reason);
        }
        ask(context, KEEPER, "Sync chapter changes", &packet).await?
    };
    let persist = |plan: &Plan, verdict: Option<Disposition>| {
        review_queue::update_job(root, id, |j| {
            if j.state != "running" {
                return Err("Chapter sync superseded or stopped".into());
            }
            let change = j.sync.as_mut().ok_or("Missing sync")?;
            change.plan = Some(plan.clone());
            change.stamp = Some(scope.stamp.clone());
            change.verdict = verdict;
            Ok(())
        })
    };
    let verdict = if let Some(v) = change
        .verdict
        .clone()
        .filter(|v| fresh && (v.approve || v.conflict.is_some()))
    {
        v
    } else {
        // Validate before spending a reviewer call or persisting an unexecutable plan.
        let mut preview = load(root)?;
        let source = read_section(root, &preview, change.section)?;
        validate_and_apply(&mut preview, &change, &scope, &source.text, &plan)?;
        if change.role() == "canon_keeper" && !(plan.updates.is_empty() && plan.creates.is_empty())
        {
            return review_queue::update_job(root, id, |j| {
                if j.state == "running" {
                    let s = j.sync.as_mut().ok_or("Missing sync")?;
                    s.plan = Some(plan);
                    s.stamp = Some(scope.stamp);
                    s.verdict = None;
                    j.state = "ready".into();
                }
                Ok(())
            });
        }
        persist(&plan, None)?;
        let v = if plan.updates.is_empty() && plan.creates.is_empty() {
            Disposition {
                conflict: None,
                approve: true,
                reason: "No canon mutations proposed.".into(),
            }
        } else {
            ask(
                context,
                REVIEW,
                "Review changed canon entries",
                &json!({"scope":scope.packet,"patch":plan}),
            )
            .await?
        };
        persist(&plan, Some(v.clone()))?;
        v
    };
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(root)?;
    let idx = book
        .review_jobs
        .iter()
        .position(|j| j.id == id && j.state == "running")
        .ok_or("Chapter sync superseded or stopped")?;
    let change = book.review_jobs[idx].sync.clone().unwrap();
    let source = read_section(root, &book, change.section)?;
    if self::scope(&book, &change, &source.text)?.stamp != scope.stamp {
        return Err(
            "Affected canon changed during sync. Retry this job against current records.".into(),
        );
    }
    if verdict.reason.trim().is_empty() {
        return Err("Missing review reason".into());
    }
    if let Some(conflict) = verdict.conflict {
        let referral = editorial::refer(root, &mut book, id, None, conflict)?;
        book.review_jobs[idx]
            .receipts
            .push(json!({"editorial_referral":referral}));
        if !verdict.approve {
            let j = &mut book.review_jobs[idx];
            j.state = "done".into();
            j.report = Some(format!("Disputed canon patch withheld. {}", verdict.reason));
            j.updated_at = crate::now_millis();
            return save(root, &book);
        }
    }
    if !verdict.approve {
        return Err(verdict.reason);
    }
    let ids = validate_and_apply(&mut book, &change, &scope, &source.text, &plan)?;
    let job = &mut book.review_jobs[idx];
    job.receipts.push(json!({"section_id":change.section,"revision":change.revision,"updated_entries":ids,"questions":plan.questions}));
    job.updated_at = crate::now_millis();
    job.state = if plan.questions.is_empty() {
        "done"
    } else {
        "waiting_for_author"
    }
    .into();
    job.report = Some(if plan.questions.is_empty() {
        format!("Updated {} canon entries. {}", ids.len(), verdict.reason)
    } else {
        plan.questions.join("\n")
    });
    save(root, &book)
}

#[cfg(test)]
mod tests {
    use super::*;
    use canonwright_workspace::WorkspaceService;
    use std::sync::Arc;

    fn entry(title: &str, detail: &str, evidence: &str, fields: Value) -> Finding {
        serde_json::from_value(json!({"kind":"entry","area":"Characters","title":title,"detail":detail,"evidence":evidence,"fields":fields})).unwrap()
    }

    #[tokio::test]
    async fn recovered_approved_delta_updates_identity_sources_and_only_changed_links() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        initialize(root, Some("novel"), None, None).unwrap();
        let old = "Rann lives at Reedbank.\nThe unrelated passage is private.\n";
        let new = old.replace("Reedbank", "Cairn Ness");
        let chapter = add_at(root, "Home", old).unwrap();
        let other = add_at(root, "Other", "A separate source.").unwrap();
        let mut book = load(root).unwrap();
        let aldis = insert_finding(
            &mut book,
            Some(other),
            entry(
                "Aldis",
                "Unchanged mother.",
                "A separate source.",
                json!({"role":"Mother"}),
            ),
            false,
        );
        let mother = json!({"target_id":aldis,"name":"Aldis","context":"Mother"});
        let rann = insert_finding(
            &mut book,
            Some(chapter),
            entry(
                "Rann",
                "Lives at Reedbank.",
                "Rann lives at Reedbank.",
                json!({"relationships":[mother],"role":"Narrator"}),
            ),
            false,
        );
        book.records[0].finding.fields.insert(
            "relationships".into(),
            json!([{"target_id":rann,"name":"Rann","context":"Daughter"}]),
        );
        book.records[1].citations.push(Citation {
            section: other,
            evidence: "A separate source.".into(),
        });
        let removed_link = json!({"name":"Reedbank","context":"Home"});
        let kept_link = json!({"name":"Harbor","context":"Works here"});
        book.records[1]
            .finding
            .fields
            .insert("places".into(), json!([removed_link, kept_link]));
        let unrelated = insert_finding(
            &mut book,
            Some(other),
            entry(
                "Una",
                "Unrelated data",
                "A separate source.",
                json!({"role":"Weaver"}),
            ),
            false,
        );
        let parent = Uuid::now_v7();
        let id = stage(
            &mut book,
            chapter,
            &SectionView {
                text: old.into(),
                revision: hash_bytes(old.as_bytes()),
            },
            &new,
            Some(parent),
        )
        .unwrap();
        let change = book.review_jobs[0].sync.clone().unwrap();
        let scope = scope(&book, &change, &new).unwrap();
        assert!(scope.ids.contains(&aldis));
        assert!(!scope.ids.contains(&unrelated));
        assert!(!scope.packet.to_string().contains("unrelated passage"));
        let plan:Plan=serde_json::from_value(json!({"updates":[{"id":rann,"detail":"Lives at Cairn Ness.","evidence":"Rann lives at Cairn Ness.","add":{"places":[{"name":"Cairn Ness","context":"Home"}]},"remove":{"places":[removed_link]}}],"creates":[],"questions":[]})).unwrap();
        let j = &mut book.review_jobs[0];
        j.state = "running".into();
        let change = j.sync.as_mut().unwrap();
        change.stamp = Some(scope.stamp);
        change.plan = Some(plan);
        change.verdict = Some(Disposition {
            conflict: None,
            approve: true,
            reason: "Verified change of residence.".into(),
        });
        save(root, &book).unwrap(); // Simulate interruption before writing the chapter.
        let context = ToolContext::new(Arc::new(WorkspaceService::new(root)));
        run(root, &context, id).await.unwrap(); // No model runtime: must replay the saved decision.
        let reloaded = load(root).unwrap();
        assert_eq!(read_section(root, &reloaded, chapter).unwrap().text, new);
        assert_eq!(json!(book.records[0]), json!(reloaded.records[0]));
        assert_eq!(json!(book.records[2]), json!(reloaded.records[2]));
        let result = &reloaded.records[1];
        assert_eq!(result.id, rann);
        assert_eq!(result.finding.evidence, "Rann lives at Cairn Ness.");
        assert!(result.citations == book.records[1].citations);
        assert_eq!(result.finding.fields["relationships"], json!([mother]));
        assert_eq!(result.finding.fields["places"][0], kept_link);
        assert_eq!(result.finding.fields["places"].as_array().unwrap().len(), 2);
        assert_eq!(reloaded.review_jobs[0].parent_job, Some(parent));
        assert_eq!(reloaded.review_jobs[0].state, "done");
        assert!(run(root, &context, id).await.is_err());
    }

    #[tokio::test]
    async fn saved_patch_conflict_queues_editorial_without_repeating_canon_review() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        initialize(root, Some("rulebook"), None, None).unwrap();
        let sec = add_at(root, "Example", "Roll once.").unwrap();
        let mut book = load(root).unwrap();
        let old = read_section(root, &book, sec).unwrap();
        let text = "Roll twice.";
        let id = stage(&mut book, sec, &old, text, None).unwrap();
        let packet = scope(&book, book.review_jobs[0].sync.as_ref().unwrap(), text).unwrap();
        let change = book.review_jobs[0].sync.as_mut().unwrap();
        change.plan = Some(Plan {
            updates: vec![],
            creates: vec![],
            questions: vec![],
        });
        change.stamp = Some(packet.stamp);
        change.verdict = Some(Disposition {
            approve: false,
            reason: "Example and core rule disagree.".into(),
            conflict: Some(editorial::Conflict {
                reason: "Check example timing.".into(),
                citations: vec![Citation {
                    section: sec,
                    evidence: text.into(),
                }],
            }),
        });
        book.review_jobs[0].state = "running".into();
        save(root, &book).unwrap();
        let context = ToolContext::new(Arc::new(WorkspaceService::new(root)));
        run(root, &context, id).await.unwrap();
        let book = load(root).unwrap();
        assert!(book.records.is_empty());
        assert_eq!(book.review_jobs.len(), 2);
        assert_eq!(book.review_jobs[1].parent_job, Some(id));
        assert!(matches!(
            book.review_jobs[1].editorial,
            Some(editorial::Assignment::Triage { .. })
        ));
    }

    #[test]
    fn deleting_the_only_source_flags_the_record_and_preserves_its_identity() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        initialize(root, Some("novel"), None, None).unwrap();
        let old = "Mara owns the ferry.";
        let sec = add_at(root, "Crossing", old).unwrap();
        let mut book = load(root).unwrap();
        let id = insert_finding(
            &mut book,
            Some(sec),
            entry("Mara", "Owns the ferry.", old, json!({"role":"Ferrier"})),
            false,
        );
        stage(
            &mut book,
            sec,
            &SectionView {
                text: old.into(),
                revision: hash_bytes(old.as_bytes()),
            },
            "",
            None,
        )
        .unwrap();
        let change = book.review_jobs[0].sync.clone().unwrap();
        let scope = scope(&book, &change, "").unwrap();
        let mut plan = Plan {
            updates: vec![],
            creates: vec![],
            questions: vec![],
        };
        assert!(validate_and_apply(&mut book, &change, &scope, "", &plan).is_err());
        plan.questions.push("Does Mara still own the ferry?".into());
        validate_and_apply(&mut book, &change, &scope, "", &plan).unwrap();
        assert_eq!(book.records[0].id, id);
        assert_eq!(book.records[0].unverified_sections, vec![sec]);
    }

    #[test]
    fn multiple_edits_carry_forward_the_unprocessed_delta_and_ignore_stale_revisions() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        initialize(root, Some("rulebook"), None, None).unwrap();
        let old = "Roll once.\n\nA separate rule.\n\nPay 2 coins.\n";
        let mid = old.replace("once", "twice");
        let new = mid.replace("2 coins", "3 coins");
        let sec = add_at(root, "Rules", old).unwrap();
        let first = save_edit(root, sec, &hash_bytes(old.as_bytes()), &mid, None)
            .unwrap()
            .unwrap();
        assert!(save_edit(root, sec, &hash_bytes(old.as_bytes()), &new, None).is_err());
        let second = save_edit(root, sec, &hash_bytes(mid.as_bytes()), &new, None)
            .unwrap()
            .unwrap();
        assert!(
            save_edit(root, sec, &hash_bytes(new.as_bytes()), &new, None)
                .unwrap()
                .is_none()
        );
        let b = load(root).unwrap();
        assert_eq!(
            b.review_jobs.iter().find(|j| j.id == first).unwrap().state,
            "superseded"
        );
        let change = b
            .review_jobs
            .iter()
            .find(|j| j.id == second)
            .unwrap()
            .sync
            .as_ref()
            .unwrap();
        let packet = scope(&b, change, &new).unwrap();
        assert_eq!(packet.passages.len(), 2);
        assert!(!packet.packet.to_string().contains("A separate rule."));
        assert_eq!(change.net.apply(&new, true).unwrap(), old);
    }
}
