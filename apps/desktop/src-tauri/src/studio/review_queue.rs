//! The first durable inbox uses the book transaction so the canon write and its
//! completion receipt are atomic. A task ID is the original proposal ID.
use super::*;
use canonwright_tools::{ToolContext, ToolError};
use std::collections::HashSet;
use std::sync::LazyLock;

#[derive(Clone, Default, Serialize, Deserialize)]
pub(super) struct Job {
    pub id: Uuid,
    #[serde(default)]
    pub work: Option<Work>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub process_drafts: Vec<process::Draft>,
    #[serde(default)]
    pub process_ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_target: Option<String>,
    #[serde(default)]
    pub process_repairs: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync: Option<chapter_sync::Change>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editorial: Option<editorial::Assignment>,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimed_role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_decision: Option<review::Decision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author_decision: Option<String>,
    #[serde(default)]
    pub parent_job: Option<Uuid>,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub worker_id: Option<String>,
    #[serde(default)]
    pub report: Option<String>,
    #[serde(default)]
    pub receipts: Vec<serde_json::Value>,
    #[serde(default)]
    pub write_in_progress: Option<String>,
    pub state: String,
    #[serde(default)]
    pub instructions: Option<String>,
    pub result_id: Option<Uuid>,
    pub error: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Work {
    pub role: String,
    pub request: String,
    pub focus: agent::Focus,
}

pub(super) fn submit(
    root: &Path,
    role: &str,
    request: &str,
    focus: &agent::Focus,
) -> Result<Uuid, String> {
    if !matches!(
        role,
        "writer" | "canon_keeper" | "editorial_reviewer" | "process_manager"
    ) || request.trim().is_empty()
        || request.len() > 24000
    {
        return Err("Choose a supported role and supply a request of up to 24 KB.".into());
    }
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(root)?;
    if let Some(existing) = book.review_jobs.iter().find(|job| {
        (matches!(job.state.as_str(), "ready" | "running")
            || (focus.job_id.is_some() && job.parent_job == focus.job_id && job.state == "done"))
            && job.parent_job == focus.job_id
            && job.work.as_ref().is_some_and(|w| {
                w.role == role && w.request == request.trim() && w.focus.same_selection(focus)
            })
    }) {
        return Ok(existing.id);
    }
    let id = Uuid::now_v7();
    let parent_job = focus.job_id;
    let mut focus = focus.clone();
    focus.job_id = Some(id);
    book.review_jobs.push(Job {
        id,
        parent_job,
        state: "ready".into(),
        updated_at: crate::now_millis(),
        work: Some(Work {
            role: role.into(),
            request: request.trim().into(),
            focus,
        }),
        ..Job::default()
    });
    save(root, &book)?;
    crate::diagnostics::event(
        "queue.submitted",
        json!({"book":root,"job":id,"role":role,"parent":parent_job}),
    );
    Ok(id)
}

pub(super) fn update_job(
    root: &Path,
    id: Uuid,
    change: impl FnOnce(&mut Job) -> Result<(), String>,
) -> Result<(), String> {
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(root)?;
    let job = book
        .review_jobs
        .iter_mut()
        .find(|j| j.id == id)
        .ok_or("Unknown work ID")?;
    change(job)?;
    job.updated_at = crate::now_millis();
    save(root, &book)
}

pub(super) fn control(root: &Path, id: Uuid, operation: &str) -> Result<(), String> {
    update_job(root, id, |job| {
        match (operation, job.state.as_str()) {
            ("cancel", "ready") => job.state = "cancelled".into(),
            ("cancel", "running") => job.state = "cancelling".into(),
            ("retry", "failed" | "cancelled") => {
                job.process_repairs = 0;
                let target = failed_example(job).map(|draft| draft.target().to_owned());
                job.state = "ready".into();
                job.repair_target = target;
                if job.process_ready {
                    job.process_ready = false;
                    let prior = std::mem::take(&mut job.receipts);
                    job.receipts
                        .push(json!({"previous_test_failure":job.error,"prior_attempt":prior}));
                    if let Some(target) = &job.repair_target {
                        job.process_repairs = 1;
                        job.receipts.push(json!({"skill_test_failure":job.error,"target":target,"attempt":1}));
                    } else {
                        job.process_drafts.clear();
                    }
                }
                job.error = None;
                job.worker_id = None;
                if let Some(tool) = job.write_in_progress.take() {
                    job.receipts
                        .push(json!({"check_interrupted_operation":tool}));
                }
            }
            _ => return Err(format!("Cannot {operation} a {} job", job.state)),
        }
        Ok(())
    })
}

pub(crate) fn record_startup_error(root: &Path, reason: &str) {
    if let Ok(_guard) = BOOK_IO.lock()
        && let Ok(mut book) = load(root)
    {
        let mut changed = false;
        for job in &mut book.review_jobs {
            if matches!(job.state.as_str(), "ready" | "running")
                && job.error.as_deref() != Some(reason)
            {
                changed = true;
                job.error = Some(reason.into());
            }
        }
        if changed {
            let _ = save(root, &book);
        }
    }
}

pub(crate) fn dispatching(root: &Path) -> bool {
    ACTIVE.lock().is_ok_and(|active| active.contains(root))
}

fn failed_example(job: &Job) -> Option<&process::Draft> {
    if job.state != "failed"
        || !job.process_ready
        || job.process_repairs >= 2
        || job.write_in_progress.is_some()
        || job.work.as_ref()?.role != "process_manager"
    {
        return None;
    }
    let error = job.error.as_deref()?;
    job.process_drafts.iter().find(|d| d.matches_failure(error))
}

fn stale_failure(job: &Job) -> bool {
    job.state == "failed"
        && job.work.is_none()
        && job.editorial.is_none()
        && job.sync.is_none()
        && job
            .error
            .as_deref()
            .is_some_and(|e| e.contains("Inputs changed after the verdict."))
}

fn orphan_ids(book: &Book) -> Vec<Uuid> {
    let known: HashSet<_> = book.review_jobs.iter().map(|j| j.id).collect();
    book.records
        .iter()
        .filter(|r| r.pending && !known.contains(&r.id) && reviewable(r))
        .map(|r| r.id)
        .collect()
}

fn reviewable(r: &Record) -> bool {
    !r.editorial.as_ref().is_some_and(|v| {
        matches!(
            v.decision,
            review::Decision::AskAuthor
                | review::Decision::ManuscriptIssue
                | review::Decision::SourceConflict
                | review::Decision::ReferralClosed
                | review::Decision::IssueResolved
        )
    })
}

pub(crate) fn needs_worker(root: &Path) -> bool {
    manifest(root).is_ok_and(|p| p.is_file())
        && load(root).is_ok_and(|book| {
            book.review_jobs.iter().any(|j| {
                failed_example(j).is_some()
                    || stale_failure(j)
                    || matches!(j.state.as_str(), "ready" | "running" | "cancelling")
            }) || !orphan_ids(&book).is_empty()
        })
}

async fn run_work(
    root: &Path,
    context: &ToolContext,
    job: &Job,
    work: &Work,
) -> Result<(), ToolError> {
    use canonwright_tools::SubagentSpec;
    let runtime = context
        .subagents
        .as_ref()
        .ok_or_else(|| ToolError::Failed("Worker runtime unavailable".into()))?;
    if job.state == "cancelling" {
        if let Some(id) = &job.worker_id {
            let _ = runtime.cancel(id);
        }
        return update_job(root, job.id, |j| {
            j.state = "cancelled".into();
            Ok(())
        })
        .map_err(ToolError::Failed);
    }
    if matches!(job.editorial, Some(editorial::Assignment::Repair { .. }))
        && job.result_id.is_some()
    {
        // Submission and verification enqueue share a manifest transaction, even
        // if the Writer process exited before emitting its final report.
        return update_job(root, job.id, |j| {
            j.state = "done".into();
            j.report = Some("Submitted for verification.".into());
            j.write_in_progress = None;
            Ok(())
        })
        .map_err(ToolError::Failed);
    }
    if job.write_in_progress.is_some() {
        return Err(ToolError::Failed(format!(
            "Interrupted during {:?}. Inspect the book before explicitly retrying.",
            job.write_in_progress
        )));
    }
    let task = format!(
        "CanonWright queued work\n{}",
        json!({"work":work,"prior_receipts":job.receipts,"repair":job.repair_target.as_ref().and_then(|id|job.process_drafts.iter().find(|d|d.target()==id))})
    );
    let worker = runtime
        .spawn(SubagentSpec {
            task,
            title: format!(
                "{} · {}",
                work.role,
                work.request.chars().take(64).collect::<String>()
            ),
            model: None,
            allow_writes: true,
            allow_commands: false,
        })
        .map_err(ToolError::Failed)?;
    crate::diagnostics::event(
        "queue.worker_started",
        json!({"job":job.id,"worker":worker.id,"role":work.role}),
    );
    update_job(root, job.id, |j| {
        j.worker_id = Some(worker.id.clone());
        Ok(())
    })
    .map_err(ToolError::Failed)?;
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        let current = load(root).map_err(ToolError::Failed)?;
        if current
            .review_jobs
            .iter()
            .any(|j| j.id == job.id && j.state == "cancelling")
        {
            let _ = runtime.cancel(&worker.id);
        }
        let state = runtime
            .get(&worker.id)
            .map_err(ToolError::Failed)?
            .ok_or_else(|| ToolError::Failed("Worker disappeared".into()))?;
        if matches!(state.status.as_str(), "completed" | "cancelled" | "failed") {
            return update_job(root, job.id, |j| {
                j.state = if j.state == "cancelling" || state.status == "cancelled" {
                    "cancelled"
                } else if state.status == "completed" {
                    "done"
                } else {
                    "failed"
                }
                .into();
                j.error = state.error;
                if j.state == "done"
                    && (matches!(work.role.as_str(), "writer" | "process_manager")
                        || (work.role == "canon_keeper"
                            && (!work.focus.author_sources.is_empty()
                                || j.receipts.iter().any(|r| r["ok"] == false))))
                    && !j.receipts.iter().any(|v| v["ok"] == true)
                {
                    j.state = "failed".into();
                    j.error = Some(
                        "Worker finished without a successful book change. See its report.".into(),
                    );
                }
                if j.state == "done" && work.role == "writer"
                    && let Some(last) = j.receipts.last().filter(|r| r["ok"] == false)
                {
                    j.state = "failed".into();
                    j.error = Some(format!("Final Writer operation failed: {}. Check earlier receipts for saved work.",
                        last["error"].as_str().unwrap_or("see tool receipt")));
                }
                if j.state == "done"
                    && matches!(j.editorial, Some(editorial::Assignment::Repair { .. }))
                    && j.result_id.is_none()
                {
                    j.state = "failed".into();
                    j.error =
                        Some("Writer did not submit the revised passage for verification.".into());
                }
                if j.state == "done"
                    && work.role == "process_manager"
                    && !j.process_drafts.is_empty()
                {
                    j.process_ready = true;
                    j.state = "ready".into();
                }
                if j.repair_target.is_some()
                    && j.process_ready
                    && !j
                        .receipts
                        .iter()
                        .any(|r| r["corrected_attempt"] == json!(j.process_repairs))
                {
                    j.state = "failed".into();
                    j.process_ready = false;
                    j.error =
                        Some("Process Manager finished without correcting the failed skill".into());
                }
                if j.process_ready {
                    j.repair_target = None;
                }
                j.report = Some(state.result);
                Ok(())
            })
            .map_err(ToolError::Failed);
        }
    }
}

