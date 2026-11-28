//! Author decisions are cited to saved chat messages, independently of manuscript evidence.
use super::*;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Statement {
    pub session: String,
    pub message: u64,
    pub quote: String,
}

pub(crate) fn messages(rows: Vec<canonwright_store::types::SessionMessage>) -> Vec<Statement> {
    rows.into_iter()
        .filter(|row| row.role == "user")
        .filter_map(|row| {
            let text = row.content["display"].as_str()?;
            Some(Statement {
                session: row.session_id.to_string(),
                message: row.seq,
                quote: text.into(),
            })
        })
        .collect()
}

// References are resolved only against the caller's saved source set. Including
// the text in the ID keeps it stable when a selected passage enters a worker job.
fn passages(sources: &[Statement]) -> impl Iterator<Item = (String, Statement)> + '_ {
    sources.iter().rev().flat_map(|source| {
        source
            .quote
            .split_inclusive(['.', '!', '?', '\n'])
            .flat_map(|sentence| {
                let mut rest = sentence.trim();
                std::iter::from_fn(move || {
                    if rest.is_empty() {
                        return None;
                    }
                    let end = rest.char_indices().nth(1024).map_or(rest.len(), |(i, _)| i);
                    let (part, tail) = rest.split_at(end);
                    rest = tail.trim_start();
                    Some(part.trim_end())
                })
            })
            .filter(|text| !text.is_empty())
            .map(move |text| {
                let id = format!(
                    "{}:{}:{}",
                    source.session,
                    source.message,
                    hash_bytes(text.as_bytes())
                );
                (
                    id,
                    Statement {
                        quote: text.into(),
                        ..source.clone()
                    },
                )
            })
    })
}

pub(super) fn list_passages(sources: &[Statement], query: &str, offset: usize) -> Value {
    let needle = query.to_lowercase();
    let mut matches = passages(sources)
        .filter(|(_, s)| s.quote.to_lowercase().contains(&needle))
        .skip(offset);
    let rows: Vec<_> = matches
        .by_ref()
        .take(12)
        .map(|(id, s)| json!({"source_ref":id,"message":s.message,"text":s.quote}))
        .collect();
    json!({"passages":rows,"next_offset":matches.next().map(|_|offset+rows.len())})
}

pub(super) fn select_sources(
    sources: &[Statement],
    args: &Value,
    legacy: &str,
) -> Result<Vec<Statement>, String> {
    if args["source_refs"].is_null() {
        return excerpts(sources, &args[legacy]);
    }
    if !args[legacy].is_null() {
        return Err("Choose source_refs or literal quotes".into());
    }
    let refs: Vec<String> = serde_json::from_value(args["source_refs"].clone()).map_err(error)?;
    if !(1..=24).contains(&refs.len()) {
        return Err("Select 1–24 saved passages".into());
    }
    let found: std::collections::HashMap<_, _> = passages(sources)
        .filter(|(id, _)| refs.contains(id))
        .collect();
    let mut selected = Vec::new();
    for id in refs {
        let passage = found.get(&id).ok_or_else(|| {
            format!("Unknown source reference {id}. Use book_author_passages in this assignment.")
        })?;
        if !selected.contains(passage) {
            selected.push(passage.clone());
        }
    }
    Ok(selected)
}

