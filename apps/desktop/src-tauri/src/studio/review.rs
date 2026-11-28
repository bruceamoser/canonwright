use super::*;
use canonwright_tools::{SubagentRuntime, SubagentSpec, ToolContext, ToolError, ToolResult};
use std::sync::Arc;

pub(crate) const EDITORIAL_PROMPT: &str = include_str!("../../skills/editorial-review/SKILL.md");

pub(crate) const KEEPER_PROMPT: &str = include_str!("../../skills/apply-canon-verdict/SKILL.md");

fn parse_verdict(response: &str) -> Result<Verdict, ToolError> {
    let raw = response.trim();
    // Only the terminal JSON block is authoritative. Earlier examples or quoted decisions
    // must never be used to approve a finding after a malformed final response.
    let body = if let Some((prefix, block)) = raw.split_once("```")
        && !block.trim_end().ends_with("```")
    {
        if prefix.contains('{') {
            return Err(ToolError::Failed("Multiple potential verdicts".into()));
        }
        let (lang, rest) = block
            .split_once('\n')
            .ok_or_else(|| ToolError::Failed("Incomplete verdict fence".into()))?;
        if !(lang.trim().is_empty() || lang.trim().eq_ignore_ascii_case("json"))
            || !rest.contains("```")
        {
            return Err(ToolError::Failed("Incomplete JSON verdict fence".into()));
        }
        rest.trim()
    } else if let Some(fenced) = raw.strip_suffix("```") {
        let (prefix, block) = fenced
            .trim_end()
            .rsplit_once("```")
            .ok_or_else(|| ToolError::Failed("Missing opening verdict code fence.".into()))?;
        let (lang, json) = block
            .split_once('\n')
            .ok_or_else(|| ToolError::Failed("Incomplete verdict code block.".into()))?;
        if !(prefix.is_empty() || prefix.ends_with('\n'))
            || !(lang.trim().is_empty() || lang.trim().eq_ignore_ascii_case("json"))
        {
            return Err(ToolError::Failed(
                "Expected a JSON verdict code block.".into(),
            ));
        }
        json.trim()
    } else if raw.starts_with('{') {
        raw
    } else {
        // A verdict may follow the reviewer's written report on a fresh line.
        match raw.match_indices('\n').find(|(i, _)| {
            raw[i + 1..]
                .trim_start_matches([' ', '\t'])
                .starts_with('{')
        }) {
            Some((i, _)) => raw[i + 1..].trim_start(),
            None => raw,
        }
    };
    validated_verdict(body)
}

fn validated_verdict(body: &str) -> Result<Verdict, ToolError> {
    let mut stream = serde_json::Deserializer::from_str(body).into_iter::<Verdict>();
    let verdict = stream
        .next()
        .transpose()
        .map_err(|err| ToolError::Failed(format!("Invalid verdict: {err}")))?
        .ok_or_else(|| ToolError::Failed("Missing verdict".into()))?;
    let tail = body[stream.byte_offset()..]
        .trim()
        .trim_start_matches("```")
        .trim();
    if tail.contains(['{', '}', '`'])
        || tail.to_lowercase().contains("final:")
        || tail.to_lowercase().contains("decision:")
    {
        return Err(ToolError::Failed("Ambiguous text following verdict".into()));
    }
    if verdict.reason.trim().is_empty() {
        return Err(ToolError::Failed("Reviewer supplied no reason.".into()));
    }
    Ok(verdict)
}

// Receipt redirects are the sole history used to repair obsolete link IDs.
fn redirected(book: &Book, mut id: Uuid) -> Uuid {
    for _ in 0..book.review_jobs.len() {
        if book.records.iter().any(|r| r.id == id) {
            break;
        }
        let Some(next) = book
            .review_jobs
            .iter()
            .find(|j| j.id == id && j.state == "done")
            .and_then(|j| j.result_id)
            .filter(|n| *n != id)
        else {
            break;
        };
        id = next;
    }
    id
}