static ACTIVE: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(Default::default);

pub(super) fn record_result(book: &mut Book, id: Uuid, result: Uuid, decision: review::Decision, title: &str) {
    if let Some(job) = book.review_jobs.iter_mut().find(|job| job.id == id) {
        job.review_decision = Some(decision);
        job.reviewed_title = Some(title.to_owned());
        if decision == review::Decision::Revise {
            job.instructions = book
                .records
                .iter()
                .find(|r| r.id == id)
                .and_then(|r| r.editorial.as_ref())
                .map(|v| v.reason.clone());
        }
        job.updated_at = crate::now_millis();
        job.result_id = Some(result);
        job.state = match decision {
            review::Decision::Revise => "ready",
            review::Decision::AskAuthor => "waiting_for_author",
            _ => "done",
        }
        .into();
        job.error = None;
    }
}

pub(super) fn retry_failed_reviews(root: &Path, ids: &[Uuid]) -> Result<usize, String> {
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(root)?;
    let selected: HashSet<_> = ids.iter().copied().collect();
    let mut count = 0;
    for j in &mut book.review_jobs {
        if selected.contains(&j.id)
            && j.state == "failed"
            && j.sync.is_none()
            && j.editorial.is_none()
            && j.work.is_none()
        {
            j.state = "ready".into();
            j.updated_at = crate::now_millis();
            j.receipts.push(json!({"retry_reason":j.error.take()}));
            count += 1;
        }
    }
    if count > 0 {
        save(root, &book)?;
    }
    Ok(count)
}

