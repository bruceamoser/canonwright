use super::*;
use serde_json::Value;
use std::process::Command;
use std::time::Duration;

fn token(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 90
        && s.starts_with(|c: char| c.is_ascii_alphanumeric())
        && !s.contains("..")
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
}

fn run(root: &Path, bin: &str, args: &[&str]) -> Result<String, String> {
    let out = crate::process::run_bounded(
        Command::new(bin)
            .current_dir(root)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0"),
        Duration::from_secs(120),
        bin,
    )?;
    if !out.status.success() {
        return Err(format!(
            "{bin}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().into())
}

pub(super) fn prepare(root: &Path, args: &Value) -> Result<Value, String> {
    let slug = args["edition"]
        .as_str()
        .filter(|s| token(s))
        .ok_or("Choose a simple edition name")?;
    let title = args["title"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or("Supply an edition title")?;
    let mut book = load(root)?;
    if let Some(ids) = args.get("section_ids") {
        let ids: Vec<Uuid> = serde_json::from_value(ids.clone()).map_err(error)?;
        if ids.is_empty()
            || ids
                .iter()
                .any(|id| !book.sections.iter().any(|s| s.id == *id))
        {
            return Err("Choose existing section IDs".into());
        }
        book.sections.retain(|s| ids.contains(&s.id));
    }
    let md = compile(root, &book)?;
    let dir = local(root, Path::new("editions"))?;
    fs::create_dir_all(&dir).map_err(error)?;
    let dest = local(root, &Path::new("editions").join(slug))?;
    if dest.exists() {
        return Err("Edition already exists. Choose a new revision name.".into());
    }
    let tmp = tempfile::tempdir_in(&dir).map_err(error)?;
    fs::write(tmp.path().join("manuscript.md"), md).map_err(error)?;
    fs::write(tmp.path().join("layout.typ"), include_str!("edition.typ")).map_err(error)?;
    let author = args["author"].as_str().unwrap_or("");
    // Markdown raw blocks and metadata cannot inject executable Typst into this build.
    run(
        tmp.path(),
        "pandoc",
        &[
            "--sandbox",
            "--from=markdown-raw_attribute-raw_tex-raw_html-yaml_metadata_block",
            "--pdf-engine=typst",
            "--template=layout.typ",
            "--metadata",
            &format!("title={title}"),
            "--metadata",
            &format!("author={author}"),
            "manuscript.md",
            "-o",
            "book.pdf",
        ],
    )?;
    fs::remove_file(tmp.path().join("layout.typ")).map_err(error)?;
    fs::rename(tmp.path(), dest).map_err(error)?;
    Ok(
        json!({"markdown":format!("editions/{slug}/manuscript.md"),"pdf":format!("editions/{slug}/book.pdf"),"sections":book.sections.len()}),
    )
}

// An interactive publishing action acts only on the book's own origin and a
// named edition. Never stage the private queue, chat citations or audit files.
pub(super) fn publish(root: &Path, args: &Value) -> Result<Value, String> {
    let op = args["operation"].as_str().ok_or("Missing operation")?;
    crate::git::ensure_book_repo(root)?;
    if op == "status" {
        return Ok(
            json!({"git":run(root,"git",&["status","--short","--branch"])?,"origin":run(root,"git",&["remote","get-url","origin"]).ok()}),
        );
    }
    let slug = args["edition"]
        .as_str()
        .filter(|s| token(s))
        .ok_or("Supply an edition name")?;
    let pdf = format!("editions/{slug}/book.pdf");
    let md = format!("editions/{slug}/manuscript.md");
    for f in [&pdf, &md] {
        if !local(root, Path::new(f))?.is_file() {
            return Err(format!("Missing {f}"));
        }
    }
    let required = |k: &str| {
        args[k]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| format!("Supply {k}"))
    };
    if op == "commit" {
        return run_commit(root, required("message")?, &md, &pdf);
    }
    let repo = required("repository")?;
    let (owner, name) = repo
        .split_once('/')
        .filter(|(o, n)| token(o) && token(n))
        .ok_or("Supply GitHub owner/repository")?;
    let gh = directories::BaseDirs::new()
        .ok_or("Missing home directory")?
        .home_dir()
        .join(".local/bin/gh-project");
    let gh = gh.to_str().ok_or("Invalid account wrapper path")?;
    let account = required("account")?;
    if !matches!(account, "personal" | "work") {
        return Err("Choose personal or work".into());
    }
    let host = format!("https://github.com/{owner}/{name}");
    let gh_run = |cmd: &[&str]| {
        let mut a = vec!["--account", account];
        a.extend_from_slice(cmd);
        run(root, gh, &a)
    };
    if op == "create_private_repo" {
        if run(root, "git", &["remote"])?
            .lines()
            .any(|r| r == "origin")
        {
            return Err("Origin already exists; inspect status first".into());
        }
        gh_run(&[
            "repo",
            "create",
            repo,
            "--private",
            "--source=.",
            "--remote=origin",
        ])?;
    } else {
        let origin = run(root, "git", &["remote", "get-url", "origin"])?;
        if ![host.as_str(), &format!("git@github.com:{repo}")]
            .contains(&origin.trim_end_matches(".git"))
        {
            return Err("Requested repository differs from this book's origin".into());
        }
        match op {
            "push" => {
                let destinations = run(root, "git", &["remote", "get-url", "--push", "--all", "origin"])?;
                if destinations.trim_end_matches(".git") != host {
                    return Err("Set the book's push URL to its account-scoped GitHub HTTPS destination".into());
                }
                let branch = run(root, "git", &["symbolic-ref", "--short", "HEAD"])?;
                gh_run(&[
                    "--run",
                    "git",
                    "-c",
                    "credential.helper=",
                    "-c",
                    &format!(
                        "credential.helper=!{} --git-credential",
                        if account == "personal" {
                            "gh-personal"
                        } else {
                            "gh-work"
                        }
                    ),
                    "push",
                    "--set-upstream",
                    "origin",
                    &branch,
                ])?;
            }
            "release" => {
                let tag = required("tag")?;
                if !token(tag) {
                    return Err("Invalid release tag".into());
                }
                let head = run(root, "git", &["rev-parse", "HEAD"])?;
                let expected = run(root, "git", &["rev-parse", &format!("HEAD:{pdf}")])?;
                if expected != run(root, "git", &["hash-object", &pdf])? {
                    return Err("Commit the current PDF before releasing".into());
                }
                gh_run(&[
                    "release",
                    "create",
                    tag,
                    &pdf,
                    "--repo",
                    repo,
                    "--target",
                    &head,
                    "--title",
                    required("title")?,
                    "--notes",
                    required("notes")?,
                ])?;
            }
            _ => return Err("Unknown publishing action".into()),
        }
    }
    Ok(json!({"operation":op,"repository":repo}))
}

fn run_commit(root: &Path, message: &str, md: &str, pdf: &str) -> Result<Value, String> {
    run(root, "git", &["add", "--", md, pdf])?;
    run(
        root,
        "git",
        &["commit", "--only", "-m", message, "--", md, pdf],
    )?;
    Ok(json!({"commit":run(root,"git",&["rev-parse","HEAD"])?}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edition_commit_preserves_unrelated_staged_work() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        crate::git::ensure_book_repo(root).unwrap();
        run(root, "git", &["config", "user.name", "Fixture"]).unwrap();
        run(root, "git", &["config", "user.email", "fixture@example.invalid"]).unwrap();
        let edition = root.join("editions/sample");
        fs::create_dir_all(&edition).unwrap();
        fs::write(edition.join("manuscript.md"), "# Sample").unwrap();
        fs::write(edition.join("book.pdf"), b"%PDF-fixture").unwrap();
        fs::write(root.join("private-notes"), "Private fixture").unwrap();
        run(root, "git", &["add", "private-notes"]).unwrap();
        publish(root, &json!({"operation":"commit","edition":"sample","message":"Sample edition"})).unwrap();
        assert_eq!(run(root,"git",&["ls-tree","-r","--name-only","HEAD"]).unwrap(),
                   "editions/sample/book.pdf\neditions/sample/manuscript.md");
        assert_eq!(run(root,"git",&["diff","--cached","--name-only"]).unwrap(),"private-notes");
    }
}