// A previous merge can redirect a relationship after a verdict is saved. Accept
// the old fingerprint only if replacing documented link IDs reproduces it exactly.
fn matches_redirected_stamp(book: &Book, packet: &serde_json::Value, expected: &str) -> bool {
    let mut slots = vec![];
    let mut scan = |fields: &serde_json::Value, base: String| {
        if let Some(map) = fields.as_object() {
            for (key, value) in map {
                for (i, item) in value.as_array().into_iter().flatten().enumerate() {
                    let Some(id) = item["target_id"]
                        .as_str()
                        .and_then(|s| s.parse::<Uuid>().ok())
                    else {
                        continue;
                    };
                    let originals: Vec<_> = book
                        .review_jobs
                        .iter()
                        .filter(|j| j.state == "done" && j.id != id && j.result_id == Some(id))
                        .map(|j| json!(j.id))
                        .collect();
                    if !originals.is_empty() {
                        slots.push((format!("{base}/{key}/{i}/target_id"), originals));
                    }
                }
            }
        }
    };
    scan(&packet[0]["fields"], "/0/fields".into());
    for (i, r) in packet[2].as_array().into_iter().flatten().enumerate() {
        scan(&r["finding"]["fields"], format!("/2/{i}/finding/fields"));
    }
    let combinations = slots
        .iter()
        .fold(1usize, |n, (_, ids)| n.saturating_mul(1 + ids.len()));
    if combinations > 4096 {
        return false;
    }
    let mut variant = packet.clone();
    for mask in 1..combinations {
        let mut n = mask;
        for (path, ids) in &slots {
            let index = n % (1 + ids.len());
            n /= 1 + ids.len();
            *variant.pointer_mut(path).unwrap() = if index == 0 {
                packet.pointer(path).unwrap().clone()
            } else {
                ids[index - 1].clone()
            };
        }
        if hash_bytes(variant.to_string().as_bytes()) == expected {
            return true;
        }
    }
    false
}

fn evidence_quote(source: &str, quote: &str) -> Option<String> {
    if source.contains(quote) && !quote.is_empty() {
        return Some(quote.into());
    }
    let quote = quote
        .rsplit_once(" (p. ")
        .filter(|(_, page)| {
            page.strip_suffix(')')
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()))
        })
        .map_or(quote, |(s, _)| s);
    let fold = |c| match c {
        '“' | '”' => '"',
        '‘' | '’' => '\'',
        c => c,
    };
    let normalized: String = source.chars().map(fold).collect();
    let query: String = quote.chars().map(fold).collect();
    if query.is_empty() {
        return None;
    }
    let offset = normalized.find(&query)?;
    Some(
        source
            .chars()
            .skip(normalized[..offset].chars().count())
            .take(query.chars().count())
            .collect(),
    )
}

fn apply_correction(
    root: &Path,
    book: &mut Book,
    index: usize,
    verdict: &Verdict,
    source: Option<&SectionView>,
) -> Result<(), ToolError> {
    let record = &book.records[index];
    let template = book.template(&record.finding.area);
    let defines = |k: &str| template.fields.iter().any(|f| f.key == k);
    let mut fields = verdict.corrected_fields.clone().unwrap_or_default();
    let invalid = |s: &str| ToolError::InvalidArgument(s.into());
    if !defines("fields")
        && let Some(nested) = fields.remove("fields")
    {
        let inner: Values =
            serde_json::from_value(nested).map_err(|_| invalid("Expected a nested field map"))?;
        for (k, v) in inner {
            if fields.insert(k, v.clone()).is_some_and(|old| old != v) {
                return Err(invalid("Conflicting nested and top-level correction"));
            }
        }
    }
    let mut detail = verdict.corrected_detail.clone();
    if !defines("detail")
        && let Some(v) = fields.remove("detail")
    {
        let nested = v
            .as_str()
            .ok_or_else(|| invalid("Expected a corrected summary"))?;
        if detail.as_ref().is_some_and(|s| s != nested) {
            return Err(invalid("Conflicting corrected summaries"));
        }
        detail = Some(nested.into());
    }
    let mut citations = vec![];
    if !defines("evidence")
        && let Some(evidence) = fields.remove("evidence")
    {
        let section = record
            .section
            .ok_or_else(|| invalid("Missing source section for evidence"))?;
        let source = source.ok_or_else(|| invalid("Missing source text for evidence"))?;
        for quote in evidence
            .as_array()
            .cloned()
            .unwrap_or_else(|| vec![evidence])
        {
            let text = quote
                .as_str()
                .and_then(|q| evidence_quote(&source.text, q))
                .ok_or_else(|| {
                    invalid("Correction cites a quotation absent from the source section")
                })?;
            citations.push(Citation {
                section,
                evidence: text,
            });
        }
    }
    for field in &template.fields {
        if !matches!(field.format, templates::Format::Links) {
            continue;
        }
        let Some(items) = fields
            .get_mut(&field.key)
            .and_then(serde_json::Value::as_array_mut)
        else {
            continue;
        };
        for link in items {
            if let Some(id) = link["target_id"].as_str().and_then(|s| s.parse().ok()) {
                link["target_id"] = json!(redirected(book, id));
            }
            if link.is_object() && link["context"].is_null() {
                let mut matches = record
                    .finding
                    .fields
                    .get(&field.key)
                    .and_then(|v| v.as_array())
                    .into_iter()
                    .flatten()
                    .filter(|item| {
                        item["name"] == link["name"] && item["target_id"] == link["target_id"]
                    });
                if let Some(item) = matches.next()
                    && matches.next().is_none()
                {
                    link["context"] = item["context"].clone();
                }
            }
        }
    }
    let mut patch = json!({"operation":"edit","record_id":record.id,"fields":fields});
    if let Some(summary) = detail {
        patch["detail"] = json!(summary);
    }
    agent::manage_record(root, book, &patch)?;
    let record = &mut book.records[index];
    for cite in citations {
        if cite.evidence != record.finding.evidence && !record.citations.contains(&cite) {
            record.citations.push(cite);
        }
    }
    Ok(())
}

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Verdict {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflict_citations: Option<Vec<Citation>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrected_detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrected_fields: Option<Values>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    input_stamp: Option<String>,
    pub decision: Decision,
    pub reason: String,
}