pub(super) fn enqueue(root: &Path, id: Option<Uuid>, scope: &str) -> Result<Vec<Uuid>, String> {
    if !matches!(scope, "unchecked" | "decided" | "all") {
        return Err("Unknown queue scope".into());
    }
    let _guard = BOOK_IO.lock().map_err(error)?;
    let mut book = load(root)?;
    let ids = queue_records(&mut book, id, scope);
    save(root, &book)?;
    Ok(ids)
}

pub(super) fn queue_records(book: &mut Book, id: Option<Uuid>, scope: &str) -> Vec<Uuid> {
    let ids: Vec<_> = book
        .records
        .iter()
        .filter(|r| r.pending && id.is_none_or(|id| r.id == id))
        .filter(|r| match r.editorial.as_ref().map(|v| v.decision) {
            None => scope != "decided",
            Some(
                review::Decision::AskAuthor
                | review::Decision::ManuscriptIssue
                | review::Decision::SourceConflict
                | review::Decision::ReferralClosed
                | review::Decision::IssueResolved,
            ) => false,
            _ => scope != "unchecked",
        })
        .map(|r| r.id)
        .collect();
    for id in &ids {
        let instructions = book
            .records
            .iter()
            .find(|r| r.id == *id)
            .and_then(|r| r.editorial.as_ref())
            .map(|v| v.reason.clone());
        match book.review_jobs.iter_mut().find(|j| j.id == *id) {
            Some(j) if j.state == "failed" => {
                j.state = "ready".into();
                j.error = None;
            }
            None => book.review_jobs.push(Job {
                id: *id,
                instructions,
                state: "ready".into(),
                result_id: None,
                error: None,
                ..Job::default()
            }),
            _ => {}
        }
    }
    ids
}

