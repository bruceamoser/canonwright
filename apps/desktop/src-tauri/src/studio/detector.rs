use super::*;
use serde_json::Value;
use std::{
    io::{Seek, SeekFrom},
    process::Command,
    time::Duration,
};

pub(super) fn assess(root: &Path, book: &Book, args: &Value) -> Result<String, String> {
    let mut sections = Vec::new();
    match (args.get("text"), args.get("section_id")) {
        (Some(text), None) => {
            let text = text.as_str().ok_or("Supply a text string")?;
            sections.push(json!({"title":"Supplied text","text":text}));
        }
        (None, selected) => {
            let id = selected
                .map(|v| serde_json::from_value::<Uuid>(v.clone()))
                .transpose()
                .map_err(error)?;
            for section in &book.sections {
                if id.is_some_and(|id| section.id != id) {
                    continue;
                }
                let view = read_section(root, book, section.id)?;
                sections.push(json!({"section_id":section.id,"title":section.title,"revision":view.revision,"text":view.text}));
            }
            if id.is_some() && sections.is_empty() {
                return Err("Unknown section ID".into());
            }
        }
        _ => return Err("Choose either a manuscript section or supplied text".into()),
    }
    let threshold = args
        .get("threshold")
        .map_or(Some(0.8), Value::as_f64)
        .filter(|n| (0.0..=1.0).contains(n))
        .ok_or("Threshold must be between zero and one")?;
    let offset = args
        .get("offset")
        .map_or(Some(0), Value::as_u64)
        .ok_or("Invalid result offset")?;
    let base = directories::BaseDirs::new().ok_or("Missing data directory")?;
    let runtime = base.data_dir().join("canonwright/detector-v1");
    let mode = args
        .get("mode")
        .map_or(Some("quick"), Value::as_str)
        .filter(|m| {
            matches!(
                *m,
                "quick" | "follow_up" | "strict" | "fast_detectgpt" | "editlens"
            )
        })
        .ok_or("Unknown detector mode")?;
    let python = if runtime.join("venv-cu128/verified").is_file() {
        runtime.join("venv-cu128/bin/python")
    } else {
        runtime.join("venv/bin/python")
    };
    if !python.is_file()
        || !runtime.join("model/model.safetensors").is_file()
        || (mode == "follow_up" && !runtime.join("predictability/model.safetensors").is_file())
    {
        return Err("Local detector setup required. Run sh /usr/share/canonwright/detector/install.sh once (requires uv, curl and about 4 GB of disk space).".into());
    }
    let mut input = tempfile::tempfile().map_err(error)?;
    serde_json::to_writer(
        &mut input,
        &json!({"sections":sections,"threshold":threshold,"offset":offset,"mode":mode}),
    )
    .map_err(error)?;
    input.seek(SeekFrom::Start(0)).map_err(error)?;
    let output = crate::process::run_bounded(
        Command::new(python)
            .args(["-I", "-c", include_str!("../../detector/assess.py")])
            .env("HF_HUB_OFFLINE", "1")
            .stdin(input),
        Duration::from_secs(1800),
        "local manuscript detector",
    )?;
    if !output.status.success() {
        return Err(format!(
            "Local assessment failed: {}",
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .last()
                .unwrap_or("worker stopped")
        ));
    }
    String::from_utf8(output.stdout).map_err(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "uses the optional local Desklib runtime"]
    fn scores_saved_sections_and_preserves_unicode_offsets() {
        let dir = tempfile::tempdir().unwrap();
        initialize(dir.path(), Some("novel"), None, None).unwrap();
        let text = "Éva kept her mother’s key in a tin.\nThe river had reached the kitchen door. "
            .repeat(18);
        let id = add_at_with_parent(dir.path(), "Flood", &text, None).unwrap();
        let book = load(dir.path()).unwrap();
        let output = assess(dir.path(), &book, &json!({"threshold":0})).unwrap();
        let report: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(report["skipped_short_count"], 1);
        assert_eq!(report["mode"], "quick");
        assert_eq!(report["second_pass"]["scored"], 0);
        let follow = assess(
            dir.path(),
            &book,
            &json!({"threshold":0,"mode":"follow_up"}),
        )
        .unwrap();
        let follow: Value = serde_json::from_str(&follow).unwrap();
        assert_eq!(
            follow["classifier_summary"]["cached_windows"],
            report["scored_windows"]
        );
        assert_eq!(follow["second_pass"]["scored"], report["scored_windows"]);
        assert_eq!(report["second_pass"]["unavailable"], 0);
        let strict = assess(
            dir.path(),
            &book,
            &json!({"text":text,"mode":"strict","threshold":0}),
        )
        .unwrap();
        let strict: Value = serde_json::from_str(&strict).unwrap();
        assert_eq!(strict["mode"], "strict");
        assert_eq!(strict["second_pass"]["enabled"], false);
        assert_eq!(
            strict["corroborated_windows"],
            strict["binoculars"]["promoted"]
        );
        assert!(
            strict["results"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["review_tier"] != "both_flagged")
        );
        assert!(report["scored_windows"].as_u64().unwrap() >= 2);
        for row in report["results"].as_array().unwrap() {
            assert_eq!(row["section_id"], json!(id));
            let at = row["start_char"].as_u64().unwrap() as usize;
            let quote = row["excerpt"].as_str().unwrap();
            assert_eq!(
                text.chars()
                    .skip(at)
                    .take(quote.chars().count())
                    .collect::<String>(),
                quote
            );
        }
    }
}