#[derive(Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Decision {
    Duplicate,
    Merge,
    SourceConflict,
    ReferralClosed,
    ManuscriptIssue,
    IssueResolved,
    Approve,
    Revise,
    AskAuthor,
}

// Cancelling the parent tool must also stop its reviewer.
struct RunningReview(Arc<dyn SubagentRuntime>, Option<String>);
impl Drop for RunningReview {
    fn drop(&mut self) {
        if let Some(id) = &self.1 {
            let _ = self.0.cancel(id);
        }
    }
}

pub(super) async fn run(
    root: &Path,
    context: &ToolContext,
    id: Option<Uuid>,
) -> Result<ToolResult, ToolError> {
    let book = load(root).map_err(ToolError::Failed)?;
    let candidate = match id {
        Some(id) => Some(
            book.records
                .iter()
                .find(|r| r.id == id && r.pending)
                .ok_or_else(|| {
                    ToolError::InvalidArgument("No pending finding with that ID".into())
                })?,
        ),
        None => book
            .records
            .iter()
            .find(|r| r.pending && r.editorial.is_none()),
    };
    let Some(record) = candidate else {
        return Ok(ToolResult::ok("No unchecked findings remain."));
    };
    let original = json!(record);
    let source = record
        .section
        .map(|id| read_section(root, &book, id))
        .transpose()
        .map_err(ToolError::Failed)?;
    let revision = source.as_ref().map(|s| &s.revision);
    let excerpt = source.as_ref().and_then(|s| {
        let offset = s.text.find(&record.finding.evidence)?;
        let mut start = offset.saturating_sub(2400);
        let mut end = (offset + record.finding.evidence.len() + 2400).min(s.text.len());
        while !s.text.is_char_boundary(start) {
            start -= 1;
        }
        while !s.text.is_char_boundary(end) {
            end += 1;
        }
        Some(&s.text[start..end])
    });
    let existing = candidates(&book, record);
    let prior = prior_verdict(&book, record.id);
    let packet = json!([
        record.finding,
        revision,
        existing,
        book.template(&record.finding.area)
    ]);
    let stamp = hash_bytes(packet.to_string().as_bytes());
    let saved = record.editorial.as_ref();
    let mut verdict = if let Some(previous) = saved.filter(|v| v.decision != Decision::Revise) {
        if matches!(
            previous.decision,
            Decision::ReferralClosed
                | Decision::ManuscriptIssue
                | Decision::AskAuthor
                | Decision::IssueResolved
        ) {
            let _guard = BOOK_IO.lock().map_err(|e| ToolError::Failed(error(e)))?;
            let mut current = load(root).map_err(ToolError::Failed)?;
            review_queue::record_result(&mut current, record.id, record.id, previous.decision, &record.finding.title);
            save(root, &current).map_err(ToolError::Failed)?;
            return Ok(ToolResult::ok(
                json!({"record_id":record.id,"editorial":previous}).to_string(),
            ));
        }
        if previous
            .input_stamp
            .as_ref()
            .is_some_and(|s| s != &stamp && !matches_redirected_stamp(&book, &packet, s))
        {
            // Return this specific finding to its reviewer with the original decision and
            // current evidence. Replaying the unchanged command cannot repair a stale snapshot.
            let _guard = BOOK_IO.lock().map_err(|e| ToolError::Failed(error(e)))?;
            let mut current = load(root).map_err(ToolError::Failed)?;
            let row = current
                .records
                .iter_mut()
                .find(|r| r.id == record.id && json!(r) == original)
                .ok_or_else(|| {
                    ToolError::Stale("Finding changed while routing stale verdict".into())
                })?;
            let job = current
                .review_jobs
                .iter_mut()
                .find(|j| j.id == record.id && j.state == "running")
                .ok_or_else(|| ToolError::Stale("Stale verdict requires an active job".into()))?;
            if job
                .receipts
                .iter()
                .filter(|r| r.get("superseded_verdict").is_some())
                .count()
                >= 2
            {
                job.state = "waiting_for_author".into();
                job.report = Some(
                    "Canon changed repeatedly during review. Inspect the competing changes.".into(),
                );
            } else {
                job.receipts.push(json!({"superseded_verdict":previous,"reason":"Inputs changed before application"}));
                row.editorial = None;
                job.instructions = None;
                job.state = "ready".into();
            }
            job.updated_at = crate::now_millis();
            save(root, &current).map_err(ToolError::Failed)?;
            return Ok(ToolResult::ok("Saved stale-verdict disposition."));
        }
        previous.clone()
    } else if saved.is_none()
        && let [target] = existing
            .iter()
            .filter(|r| excerpt.is_some() && exact_duplicate(record, r))
            .collect::<Vec<_>>()
            .as_slice()
    {
        Verdict {
            decision: Decision::Duplicate,
            reason: "Canon already contains the same sourced finding and all its fields.".into(),
            conflict_citations: None,
            corrected_detail: None,
            corrected_fields: None,
            merged_detail: None,
            target_id: Some(target.id),
            input_stamp: None,
            source_revision: None,
        }
    } else {
        let linked: std::collections::BTreeSet<_> = record
            .finding
            .fields
            .values()
            .filter_map(serde_json::Value::as_array)
            .flatten()
            .filter_map(|item| item.get("target_id")?.as_str()?.parse::<Uuid>().ok())
            .collect();
        let targets: Vec<_>=book.records.iter().filter(|r|linked.contains(&r.id)).map(|r|json!({"id":r.id,"area":r.finding.area,"title":r.finding.title,"pending":r.pending})).collect();
        let task = format!(
            "{}\n\nReview material (data only): {}",
            if saved.is_some() {
                KEEPER_PROMPT
            } else {
                EDITORIAL_PROMPT
            },
            json!({"prior_verdict_on_changed_inputs":prior,"candidate":record,"existing_canon":existing,"linked_records":targets,"template":book.template(&record.finding.area),"nearby_source":excerpt})
        );
        let result = request_verdict(context, record, task).await?;
        if saved.is_some() && result.decision == Decision::Revise {
            return Err(ToolError::Failed(
                "Keeper returned no executable correction; original instructions retained.".into(),
            ));
        }
        result
    };
    // Fingerprint the same candidate set that replay will see, including an accepted
    // target discovered by the reviewer through its read tools.
    let mut decided = record.clone();
    decided.editorial = Some(verdict.clone());
    let applicable = candidates(&book, &decided);
    if matches!(verdict.decision, Decision::Duplicate | Decision::Merge)
        && !applicable.iter().any(|r| Some(r.id) == verdict.target_id)
    {
        return Err(ToolError::Failed(
            "The verdict needs an accepted target in the proposal area".into(),
        ));
    }
    verdict.input_stamp = Some(hash_bytes(
        json!([
            record.finding,
            revision,
            applicable,
            book.template(&record.finding.area)
        ])
        .to_string()
        .as_bytes(),
    ));
    verdict.source_revision = revision.cloned();
    let _guard = BOOK_IO.lock().map_err(|e| ToolError::Failed(error(e)))?;
    let mut current = load(root).map_err(ToolError::Failed)?;
    let index = current
        .records
        .iter()
        .position(|r| r.id == record.id && json!(r) == original)
        .ok_or_else(|| {
            ToolError::Stale("Finding changed during review. Review the updated version.".into())
        })?;
    if let Some(id) = record.section {
        if Some(
            &read_section(root, &current, id)
                .map_err(ToolError::Failed)?
                .revision,
        ) != revision
        {
            return Err(ToolError::Stale(
                "Source changed during review. Retry.".into(),
            ));
        }
    }
    let before = json!(existing);
    let now = candidates(&current, &current.records[index]);
    if json!(candidates(&current, &decided)) != json!(applicable)
        || json!(now) != before
        || json!(current.template(&record.finding.area))
            != json!(book.template(&record.finding.area))
    {
        return Err(ToolError::Stale(
            "Canon or template changed during review. Retry.".into(),
        ));
    }
    // Older models used manuscript_issue for a new source conflict. Route that
    // response through triage too; existing open issues keep their old meaning.
    if matches!(
        verdict.decision,
        Decision::SourceConflict | Decision::ManuscriptIssue
    ) {
        verdict.decision = Decision::SourceConflict;
        let conflict =
            editorial::from_record(&current.records[index], &verdict).map_err(ToolError::Failed)?;
        let child = editorial::refer(root, &mut current, record.id, Some(record.id), conflict)
            .map_err(ToolError::Failed)?;
        current.records[index].editorial = Some(verdict.clone());
        review_queue::record_result(&mut current, record.id, record.id, verdict.decision, &record.finding.title);
        save(root, &current).map_err(ToolError::Failed)?;
        return Ok(ToolResult::ok(
            json!({"record_id":record.id,"editorial_job":child,"editorial":verdict}).to_string(),
        )
        .with_metadata(json!({"book_records_changed":true})));
    }
    if matches!(
        verdict.decision,
        Decision::ReferralClosed | Decision::IssueResolved
    ) {
        return Err(ToolError::Failed(
            "Only Editorial can close a referral or verify a manuscript fix".into(),
        ));
    }
    current.records[index].editorial = Some(verdict.clone());
    // Persist the executable decision before applying. After interruption the same
    // proposal resumes here using its saved verdict, without invoking a model.
    save(root, &current).map_err(ToolError::Failed)?;
    if verdict.corrected_detail.is_some() || verdict.corrected_fields.is_some() {
        if !matches!(verdict.decision, Decision::Approve | Decision::Merge) {
            return Err(ToolError::InvalidArgument(
                "Corrections require approve or merge.".into(),
            ));
        }
        apply_correction(root, &mut current, index, &verdict, source.as_ref())?;
    }
    let saved_id = match verdict.decision {
        Decision::Duplicate | Decision::Merge => {
            let target = verdict.target_id.ok_or_else(|| {
                ToolError::Failed("Reviewer must identify the accepted target.".into())
            })?;
            let expected = book
                .records
                .iter()
                .find(|r| r.id == target && !r.pending && applicable.iter().any(|c| c.id == target))
                .ok_or_else(|| {
                    ToolError::Failed("Reviewer selected an unknown canon target.".into())
                })?;
            let dest = current
                .records
                .iter()
                .position(|r| r.id == target && json!(r) == json!(expected))
                .ok_or_else(|| ToolError::Stale("Merge target changed. Retry.".into()))?;
            if verdict.decision == Decision::Merge {
                if let Some(source) = &source {
                    validate_finding(&record.finding, &book.areas(), &source.text)
                        .map_err(ToolError::Failed)?;
                }
                if !record.finding.fields.is_empty() {
                    check_entry(&current, &record.finding).map_err(ToolError::Failed)?;
                }
            }
            let id = consolidate(
                &mut current,
                index,
                dest,
                verdict.decision == Decision::Merge,
            )
            .map_err(ToolError::Failed)?;
            if let Some(detail) = &verdict.merged_detail {
                let patch = json!({"operation":"edit","record_id":id,"detail":detail});
                agent::manage_record(root, &mut current, &patch)?;
            }
            id
        }
        Decision::Approve => approve(root, &mut current, index).map_err(ToolError::Failed)?,
        Decision::IssueResolved => {
            return Err(ToolError::Failed(
                "Only a verified manuscript edit can resolve an issue.".into(),
            ));
        }
        Decision::ManuscriptIssue if source.is_none() => {
            return Err(ToolError::Failed(
                "Manuscript issue needs a source section.".into(),
            ));
        }
        _ => record.id,
    };
    let pending = current
        .records
        .iter()
        .any(|r| r.id == saved_id && r.pending);
    review_queue::record_result(&mut current, record.id, saved_id, verdict.decision, &record.finding.title);
    save(root, &current).map_err(ToolError::Failed)?;
    Ok(ToolResult::ok(
        json!({"record_id":saved_id,"pending":pending,"editorial":verdict}).to_string(),
    )
    .with_metadata(json!({"book_records_changed":true})))
}