pub(crate) fn wake(root: &Path, context: &ToolContext) -> Result<(), ToolError> {
    if context.subagents.is_none() {
        return Ok(());
    }
    let root = root.to_owned();
    let mut active = ACTIVE.lock().map_err(|e| ToolError::Failed(error(e)))?;
    if !active.insert(root.clone()) {
        return Ok(());
    }
    if !needs_worker(&root) {
        active.remove(&root);
        return Ok(());
    }
    if let Err(cause) = crate::git::ensure_book_repo(&root) {
        active.remove(&root);
        return Err(ToolError::Failed(cause));
    }
    // The OS releases the dispatch lease on process exit, including a crash.
    let lease =
        match local(&root, Path::new(".canonwright/studio/dispatcher.lock")).and_then(|path| {
            std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(path)
                .map_err(error)
        }) {
            Ok(file) => file,
            Err(e) => {
                active.remove(&root);
                return Err(ToolError::Failed(error(e)));
            }
        };
    if let Err(e) = lease.try_lock() {
        active.remove(&root);
        return match e {
            std::fs::TryLockError::WouldBlock => Ok(()),
            std::fs::TryLockError::Error(e) => Err(ToolError::Failed(error(e))),
        };
    }
    let context = context.clone();
    tauri::async_runtime::spawn(async move {
        let mut workers = tokio::task::JoinSet::new();
        let mut busy = std::collections::HashMap::<Uuid, String>::new();
        loop {
            {
                let mut active = ACTIVE.lock().unwrap();
                let claimed = (|| -> Result<Vec<(String, Job)>, String> {
                    let _guard = BOOK_IO.lock().map_err(error)?;
                    let mut book = load(&root)?;
                    for id in orphan_ids(&book) {
                        queue_records(&mut book, Some(id), "all");
                    }
                    let mut occupied: HashSet<String> = busy.values().cloned().collect();
                    let mut claims = vec![];
                    for index in 0..book.review_jobs.len() {
                        let job = &book.review_jobs[index];
                        if busy.contains_key(&job.id)
                            || (failed_example(job).is_none()
                                && !stale_failure(job)
                                && !matches!(
                                    job.state.as_str(),
                                    "ready" | "running" | "cancelling"
                                ))
                        {
                            continue;
                        }
                        let role = job_role(&book, job).to_owned();
                        if !occupied.insert(role.clone()) {
                            continue;
                        }
                        let job = &mut book.review_jobs[index];
                        if let Some(draft) = failed_example(job) {
                            job.repair_target = Some(draft.target().to_owned());
                            job.process_repairs += 1;
                            job.receipts.push(json!({"skill_test_failure":job.error,"target":job.repair_target,"attempt":job.process_repairs}));
                            job.process_ready = false;
                            job.worker_id = None;
                        }
                        if stale_failure(job) {
                            job.receipts.push(json!({"recovered_failure":job.error}));
                        }
                        if job.state != "cancelling" {
                            job.state = "running".into();
                        }
                        job.claimed_role = Some(role.clone());
                        job.attempts += 1;
                        job.updated_at = crate::now_millis();
                        job.error = None;
                        claims.push((role, job.clone()));
                    }
                    if !claims.is_empty() {
                        save(&root, &book)?;
                    }
                    Ok(claims)
                })();
                for (role, job) in claimed.unwrap_or_else(|e| {
                    crate::diagnostics::record(
                        "ERROR",
                        &format!("Queue claim failed in {}: {e}", root.display()),
                    );
                    vec![]
                }) {
                    let id = job.id;
                    crate::diagnostics::event(
                        "queue.claimed",
                        json!({"book":root,"job":id,"role":role,"attempt":job.attempts,"parent":job.parent_job,"repair_target":job.repair_target}),
                    );
                    busy.insert(id, role);
                    let root = root.clone();
                    let context = context.clone();
                    workers.spawn(async move {
                        execute_job(&root, &context, job).await;
                        id
                    });
                }
                if busy.is_empty() {
                    drop(lease);
                    active.remove(&root);
                    return;
                }
            }
            // Poll for new work in idle roles while another role is awaiting inference.
            tokio::select! {
                Some(result)=workers.join_next()=>{match result{Ok(id)=>{busy.remove(&id);},Err(e)=>{eprintln!("Book worker stopped: {e}");break;}}},
                _=tokio::time::sleep(std::time::Duration::from_millis(300))=>{}
            }
        }
        workers.shutdown().await;
        let mut active = ACTIVE.lock().unwrap();
        drop(lease);
        active.remove(&root);
    });
    Ok(())
}

