# CanonWright book agents and work queues

## Product contract

The author talks to one Chat agent. It can see the active project, passages, canon, outstanding decisions, work history, and saved results. It can answer directly, open a preview, and place requests into the appropriate inbox. A request returns a work ID immediately. The author can continue the conversation or change sessions while the job runs.

Five autonomous roles consume work. A role is an authority over specific operations. The dispatcher, validator, and file writer are ordinary application services. They cost no model calls. Each model invocation receives one job, the applicable version of its skill, the book type, the author’s project directives, the relevant data, and its role’s actual tool registry.

## Role contracts

The identifiers below name the target operations. Phase one retains existing tool names at the application boundary.

| Role | Behind-the-scenes skills | Tool authority | Standing directive |
| --- | --- | --- | --- |
| Chat | interpret_request, inspect_book, show_item, dispatch_work, report_progress, collect_author_decision | Read manuscript, canon, templates, issues, work and receipts. Open previews, submit typed requests to the five roles, attach author answers or authorization | Keep the conversation available. Carry the author’s scope and cited attachments into the request. Report a change as complete only from its receipt. |
| Writer | write_to_book, update_book, reorder_sections, build_manuscript, work_manuscript_issue, request_canon_update, request_canon_review, request_editorial_review | Read outline, canon and issues. Create sections, patch passages against a revision, order chapters, compile a combined manuscript, resolve a verified issue, submit downstream requests | Work within the author’s instructions. Preserve voice and evidence. A change to source text identifies the sections and revisions that may invalidate canon. |
| Canon Keeper | extract_from_passage, populate_template, resolve_links, submit_canon_proposal, apply_canon_verdict, sync_after_edit | Read sources, records, templates, backlinks and verdicts. Stage record/relationship patches and submit proposals. Apply an authorized verdict by ID | Preserve stable IDs, source passages and relationship direction. Omit unknown values. Execute an existing decision once. |
| Canon Reviewer | verify_proposal, adjudicate_identity, check_relationships, issue_canon_verdict, refer_source_conflict | Read the proposal, candidate targets, citations and templates. Issue a structured disposition or refer a source conflict | Decide whether canon reflects the manuscript. Identify the exact target and change. Leave creative uncertainty explicit. |
| Editorial Reviewer | review_passage, review_across_sections, test_rule_example, assess_table_usability, create_manuscript_issue, update_manuscript_issue | Read manuscript, canon, author brief and issues. Create or amend a local manuscript issue, or close a referral with an explanation | Evaluate the reader or player experience. State the problem, evidence and completion criterion. An intentional ambiguity may require no edit. |
| Process Manager | create_skill, update_skill, retire_skill, assign_skill, create_template, update_template, migrate_records, test_process_change | Inspect project processes and examples. Version, validate, publish and roll back skill/template changes. Preview and run schema migrations | Own the processes and their organization. Keep version history and test against representative existing records. Role privileges belong to application policy. |

All roles can request a process change. Project skills and templates can specialize novel, rulebook and content-book workflows without creating new roles. Runtime allowlists must cover indirect tools, including shell, file writes, GitHub, MCP discovery and delegation. Local manuscript issues and publishing external issues are separate operations.

## Routes and completion receipts

| Producer and request | Subscriber | Durable result |
| --- | --- | --- |
| Chat submits author-directed writing or author authorizes work on an issue | Writer | Source revision or build artifact; affected-section IDs for canon sync |
| Writer or Chat requests canon extraction/update | Keeper | Cited proposal with typed field and link changes |
| Keeper submits a proposal, or Chat/Writer challenges a canon claim | Canon Reviewer | One verdict tied to the proposal version, target, evidence and correction |
| Canon Reviewer issues an executable verdict | Application under Keeper authority | Accepted record ID/revision, or closed duplicate, with redirected links |
| Saved decision needs interpretation or an additional extraction | Keeper | An executable correction or a specific blocked reason; no second canon review |
| Chat/Writer requests editorial review, or Canon Reviewer refers contradictory passages | Editorial Reviewer | Linked manuscript issue with completion criteria, or explained closure |
| Editorial Reviewer creates an issue | Writer backlog | Source edit after authorization and a verified resolution on the issue |
| Any role requests a workflow change | Process Manager | Tested, versioned skill or template and migration receipt |
| Any role needs an author decision | Chat | Focused question and saved answer routed back to the original job |

## Queue contract

Use one logical durable queue per book with filtered role inboxes. Named operations determine recipients. Each item carries a stable ID, book, kind, owner, input IDs and revisions, originating request, authorization scope, state, attempts, timestamps, and result or error. Link children to the originating job so Chat can explain a multi-step request without reconstructing conversation history.

The target dispatcher claims eligible work through a transaction and a renewable lease. Start with one model worker per book. Wake workers when requests arrive and recover expired claims when the project runtime starts. Persist the outcome and its downstream request together. Application writes use the job ID as an idempotency key. Compare current revisions before mutation. A changed source produces a specific conflict. A transient runtime failure is eligible for bounded retry; author questions wait for an answer. Completed jobs retain receipts after the originating proposal has merged away.

For chapter files, persist the intended operation and expected revision before writing. On recovery compare the source hash with both expected and resulting hashes to distinguish already applied, safe to retry, or conflict. Canon changes and link redirects can share one manifest replacement today. A later SQLite work index should use an outbox and the book receipt to reconcile across stores.

## Implementation sequence and acceptance gates