pub(super) fn resolve_issue(
    root: &Path,
    book: &mut Book,
    id: Uuid,
    revision: &str,
    passage: &str,
    resolution: &str,
) -> Result<(), String> {
    let record = book
        .records
        .iter()
        .find(|r| r.id == id)
        .ok_or("Unknown issue")?;
    let verdict = record
        .editorial
        .as_ref()
        .filter(|v| v.decision == Decision::ManuscriptIssue)
        .ok_or("No open manuscript issue for this ID")?;
    let section = read_section(
        root,
        &book,
        record.section.ok_or("Missing manuscript section")?,
    )?;
    if revision != section.revision || verdict.source_revision.as_deref() == Some(revision) {
        return Err("Edit the affected manuscript section and read its current revision before closing the issue.".into());
    }
    if passage.trim().is_empty() || !section.text.contains(passage) || resolution.trim().is_empty()
    {
        return Err("Provide a resolution and a verbatim passage from the revised section.".into());
    }
    let note = format!(
        "{}\nResolved: {}\nVerified passage: {}",
        verdict.reason,
        resolution.trim(),
        passage
    );
    let verdict = book
        .records
        .iter_mut()
        .find(|r| r.id == id)
        .unwrap()
        .editorial
        .as_mut()
        .unwrap();
    verdict.decision = Decision::IssueResolved;
    verdict.reason = note;
    verdict.source_revision = Some(revision.into());
    Ok(())
}