pub(super) fn job_role<'a>(book: &Book, job: &'a Job) -> &'a str {
    if let Some(task) = &job.editorial {
        return task.role();
    }
    if let Some(sync) = &job.sync {
        return sync.role();
    }
    if let Some(work) = &job.work {
        return &work.role;
    }
    if job.instructions.is_some()
        || book
            .records
            .iter()
            .find(|r| r.id == job.id)
            .is_some_and(|r| r.editorial.is_some())
    {
        "canon_keeper"
    } else {
        "canon_reviewer"
    }
}

async fn execute_job(root: &Path, context: &ToolContext, job: Job) {
    let id = job.id;
    let outcome = match &job.work {
        Some(work) if !job.process_ready => run_work(&root, &context, &job, work).await,
        _ => {
            let run = async {
                if job.process_ready {
                    process::publish(&root, &context, &job)
                        .await
                        .map_err(ToolError::Failed)
                } else if let Some(task) = &job.editorial {
                    editorial::run(&root, &context, id, task)
                        .await
                        .map_err(ToolError::Failed)
                } else if job.sync.is_some() {
                    chapter_sync::run(&root, &context, id)
                        .await
                        .map_err(ToolError::Failed)
                } else {
                    review::run(&root, &context, Some(id)).await.map(|_| ())
                }
            };
            tokio::pin!(run);
            loop {
                if job.sync.is_some()
                    && load(&root).ok().is_some_and(|b| {
                        b.review_jobs
                            .iter()
                            .any(|j| j.id == id && j.state == "superseded")
                    })
                {
                    break Ok(());
                }
                if load(&root).ok().is_some_and(|b| {
                    b.review_jobs
                        .iter()
                        .any(|j| j.id == id && j.state == "cancelling")
                }) {
                    break update_job(&root, id, |j| {
                        j.state = "cancelled".into();
                        Ok(())
                    })
                    .map_err(ToolError::Failed);
                }
                tokio::select! {
                    result=&mut run => break result.map(|_|()),
                    _=tokio::time::sleep(std::time::Duration::from_millis(500))=>{}
                }
            }
        }
    };
    if let Err(e) = outcome {
        let result = (|| -> Result<(), String> {
            let _guard = BOOK_IO.lock().map_err(error)?;
            let mut book = load(&root)?;
            if let Some(job) = book
                .review_jobs
                .iter_mut()
                .find(|j| j.id == id && j.state == "running")
            {
                let reason = e.to_string().to_lowercase();
                let changed_during_review = matches!(e, ToolError::Stale(_))
                    && job.work.is_none()
                    && job.sync.is_none()
                    && job.editorial.is_none();
                let transient = [
                    "429",
                    "503",
                    "502",
                    "connection reset",
                    "connection refused",
                ]
                .iter()
                .any(|msg| reason.contains(msg));
                job.state = if changed_during_review && job.attempts < 4 {
                    "ready"
                } else if transient
                    && job.attempts < 3
                    && job.receipts.is_empty()
                    && job.write_in_progress.is_none()
                {
                    "ready"
                } else {
                    "failed"
                }
                .into();
                job.error = Some(e.to_string());
            }
            save(&root, &book)
        })();
        if let Err(e) = result {
            eprintln!("Failed to persist job error: {e}");
        }
    }
    if let Ok(book) = load(root)
        && let Some(j) = book.review_jobs.iter().find(|j| j.id == id)
    {
        crate::diagnostics::event(
            "queue.settled",
            json!({"book":root,"job":id,"state":j.state,"error":j.error,"result":j.result_id,"receipts":j.receipts.len(),"elapsed_ms":crate::now_millis()-job.updated_at}),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canonwright_tools::{SubagentInfo, SubagentRuntime, SubagentSpec};
    use canonwright_workspace::WorkspaceService;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[test]
    fn queued_requests_keep_scope_and_support_cancel_and_retry() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        initialize(root, Some("novel"), None, None).unwrap();
        let focus = agent::Focus::book("book".into());
        let id = submit(root, "writer", "Build manuscript", &focus).unwrap();
        assert_eq!(
            submit(root, "writer", "Build manuscript", &focus).unwrap(),
            id
        );
        assert!(needs_worker(root));
        control(root, id, "cancel").unwrap();
        assert!(!needs_worker(root));
        control(root, id, "retry").unwrap();
        let book = load(root).unwrap();
        let job = &book.review_jobs[0];
        assert_eq!(job.work.as_ref().unwrap().focus.job_id, Some(id));
        assert_eq!(job.state, "ready");
        assert!(submit(root, "chat", "edit", &focus).is_err());
    }

    #[tokio::test]
    async fn startup_drains_unclaimed_findings_and_legacy_stale_jobs() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        initialize(root, None, None, None).unwrap();
        let section = add_at(root, "Chapter", "A lake.").unwrap();
        let mut book = load(root).unwrap();
        let finding:Finding=serde_json::from_value(json!({"kind":"note","area":"Locations","title":"Lake","detail":"A lake.","evidence":"A lake."})).unwrap();
        let target = insert_finding(&mut book, Some(section), finding.clone(), false);
        let stale = insert_finding(&mut book, Some(section), finding.clone(), true);
        let orphan = insert_finding(&mut book, Some(section), finding, true);
        book.records[1].editorial=Some(serde_json::from_value(json!({"decision":"duplicate","reason":"Already present","target_id":target,"input_stamp":"outdated"})).unwrap());
        book.review_jobs.push(Job {
            id: stale,
            state: "failed".into(),
            error: Some("stale state: Inputs changed after the verdict.".into()),
            ..Job::default()
        });
        save(root, &book).unwrap();
        assert!(needs_worker(root));
        let runtime = Arc::new(Keeper {
            calls: AtomicUsize::new(0),
            target,
        });
        let mut cx = ToolContext::new(Arc::new(WorkspaceService::new(root)));
        cx.subagents = Some(runtime.clone());
        wake(root, &cx).unwrap();
        for _ in 0..100 {
            if !needs_worker(root) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        let book = load(root).unwrap();
        for id in [stale, orphan] {
            let j = book.review_jobs.iter().find(|j| j.id == id).unwrap();
            assert_eq!(j.state, "done");
            assert_eq!(j.result_id, Some(target));
        }
        assert!(
            book.review_jobs[0]
                .receipts
                .iter()
                .any(|r| r.get("superseded_verdict").is_some())
        );
        assert_eq!(runtime.calls.load(Ordering::SeqCst), 0);
    }

    struct Keeper {
        calls: AtomicUsize,
        target: Uuid,
    }
    impl Keeper {
        fn response(&self) -> SubagentInfo {
            SubagentInfo { id:"keeper".into(),title:"Keeper".into(), task:String::new(), model:"fixture".into(),
                status:"completed".into(), log:String::new(),result:json!({"decision":"merge","target_id":self.target,
                "merged_detail":"The lake is unnamed.","reason":"Applied the recorded wording correction."}).to_string(),error:None,
                steps:1,created_at:0,updated_at:0,finished_at:Some(0) }
        }
    }
    impl SubagentRuntime for Keeper {
        fn spawn(&self, spec: SubagentSpec) -> Result<SubagentInfo, String> {
            assert!(spec.task.starts_with(review::KEEPER_PROMPT));
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.response())
        }
        fn get(&self, _: &str) -> Result<Option<SubagentInfo>, String> {
            Ok(Some(self.response()))
        }
        fn list(&self, _: usize) -> Result<Vec<SubagentInfo>, String> {
            Ok(vec![])
        }
        fn cancel(&self, _: &str) -> Result<SubagentInfo, String> {
            Ok(self.response())
        }
    }

    #[derive(Default)]
    struct GatedRoles {
        started: Mutex<Vec<String>>,
        release: std::sync::atomic::AtomicBool,
    }
    impl GatedRoles {
        fn info(&self, id: &str) -> SubagentInfo {
            SubagentInfo {
                id: id.into(),
                title: id.into(),
                task: String::new(),
                model: "fixture".into(),
                status: if self.release.load(Ordering::SeqCst) {
                    "completed"
                } else {
                    "running"
                }
                .into(),
                log: String::new(),
                result: "Done".into(),
                error: None,
                steps: 1,
                created_at: 0,
                updated_at: 0,
                finished_at: None,
            }
        }
    }
    impl SubagentRuntime for GatedRoles {
        fn spawn(&self, spec: SubagentSpec) -> Result<SubagentInfo, String> {
            self.started.lock().unwrap().push(spec.title.clone());
            Ok(self.info(&spec.title))
        }
        fn get(&self, id: &str) -> Result<Option<SubagentInfo>, String> {
            Ok(Some(self.info(id)))
        }
        fn list(&self, _: usize) -> Result<Vec<SubagentInfo>, String> {
            Ok(vec![])
        }
        fn cancel(&self, id: &str) -> Result<SubagentInfo, String> {
            Ok(self.info(id))
        }
    }
    #[tokio::test]
    async fn different_roles_overlap_and_each_role_runs_one_job_at_a_time() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        initialize(root, None, None, None).unwrap();
        let focus = agent::Focus::book("book".into());
        for (role, request) in [
            ("canon_keeper", "first"),
            ("canon_keeper", "second"),
            ("editorial_reviewer", "editorial"),
        ] {
            submit(root, role, request, &focus).unwrap();
        }
        let runtime = Arc::new(GatedRoles::default());
        let mut context = ToolContext::new(Arc::new(WorkspaceService::new(root)));
        context.subagents = Some(runtime.clone());
        wake(root, &context).unwrap();
        for _ in 0..100 {
            if runtime.started.lock().unwrap().len() == 2 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        let book = load(root).unwrap();
        assert_eq!(book.review_jobs[0].state, "running");
        assert_eq!(book.review_jobs[1].state, "ready");
        assert_eq!(book.review_jobs[2].state, "running");
        runtime.release.store(true, Ordering::SeqCst);
        for _ in 0..100 {
            if load(root)
                .unwrap()
                .review_jobs
                .iter()
                .all(|j| j.state == "done")
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert!(
            load(root)
                .unwrap()
                .review_jobs
                .iter()
                .all(|j| j.state == "done")
        );
        assert_eq!(runtime.started.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn resumed_queue_routes_old_feedback_once_and_replays_saved_merge_without_a_model() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        initialize(root, Some("novel"), None, None).unwrap();
        let sec = add_at(root, "Lake", "A lake.").unwrap();
        let mut book = load(root).unwrap();
        let finding: Finding=serde_json::from_value(json!({"kind":"entry","area":"Locations","title":"Lake","detail":"Named lake.","evidence":"A lake."})).unwrap();
        let target = insert_finding(&mut book, Some(sec), finding.clone(), false);
        let a = insert_finding(&mut book, Some(sec), finding.clone(), true);
        let b = insert_finding(&mut book, Some(sec), finding, true);
        book.records[1].editorial=Some(serde_json::from_value(json!({"decision":"revise","reason":"Correct the accepted wording to say the lake is unnamed."})).unwrap());
        book.records[2].editorial = Some(
            serde_json::from_value(
                json!({"decision":"duplicate","target_id":target,"reason":"Already in canon."}),
            )
            .unwrap(),
        );
        save(root, &book).unwrap();
        assert!(enqueue(root, None, "unchecked").unwrap().is_empty());
        assert_eq!(enqueue(root, None, "decided").unwrap(), vec![a, b]);
        enqueue(root, None, "decided").unwrap();
        // Simulate reopening a book after a persisted claim.
        let mut book = load(root).unwrap();
        book.review_jobs[0].state = "running".into();
        save(root, &book).unwrap();
        let runtime = Arc::new(Keeper {
            calls: AtomicUsize::new(0),
            target,
        });
        let mut context = ToolContext::new(Arc::new(WorkspaceService::new(root)));
        context.subagents = Some(runtime.clone());
        wake(root, &context).unwrap();
        wake(root, &context).unwrap();
        for _ in 0..100 {
            let book = load(root).unwrap();
            if book.review_jobs.iter().all(|j| j.state == "done") {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        let book = load(root).unwrap();
        assert_eq!(book.records.len(), 1);
        assert_eq!(book.records[0].finding.detail, "The lake is unnamed.");
        assert_eq!(runtime.calls.load(Ordering::SeqCst), 1);
        assert_eq!(book.review_jobs.len(), 2);
        assert!(
            book.review_jobs
                .iter()
                .all(|j| j.result_id == Some(target) && j.state == "done")
        );
        assert!(enqueue(root, None, "all").unwrap().is_empty());
    }
}
