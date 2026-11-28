use super::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(super) type Values = BTreeMap<String, Value>;
pub(super) type Templates = BTreeMap<String, Template>;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Format {
    Text,
    List,
    Links,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Field {
    pub key: String,
    pub label: String,
    pub guidance: String,
    pub format: Format,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Template {
    pub revision: u64,
    pub fields: Vec<Field>,
}

impl Book {
    pub(super) fn template(&self, area: &str) -> Template {
        self.templates
            .get(area)
            .cloned()
            .unwrap_or_else(|| builtin(area))
    }
}

fn builtin(area: &str) -> Template {
    // A trailing * marks a text list and @ marks record links. Guidance is also presented to the writer in the entry editor.
    let rows: &[(&str, &str)] = match area {
        "Characters" => &[
            ("Role", "What part does this person play in the story?"),
            ("Aliases*", "Names and titles used for this person."),
            ("Motivation", "What do they want, and why?"),
            ("Stakes", "What would failure cost them?"),
            (
                "Relationships@",
                "Name each connection and what binds or divides them.",
            ),
            (
                "Places@",
                "Link home, origin, places visited, or places governed; describe the connection.",
            ),
            (
                "Affiliations@",
                "Link factions and describe membership or allegiance.",
            ),
            ("Voice", "Distinctive speech or habits shown in the text."),
            (
                "Turning points*",
                "Decisions or events that change their course.",
            ),
        ],
        "Locations" => &[
            (
                "Place type",
                "Settlement, landmark, region, or other kind of place.",
            ),
            ("Region@", "Where is it, relative to known places?"),
            (
                "Features*",
                "Distinctive sights, sounds, terrain, or structures.",
            ),
            ("Inhabitants@", "Who lives here or controls access?"),
            (
                "Routes@",
                "How do people enter, leave, or travel through it?",
            ),
            (
                "Significance",
                "What happens here that matters to the book?",
            ),
        ],
        "Factions" => &[
            ("Purpose", "What holds the group together?"),
            ("Bases@", "Link places where the group is based or active."),
            ("Agenda", "What change is it trying to bring about?"),
            ("Members@", "Named members and their roles."),
            ("Resources*", "People, assets, or authority it can call on."),
            ("Allies@", "Groups or people supporting it."),
            ("Rivals@", "Who opposes it, and over what?"),
        ],
        "Timeline" => &[
            ("Event", "What changed?"),
            ("Timing", "An explicit date or relative order."),
            ("Participants@", "Who took part?"),
            ("Location@", "Where did it occur?"),
            ("Consequences*", "What followed because of this event?"),
        ],
        "Story threads" => &[
            (
                "Dramatic question",
                "What unresolved question drives this thread?",
            ),
            ("Setup", "Where and how is it introduced?"),
            ("Developments*", "What complicates or advances it?"),
            ("Payoff", "How is it resolved, if shown?"),
        ],
        "World rules" => &[
            ("Principle", "What pattern or law operates in this world?"),
            ("Scope", "Where and for whom does it apply?"),
            ("Exceptions*", "Established limits or exceptions."),
            ("Consequences*", "What happens when it operates?"),
            (
                "Belief or fact",
                "Who believes this, or how is it established?",
            ),
        ],
        "Terms" => &[
            ("Definition", "What precisely does this term mean?"),
            ("Scope", "Which subsystem uses it?"),
            (
                "Related terms@",
                "Distinguish terms a reader might confuse.",
            ),
        ],
        "Procedures" => &[
            ("Trigger", "When should this procedure begin?"),
            ("Inputs*", "What does the table need before starting?"),
            ("Steps*", "Actions in play order."),
            ("Outcomes*", "Possible results and their effects."),
            ("Exceptions*", "When is the normal sequence altered?"),
        ],
        "Examples" => &[
            ("Rule illustrated@", "Which rule does this demonstrate?"),
            ("Setup", "Starting situation and inputs."),
            ("Decisions*", "Actions and calculations in order."),
            ("Result", "What outcome demonstrates the rule?"),
        ],
        "Player options" => &[
            ("Eligibility", "Who may choose this option?"),
            ("Cost", "What is spent or given up?"),
            ("Effect", "What changes in play?"),
            ("Limits*", "Timing, uses, restrictions, or prerequisites."),
        ],
        "Stat blocks" => &[
            ("Role", "How does this entity function in play?"),
            ("Statistics*", "Named statistics with values and units."),
            ("Actions*", "Actions and their mechanical effects."),
            ("Defenses*", "Defenses, resistances, and vulnerabilities."),
        ],
        "Errata" => &[
            ("Affected rule@", "Which passage or rule is affected?"),
            ("Problem", "What is incorrect or ambiguous?"),
            ("Correction", "What replacement has been agreed?"),
            ("Impact", "What other rules or examples are affected?"),
        ],
        "Encounters" => &[
            ("Site@", "Link the encounter location."),
            ("Participants@", "Link characters and factions present."),
            ("Situation", "What do the players encounter?"),
            ("Objective", "What can the participants achieve?"),
            ("Opposition*", "Opponents, obstacles, and pressures."),
            ("Approaches*", "Supported ways to engage."),
            ("Outcomes*", "How can the situation change?"),
        ],
        "Clues" => &[
            ("Discovery", "What can the players learn?"),
            ("Source", "Who or what provides the clue?"),
            ("Access", "How can players obtain it?"),
            ("Leads@", "Where can it point them next?"),
        ],
        "Tables" => &[
            ("Purpose", "What decision does the table resolve?"),
            ("Selection", "How is a row chosen?"),
            ("Results*", "Keep each range or key with its result."),
        ],
        "Handouts" => &[
            ("Audience", "Who receives it?"),
            ("Delivery", "When and how is it given out?"),
            ("Player text", "What may players actually see?"),
            ("GM notes", "What context stays behind the screen?"),
        ],
        _ => &[
            ("Role", "How does this item function in the book?"),
            ("Connections@", "Specific links to other entries."),
        ],
    };
    let fields = rows
        .iter()
        .map(|(name, guidance)| {
            let label = name.trim_end_matches(['*', '@']);
            Field {
                key: label.to_lowercase().replace(' ', "_"),
                label: label.into(),
                guidance: (*guidance).into(),
                format: if name.ends_with('@') {
                    Format::Links
                } else if name.ends_with('*') {
                    Format::List
                } else {
                    Format::Text
                },
            }
        })
        .collect();
    Template {
        revision: 0,
        fields,
    }
}

impl Template {
    pub(super) fn validate(&self) -> Result<(), String> {
        if !(1..=32).contains(&self.fields.len()) {
            return Err("Supply 1–32 template fields".into());
        }
        let mut keys = BTreeSet::new();
        for f in &self.fields {
            if !f.key.starts_with(|c: char| c.is_ascii_lowercase())
                || f.key.len() > 64
                || !f
                    .key
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
                || !keys.insert(&f.key)

            {
                return Err(format!(
                    "Invalid or duplicate field '{}'. Use a stable lowercase key, label and guidance.",
                    f.key
                ));
            }
            if f.label.trim().is_empty() || f.label.len() > 100 {
                return Err(format!("Field {} needs a label of 1–100 bytes.", f.key));
            }
            if f.guidance.trim().is_empty() || f.guidance.len() > 600 {
                return Err(format!("Field {} needs guidance of 1–600 bytes (received {}). Keep its key.", f.key, f.guidance.len()));
            }
        }
        Ok(())
    }

    pub(super) fn check(&self, values: &Values) -> Result<(), String> {
        for (key, value) in values {
            let field = self.fields.iter().find(|f| &f.key == key).ok_or_else(|| {
                format!("Unknown template field '{key}'. Read book_read_template.")
            })?;
            let text = |v: &Value| {
                v.as_str()
                    .is_some_and(|s| !s.trim().is_empty() && s.len() <= 4000)
            };
            let valid = match field.format {
                Format::Text => text(value),
                Format::Links => value.as_array().is_some_and(|items| {
                    (1..=64).contains(&items.len())
                        && items.iter().all(|item| {
                            let Some(obj) = item.as_object() else {
                                return false;
                            };
                            obj.keys()
                                .all(|key| matches!(key.as_str(), "name" | "target_id" | "context"))
                                && text(&item["name"])
                                && text(&item["context"])
                                && (item["target_id"].is_null()
                                    || item["target_id"]
                                        .as_str()
                                        .is_some_and(|id| id.parse::<Uuid>().is_ok()))
                        })
                }),
                Format::List => value
                    .as_array()
                    .is_some_and(|items| (1..=64).contains(&items.len()) && items.iter().all(text)),
            };
            if !valid {
                return Err(format!(
                    "Invalid value for '{}'. Follow the template format. A link needs name and context, plus target_id when resolved.",
                    field.label
                ));
            }
        }
        Ok(())
    }
}

pub(super) fn check_entry(book: &Book, finding: &Finding) -> Result<(), String> {
    if finding.kind != FindingKind::Entry && finding.fields.is_empty() {
        return Ok(());
    }
    if finding.fields.is_empty() {
        return Err("An entry needs at least one supported template field. Read book_read_template for this area.".into());
    }
    book.template(&finding.area).check(&finding.fields)?;
    for item in finding
        .fields
        .values()
        .filter_map(Value::as_array)
        .flatten()
    {
        if let Some(id) = item.get("target_id").and_then(Value::as_str) {
            let id = id.parse::<Uuid>().map_err(error)?;
            if !book
                .records
                .iter()
                .any(|r| r.id == id && r.finding.kind == FindingKind::Entry)
            {
                return Err(format!(
                    "Unknown target {id}. Use book_find_entries to locate a record in this book."
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn remove_record(book: &mut Book, index: usize) -> Uuid {
    let removed = book.records.remove(index);
    let id = removed.id.to_string();
    for record in &mut book.records {
        for value in record
            .finding
            .fields
            .values_mut()
            .chain(record.retained.iter_mut().map(|f| &mut f.value))
        {
            for item in value.as_array_mut().into_iter().flatten() {
                if item.get("target_id").and_then(Value::as_str) == Some(&id) {
                    item.as_object_mut().unwrap().remove("target_id");
                    item["name"] = json!(removed.finding.title);
                }
            }
        }
    }
    removed.id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_area_has_a_valid_purpose_built_template() {
        for kind in [Kind::Novel, Kind::Rulebook, Kind::ContentBook] {
            for area in kind.areas() {
                let template = builtin(area);
                template.validate().unwrap();
                assert!(template.fields.len() >= 3, "Missing template for {area}");
            }
        }
    }
}