fn exact_duplicate(candidate: &Record, canon: &Record) -> bool {
    let title = |r: &Record| {
        let value = r.finding.title.trim().to_lowercase();
        if candidate.finding.kind == FindingKind::Question {
            value
                .strip_prefix("open question: ")
                .unwrap_or(&value)
                .to_owned()
        } else {
            value
        }
    };
    candidate.section.is_some()
        && candidate.section == canon.section
        && !candidate.finding.evidence.is_empty()
        && candidate.finding.evidence == canon.finding.evidence
        && title(candidate) == title(canon)
        && candidate.finding.area == canon.finding.area
        && candidate.finding.detail.trim() == canon.finding.detail.trim()
        && candidate.finding.fields.iter().all(|(key, v)| {
            match (canon.finding.fields.get(key), v.as_array()) {
                (Some(serde_json::Value::Array(saved)), Some(proposed)) => {
                    proposed.iter().all(|item| saved.contains(item))
                }
                (saved, _) => saved == Some(v),
            }
        })
        && candidate
            .answer
            .as_ref()
            .is_none_or(|v| canon.answer.as_ref() == Some(v))
}

fn prior_verdict(book: &Book, id: Uuid) -> Option<&serde_json::Value> {
    book.review_jobs
        .iter()
        .find(|j| j.id == id)?
        .receipts
        .iter()
        .rev()
        .find_map(|r| r.get("superseded_verdict"))
}