1. **Canon review loop (first delivery).** Queue existing unchecked proposals separately from saved decisions. Return immediately to chat. Use local duplicate matching, a read-only Canon Reviewer for first reviews and a read-only Keeper for older prose instructions that require interpretation. The application validates and writes accepted changes. Persist executable verdicts before apply, and save the canon result and queue receipt together. Verify recovery from an interrupted claim, duplicate enqueue, links, and zero reviewer calls on saved instructions.
2. **General dispatcher and Chat boundary.** Add typed job envelopes, lease/attempt history, cancellation, dependencies, startup independent of a chat turn, and status events. Expose the read/dispatch Chat allowlist only when each existing user-facing write has a routed replacement. Verify a chat can submit work, switch books and later inspect the original result. Verify denial at tool execution for out-of-role writes.
3. **Writer and Keeper requests.** Route chapter edits, compilation, extraction and source-change events through their inboxes. Use structured proposals and exact patch preconditions. Exercise a multi-file novel and a rulebook. Preserve unrelated canon when a single section changes.
4. **Editorial and issue lifecycle.** Introduce the separate Editorial Reviewer inbox and the create-manuscript-issue skill. Move the current manuscript-issue disposition behind that role. Issue creation closes the review. Authorized Writer work modifies the source and records a verified resolution. Test a canon error and a source contradiction as distinct cases.
5. **Process Manager and migration.** Route skill/template requests from chat or workers. Ship versioning, example-backed validation, migration previews and recovery. Verify that a schema change preserves populated fields and links and cannot widen role permissions.

### Phase one as shipped in 0.2.7

The review implementation stores jobs in the existing book manifest, allowing canon updates, redirected links and receipts to share an atomic save. It supports one dispatcher per book in the running desktop process. On reopening the app the next book chat turn reconnects the model runtime and resumes ready or interrupted work. The Review pane distinguishes first reviews from saved decisions and shows active counts and errors. The existing chat tool catalog, direct author overrides and local manuscript-issue flow remain available while later inboxes are built. Independent multi-process dispatch and launch-time background inference belong to phase two.

### Phase two delivery in 0.2.8

Book Chat uses a dedicated read, preview, dispatch, control and status registry. The four general inboxes accept scoped requests and the Canon Reviewer consumes the existing proposal route. Workers receive role-specific tool registries. The dispatcher loads its work at desktop startup for all registered books. Saving a missing provider key also wakes queued work. Launch-time workers use the default Flash model and the configured DeepSeek endpoint. Interactive workers use the session model.

A per-book OS file lock owns the dispatch lease until exit or process termination. Each job persists its owner, request, selected book context, attempts, worker ID, state, last update and outcome. Proposals created by a Keeper job retain its parent ID. Workers journal write receipts. An interrupted write with an uncertain outcome fails for inspection before retry. Chat can cancel and explicitly retry work, inspect a job or summarize status across the book. Desktop events refresh the open book as background work changes it.

This milestone provides dispatch and initial operations for all five roles. The richer source-change dependency graph, publication-quality editorial workflows, and versioned Process Manager migrations remain in phases three through five.

### Phase three delivery in 0.2.9

Writer passage edits persist the chapter revision, reversible edit, and a child Keeper job before replacing the source file. Writer-created sections with initial text and chapter saves from the desktop editor also enqueue reconciliation. A newer edit carries forward the earlier unprocessed changes to the same chapter. Recovery checks the expected and resulting hashes before replaying an interrupted file write.

The Keeper receives changed excerpts, records identified through citations, names or aliases, and their direct connections. Its scoped worker has no manuscript browsing tools. It proposes ID-addressed patches, explicit list additions/removals, changed citations and new entries. The Canon Reviewer checks that packet only when it contains a proposed canon mutation. The application verifies the source and record snapshot before applying the saved verdict in one manifest transaction. An unsupported deleted passage can leave a record linked and flagged for author confirmation. Chat can inspect the sync receipt and the Review pane shows the chapter and job state.

External edits made directly to chapter files and bulk source imports do not currently emit chapter change jobs. A stale revision prevents a pending job from overwriting such edits.

### Phase four delivery in 0.2.10

The Canon Reviewer can return a source conflict from a finding or a chapter-sync verdict. The application checks the cited passages and queues an Editorial Reviewer with those passages and their context. Editorial records an explained closure, a question for the author, or a local manuscript issue with a specific target passage and completion criteria. Issue creation and Writer assignment share one save. Existing open issues with matching evidence and criteria reuse the issue and assignment IDs.

The assigned Writer edits the target section through the revision-checked chapter tool. The resolve tool submits a changed passage and the Writer’s explanation for verification. The application checks that this Writer job actually changed the submitted passage. A separate Editorial Reviewer checks the live text, original issue, changed text, related citations and criteria. Its verdict and the resolution receipt are saved together. A failed verification returns the issue to the Writer with feedback once. A second failure leaves the issue open and the verification job awaiting author input. Each handoff retains its parent job ID and remains visible in Chat and Review.

Existing open issues created before this release remain available for author-directed Writer assignment. Automatic repair applies to newly confirmed manuscript issues.

### Phase five delivery in 0.2.11

Chat queues a skill or template request for the Process Manager and reads its history and job receipts. The manager stages Markdown skills with an owning role and representative examples. An isolated worker applies the proposed instructions to each example without tools. The host checks required and forbidden phrases and publishes the tested revision for the assigned agent. Updating an existing skill requires its current fingerprint.

Template changes use stable field keys and explicit renames. The host previews them against every affected canon and pending entry, then repeats validation against current records at publication. Every resolved or unresolved link must retain its name, target and context. Removed values stay on their entry under Retained fields. Job publication records migration counts and version IDs together with the changes.

Ask Chat to undo a change by name or version. Rollbacks proceed newest first for each resource and migrate current records using the inverse field mapping. Later edits survive, and later values without a field in the restored template remain available under Retained fields. Undoing creation of a populated area keeps the area and its entries. A failed check leaves the job available for inspection and retry.