pub(super) fn excerpts(sources: &[Statement], quotes: &Value) -> Result<Vec<Statement>, String> {
    let quotes: Vec<String> = serde_json::from_value(quotes.clone()).map_err(error)?;
    if quotes.is_empty() || quotes.len() > 24 {
        return Err("Supply 1–24 exact author quotations".into());
    }
    let mut cited = vec![];
    for (index, quote) in quotes.into_iter().enumerate() {
        if quote.trim().is_empty() || quote.len() > 16000 {
            return Err("Use a nonempty quotation of at most 16 KB".into());
        }
        let source=sources.iter().rev().find(|s|s.quote.contains(&quote)).ok_or_else(||format!("Quotation {} does not match a saved author passage. Use book_author_passages and pass source_refs.", index+1))?;
        let citation = Statement {
            quote,
            ..source.clone()
        };
        if !cited.contains(&citation) {
            cited.push(citation);
        }
    }
    Ok(cited)
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct ResearchSource {
    source: String,
    basis: String,
}

// A resolved namesake is only a replacement when both label and relationship match
// and every matching resolved link agrees on the target.
fn remove_resolved_twins(values: &mut Vec<Value>) -> usize {
    let resolved: Vec<_> = values
        .iter()
        .filter(|v| v["target_id"].is_string())
        .cloned()
        .collect();
    let before = values.len();
    values.retain(|v| {
        if !v.is_object() || v.get("target_id").is_some() || v.as_object().unwrap().len() != 2 {
            return true;
        }
        let mut twins = resolved
            .iter()
            .filter(|r| r["name"] == v["name"] && r["context"] == v["context"]);
        let Some(first) = twins.next() else {
            return true;
        };
        twins.any(|r| r["target_id"] != first["target_id"])
    });
    before - values.len()
}

pub(super) fn clean_research(root: &Path, job_id: Uuid, ids: &[Uuid]) -> Result<Value, String> {
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(root)?;
    if !book.review_jobs.iter().any(|j| {
        j.id == job_id
            && j.state == "running"
            && j.work.as_ref().is_some_and(|w| w.role == "canon_keeper")
    }) {
        return Err("Requires an active Canon Keeper assignment".into());
    }
    if ids.is_empty() || ids.len() > 100 {
        return Err("Supply 1–100 research record IDs".into());
    }
    let mut paragraphs = 0;
    let mut links = 0;
    for id in ids {
        let record = book
            .records
            .iter_mut()
            .find(|r| r.id == *id && !r.pending && !r.research_sources.is_empty())
            .ok_or("Expected an existing research entry")?;
        if let Some((first, rest)) = record.finding.detail.split_once("\n\n")
            && !first.is_empty()
            && rest.split("\n\n").next() == Some(first)
        {
            record.finding.detail = rest.to_owned();
            paragraphs += 1;
        }
        for v in record
            .finding
            .fields
            .values_mut()
            .filter_map(Value::as_array_mut)
        {
            links += remove_resolved_twins(v);
        }
    }
    save(root, &book)?;
    Ok(
        json!({"records":ids,"duplicate_paragraphs_removed":paragraphs,"unresolved_twins_removed":links}),
    )
}

pub(super) fn save_entry(
    root: &Path,
    job: Uuid,
    args: &Value,
    research: bool,
) -> Result<Uuid, String> {
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(root)?;
    let work = book
        .review_jobs
        .iter()
        .find(|j| j.id == job && j.state == "running")
        .and_then(|j| j.work.as_ref())
        .filter(|w| w.role == "canon_keeper")
        .ok_or("Requires an active Canon Keeper assignment")?;
    let (citations, source) = if research {
        let source = args["source"].as_str().unwrap_or("").trim();
        let basis = args["basis"].as_str().unwrap_or("");
        if source.is_empty()
            || source.len() > 4000
            || !matches!(basis, "checked_source" | "unverified_lead")
        {
            return Err("Provide the research source and its verification status".into());
        }
        (
            vec![],
            Some(ResearchSource {
                source: source.into(),
                basis: basis.into(),
            }),
        )
    } else {
        (
            select_sources(&work.focus.author_sources, args, "quotes")?,
            None,
        )
    };
    let finding:Finding=serde_json::from_value(json!({"area":args["area"],"kind":"entry","title":args["title"],"detail":args["detail"],"fields":args["fields"],"evidence":""})).map_err(error)?;
    validate_fields(&finding, &book.areas())?;
    check_entry(&book, &finding)?;
    let id = if let Some(record) = book.records.iter_mut().find(|r| {
        r.finding.area == finding.area
            && r.finding
                .title
                .trim()
                .eq_ignore_ascii_case(finding.title.trim())
    }) {
        if record.pending
            || (research && record.research_sources.is_empty())
            || (!research && record.author_sources.is_empty())
        {
            return Err("An entry with different provenance already has this name. Reconcile its identity before adding to it.".into());
        }
        // Repeated proposals add only new information; preserve existing text and links.
        for (key, incoming) in finding.fields {
            match record.finding.fields.get_mut(&key) {
                None => {
                    record.finding.fields.insert(key, incoming);
                }
                Some(saved) if *saved == incoming => {}
                Some(Value::Array(saved)) if incoming.is_array() => {
                    for v in incoming.as_array().unwrap() {
                        if !saved.contains(v) {
                            saved.push(v.clone());
                        }
                    }
                    remove_resolved_twins(saved);
                }
                Some(_) => {
                    return Err(format!(
                        "Existing {key} differs. Resolve the conflicting values explicitly."
                    ));
                }
            }
        }
        for paragraph in finding.detail.split("\n\n") {
            if !paragraph.is_empty()
                && !record
                    .finding
                    .detail
                    .split("\n\n")
                    .any(|saved| saved == paragraph)
            {
                record.finding.detail.push_str("\n\n");
                record.finding.detail.push_str(paragraph);
            }
        }
        record.id
    } else {
        insert_finding(&mut book, None, finding, false)
    };
    let index = book.records.iter().position(|r| r.id == id).unwrap();
    validate_fields(&book.records[index].finding, &book.areas())?;
    check_entry(&book, &book.records[index].finding)?;
    let record = &mut book.records[index];
    if let Some(source) = source
        && !record.research_sources.contains(&source)
    {
        record.research_sources.push(source);
    }
    for c in citations {
        if !record.author_sources.contains(&c) {
            record.author_sources.push(c);
        }
    }
    save(root, &book)?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(root: &Path, job: Uuid, args: &Value) -> Result<Uuid, String> {
        save_entry(root, job, args, false)
    }
    #[test]
    fn research_in_custom_area_retains_provenance_on_replay() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        initialize(root, None, None, None).unwrap();
        let mut book = load(root).unwrap();
        book.areas = Some(vec!["Myths".into()]);
        let key = book
            .template("Myths")
            .fields
            .into_iter()
            .find(|f| f.format == templates::Format::Text)
            .unwrap()
            .key;
        save(root, &book).unwrap();
        let job = review_queue::submit(
            root,
            "canon_keeper",
            "Save a research lead",
            &agent::Focus::book("test".into()),
        )
        .unwrap();
        let mut args = json!({"area":"Myths","title":"An account","detail":"An unconfirmed lead.","fields":{(key):"Source context"},"source":"Reading notes, page 5","basis":"unverified_lead"});
        assert!(save_entry(root, job, &args, true).is_err());
        review_queue::update_job(root, job, |j| {
            j.state = "running".into();
            Ok(())
        })
        .unwrap();
        let id = save_entry(root, job, &args, true).unwrap();
        assert_eq!(id, save_entry(root, job, &args, true).unwrap());
        args["detail"] = json!("An unconfirmed lead.\n\nA second observation.");
        save_entry(root, job, &args, true).unwrap();
        assert_eq!(
            load(root).unwrap().records[0].finding.detail,
            args["detail"].as_str().unwrap()
        );
        args["basis"] = json!("invented");
        assert!(save_entry(root, job, &args, true).is_err());
        let saved = load(root).unwrap();
        let r = &saved.records[0];
        assert_eq!(saved.records.len(), 1);
        assert!(r.author_sources.is_empty() && r.section.is_none());
        assert_eq!(r.research_sources.len(), 1);
        assert_eq!(r.research_sources[0].basis, "unverified_lead");
    }
    #[test]
    fn link_cleanup_preserves_distinct_relationships_and_ambiguous_names() {
        let lead = json!({"name":"A","context":"mentor"});
        let target = json!({"name":"A","context":"mentor","target_id":"1"});
        let other = json!({"name":"A","context":"friend"});
        let mut values = vec![lead.clone(), other.clone(), target.clone()];
        assert_eq!(remove_resolved_twins(&mut values), 1);
        assert_eq!(values, vec![other, target.clone()]);
        values.extend([
            lead.clone(),
            json!({"name":"A","context":"mentor","target_id":"2"}),
        ]);
        assert_eq!(remove_resolved_twins(&mut values), 0);
        assert!(values.contains(&lead));
    }

    #[test]
    fn passage_references_survive_handoff_and_cannot_escape_the_assignment() {
        let source = Statement {
            session: "chat".into(),
            message: 7,
            quote: "She said “wait”—then left. He stayed.".into(),
        };
        let sources = vec![source];
        let list = list_passages(&sources, "WAIT", 0);
        let id = &list["passages"][0]["source_ref"];
        let args = json!({"source_refs":[id]});
        let assignment = select_sources(&sources, &args, "author_quotes").unwrap();
        assert_eq!(assignment[0].quote, "She said “wait”—then left.");
        let keeper = select_sources(&assignment, &args, "quotes").unwrap();
        assert_eq!(keeper, assignment);
        let outside = list_passages(&sources, "stayed", 0);
        let invalid_batch = json!({"source_refs":[id, outside["passages"][0]["source_ref"]]});
        assert!(select_sources(&assignment, &invalid_batch, "quotes").is_err());
        assert!(
            select_sources(
                &assignment,
                &json!({"source_refs":[id],"quotes":["wait"]}),
                "quotes"
            )
            .is_err()
        );
        assert!(
            excerpts(&sources, &json!(["He stayed.", "wrong"]))
                .unwrap_err()
                .contains("Quotation 2")
        );
    }
    #[test]
    fn author_facts_need_an_exact_author_message_and_survive_replay() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        initialize(root, None, None, None).unwrap();
        let section = load(root).unwrap().sections[0].id;
        let before = read_section(root, &load(root).unwrap(), section).unwrap();
        let mut focus = agent::Focus::book("test".into());
        focus.author_sources = vec![Statement {
            session: "chat".into(),
            message: 7,
            quote: "Al is her father.".into(),
        }];
        let job = review_queue::submit(root, "canon_keeper", "Save premise", &focus).unwrap();
        review_queue::update_job(root, job, |j| {
            j.state = "running".into();
            Ok(())
        })
        .unwrap();
        let mut args = json!({"area":"Characters","title":"Al","detail":"Her father.","fields":{"role":"Father"},"quotes":["Al is a wizard."]});
        assert!(record(root, job, &args).is_err());
        assert!(load(root).unwrap().records.is_empty());
        args["quotes"] = json!(["Al is her father."]);
        let id = record(root, job, &args).unwrap();
        args.as_object_mut().unwrap().remove("quotes");
        args["source_refs"] =
            json!([list_passages(&focus.author_sources, "father", 0)["passages"][0]["source_ref"]]);
        assert_eq!(record(root, job, &args).unwrap(), id);
        let b = load(root).unwrap();
        assert_eq!(b.records.len(), 1);
        let r = &b.records[0];
        assert!(!r.pending && r.section.is_none() && r.finding.evidence.is_empty());
        assert_eq!(r.author_sources, focus.author_sources);
        assert_eq!(
            before.revision,
            read_section(root, &b, section).unwrap().revision
        );
    }
}