fn candidates<'a>(book: &'a Book, candidate: &Record) -> Vec<&'a Record> {
    let previous_target = prior_verdict(book, candidate.id)
        .and_then(|v| v["target_id"].as_str())
        .and_then(|v| v.parse::<Uuid>().ok());
    // Include alternative titles for the same passage as well as same-name records.
    book.records
        .iter()
        .filter(|r| {
            !r.pending
                && r.finding.area == candidate.finding.area
                && (r
                    .finding
                    .title
                    .eq_ignore_ascii_case(&candidate.finding.title)
                    || (r.section == candidate.section
                        && !r.finding.evidence.is_empty()
                        && r.finding.evidence == candidate.finding.evidence)
                    || r.finding.detail == candidate.finding.detail
                    || Some(r.id) == previous_target
                    || candidate.finding.detail.contains(&r.id.to_string())
                    || candidate.editorial.as_ref().is_some_and(|v| {
                        v.reason.contains(&r.id.to_string()) || v.target_id == Some(r.id)
                    }))
        })
        .collect()
}

async fn request_verdict(
    context: &ToolContext,
    record: &Record,
    task: String,
) -> Result<Verdict, ToolError> {
    let title = format!(
        "{} {}",
        if task.starts_with(KEEPER_PROMPT) {
            "Apply verdict for"
        } else {
            "Review"
        },
        record.finding.title
    );
    let result = read_worker(context, title, task).await?;
    parse_verdict(&result)
}

pub(super) async fn read_worker(
    context: &ToolContext,
    title: String,
    task: String,
) -> Result<String, ToolError> {
    let runtime = context
        .subagents
        .clone()
        .ok_or_else(|| ToolError::Failed("Book worker runtime unavailable.".into()))?;
    let worker = runtime
        .spawn(SubagentSpec {
            title,
            task,
            model: None,
            allow_writes: false,
            allow_commands: false,
        })
        .map_err(ToolError::Failed)?;
    let mut guard = RunningReview(runtime.clone(), Some(worker.id.clone()));
    let result = loop {
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        let state = runtime
            .get(&worker.id)
            .map_err(ToolError::Failed)?
            .ok_or_else(|| {
                ToolError::Failed("Reviewer unavailable; finding remains queued.".into())
            })?;
        match state.status.as_str() {
            "completed" => break state.result,
            "cancelled" | "failed" => {
                return Err(ToolError::Failed(format!(
                    "Reviewer {}. Finding remains queued. {}",
                    state.status,
                    state.error.unwrap_or_default()
                )));
            }
            _ => {}
        }
    };
    guard.1 = None;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn exact_duplicate_closes_locally_and_keeps_backlinks() {
        use canonwright_workspace::WorkspaceService;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        initialize(root, Some("novel"), None, None).unwrap();
        let sec = add_at(root, "Village", "Mara lives here.").unwrap();
        let mut book = load(root).unwrap();
        let finding:Finding=serde_json::from_value(json!({"kind":"entry","area":"Characters","title":"Mara","detail":"Resident.","evidence":"Mara lives here.","fields":{"role":"Resident"}})).unwrap();
        let canon = insert_finding(&mut book, Some(sec), finding.clone(), false);
        let proposal = insert_finding(&mut book, Some(sec), finding, true);
        let place:Finding=serde_json::from_value(json!({"kind":"entry","area":"Locations","title":"Village","detail":"Her home.","evidence":"Mara lives here.","fields":{"inhabitants":[{"name":"Mara","context":"Lives here","target_id":proposal}]}})).unwrap();
        insert_finding(&mut book, Some(sec), place, false);
        save(root, &book).unwrap();
        let ctx = ToolContext::new(Arc::new(WorkspaceService::new(root)));
        run(root, &ctx, Some(proposal)).await.unwrap();
        let book = load(root).unwrap();
        assert_eq!(book.records.len(), 2);
        assert_eq!(
            book.records[1].finding.fields["inhabitants"][0]["target_id"],
            json!(canon)
        );
        assert!(book.records[0].editorial.as_ref().unwrap().decision == Decision::Duplicate);
    }

    #[test]
    fn manuscript_issue_requires_verified_changed_section() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        initialize(root, Some("novel"), None, None).unwrap();
        let sec = add_at(root, "Arrival", "Two gates.").unwrap();
        let mut book = load(root).unwrap();
        let finding:Finding=serde_json::from_value(json!({"kind":"note","area":"Locations","title":"Gate count","detail":"Conflicts with map.","evidence":"Two gates."})).unwrap();
        let id = insert_finding(&mut book, Some(sec), finding, true);
        let original = read_section(root, &book, sec).unwrap().revision;
        book.records.last_mut().unwrap().editorial = Some(Verdict {
            decision: Decision::ManuscriptIssue,
            reason: "Correct the gate count.".into(),
            conflict_citations: None,
            corrected_detail: None,
            corrected_fields: None,
            merged_detail: None,
            target_id: None,
            source_revision: Some(original.clone()),
            input_stamp: None,
        });
        save(root, &book).unwrap();
        assert!(resolve_issue(root, &mut book, id, &original, "Two gates.", "Fixed").is_err());
        write(&section_path(root, sec).unwrap(), "Three gates.").unwrap();
        let revised = read_section(root, &book, sec).unwrap().revision;
        assert!(resolve_issue(root, &mut book, id, &revised, "Four gates.", "Fixed").is_err());
        resolve_issue(
            root,
            &mut book,
            id,
            &revised,
            "Three gates.",
            "Matched the map.",
        )
        .unwrap();
        save(root, &book).unwrap();
        assert!(
            load(root)
                .unwrap()
                .records
                .last()
                .unwrap()
                .editorial
                .as_ref()
                .unwrap()
                .decision
                == Decision::IssueResolved
        );
    }

    #[tokio::test]
    async fn replays_wrapped_merge_and_redirects_links_without_a_model() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        initialize(root, None, None, None).unwrap();
        let section = add_at(root, "Village", "A visitor. “Come inside.”").unwrap();
        let mut book = load(root).unwrap();
        let finding:Finding=serde_json::from_value(json!({"kind":"entry","area":"Characters","title":"Visitor","detail":"A visitor.","evidence":"A visitor."})).unwrap();
        let target = insert_finding(&mut book, Some(section), finding.clone(), false);
        let mut neighbor = finding.clone();
        neighbor.title = "Neighbor".into();
        neighbor.evidence = "Come inside.".into();
        let linked = insert_finding(&mut book, Some(section), neighbor, false);
        let candidate = insert_finding(&mut book, Some(section), finding, true);
        let old = Uuid::now_v7();
        book.review_jobs.push(review_queue::Job {
            id: old,
            result_id: Some(linked),
            state: "done".into(),
            ..Default::default()
        });
        let proposal = book.records.last_mut().unwrap();
        proposal.finding.fields.insert(
            "relationships".into(),
            json!([{"name":"Neighbor","target_id":linked,"context":"a friend"}]),
        );
        proposal.editorial=Some(serde_json::from_value(json!({"decision":"merge","target_id":target,"reason":"Same visitor.","corrected_fields":{"detail":"A neighbor visits.","fields":{"relationships":[{"name":"Neighbor","target_id":old}],"evidence":["\"Come inside.\" (p. 3)"]}}})).unwrap());
        save(root, &book).unwrap();
        let cx = ToolContext::new(std::sync::Arc::new(
            canonwright_workspace::WorkspaceService::new(root),
        ));
        run(root, &cx, Some(candidate)).await.unwrap();
        let book = load(root).unwrap();
        let record = &book.records[0];
        assert_eq!(record.id, target);
        assert!(record.finding.detail.contains("A neighbor visits."));
        assert_eq!(
            record.finding.fields["relationships"][0],
            json!({"name":"Neighbor","target_id":linked,"context":"a friend"})
        );
        assert_eq!(record.citations[0].evidence, "“Come inside.”");
    }

    #[test]
    fn accepts_the_terminal_verdict_after_a_report() {
        let json = r#"{"decision":"revise","reason":"Resolve the duplicate {IDs}."}"#;
        for response in [
            json.to_owned(),
            format!("Checked the manuscript.\n\n{json}"),
            format!("Read the source.\n```JSON\n{json}\n```"),
            format!("```\n{json}\n```"),
            format!("{json}\nI applied the saved instructions."),
            format!("```json\n{json}\n```\nCompleted."),
        ] {
            let verdict = parse_verdict(&response).unwrap();
            assert!(verdict.decision == Decision::Revise);
            assert_eq!(verdict.reason, "Resolve the duplicate {IDs}.");
        }
    }

    #[test]
    fn never_uses_an_earlier_example_or_an_incomplete_verdict() {
        for response in [
            "Review complete",
            "{\"decision\":\"approve\",\"reason\":\"ok\"}\n{\"decision\":\"revise\",\"reason\":\"no\"}",
            "{\"decision\":\"approve\",\"reason\":\" \"}",
            "Example:\n{\"decision\":\"approve\",\"reason\":\"ok\"}\nFinal: unknown",
            "Report\n{\"decision\":\"accept\",\"reason\":\"ok\"}",
        ] {
            assert!(parse_verdict(response).is_err());
        }
    }
}
