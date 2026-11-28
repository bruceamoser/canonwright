import { type CSSProperties, type ReactNode, type Ref, useEffect, useImperativeHandle, useMemo, useRef, useState } from "react";
import { ArrowDown, ArrowLeft, ArrowUp, BookOpen, FileText, Plus, Save } from "lucide-react";

import { listen } from "@tauri-apps/api/event";
import { openExternal, pickDirectory, pickManuscript, revealPath } from "../backend";
import type { WorkspaceSummary } from "../types";
import { type CanonLink, type BookAction, type BookStudioHandle, type BookView, type Finding, type Manuscript, type SectionView, type SkillDraft, compareCanonTitles, studioApi } from "./studio";
import AgentsPanel from "./AgentsPanel";
import ArtifactViewer from "../ArtifactViewer";
import { LinkEditor, RecordLinks } from "./CanonLinks";
import "./studio.css";

type Props = {
  project: WorkspaceSummary;
  ref: Ref<BookStudioHandle>;
  chat: ReactNode;
  navigation: (open: (tab: DetailTab) => void, active: DetailTab | null, areas: string[], selectedArea: string, openArea: (area: string) => void) => ReactNode;
  files: ReactNode;
  preview: ReactNode;
  onClosePreview: () => void;
  actions: BookAction[];
  engineBusy: boolean;
  onLibrary: () => void;
  onReview: (message: string) => void;
};

type Pane = "outline" | "preview" | "details";

type Surface = "overview" | "draft" | "records";

type DetailTab = "files" | "manuscript" | "canon" | "review" | "agents" | "skills";
const DETAIL_TABS: [DetailTab, string][] = [["files", "Files"], ["manuscript", "Manuscript"], ["canon", "Canon"], ["review", "Review"], ["agents", "Agents"], ["skills", "Skills"]];

const KIND_LABEL = { novel: "Novel", rulebook: "Core rulebook", content_book: "TTRPG content book" };

export default function BookStudio({ project, ref, chat, navigation, files, preview, onClosePreview, actions, engineBusy, onLibrary, onReview }: Props) {
  const api = useMemo(() => studioApi(project.id), [project.id]);
  const [view, setView] = useState<BookView | null>(null);
  const [sectionId, setSectionId] = useState<string | null>(null);
  const [loaded, setLoaded] = useState<SectionView | null>(null);
  const [editingPdf, setEditingPdf] = useState(false);
  const [draft, setDraft] = useState("");
  const [detailTab, setDetailTab] = useState<DetailTab>("files");
  const [surface, setSurface] = useState<Surface>("overview");
  const [recordId, setRecordId] = useState<string | null>(null);
  const consumedActions = useRef(0);
  const [area, setArea] = useState("");
  const [sectionTitle, setSectionTitle] = useState("");
  const [adding, setAdding] = useState(false);
  const [query, setQuery] = useState("");
  const [detailsOpen, setDetailsOpen] = useState(true);
  const [widths, setWidths] = useState<Partial<Record<Pane, number>>>(() => {
    try { return JSON.parse(localStorage.getItem("canonwright.panes") ?? "{}") ?? {}; } catch { return {}; }
  });
  const columns = useRef<HTMLDivElement>(null);
  const refreshGeneration = useRef(0);
  const [manuscript, setManuscript] = useState<Manuscript | null>(null);
  const [skillDraft, setSkillDraft] = useState<SkillDraft | null>(null);
  const [manual, setManual] = useState<Finding | null>(null);
  const [answer, setAnswer] = useState<{ id: string; text: string } | null>(null);
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const editor = useRef<HTMLTextAreaElement>(null);
  const dirty = loaded !== null && draft !== loaded.text;

  useEffect(() => {
    let live = true;
    void api.book().then((book) => {
      if (!live) return;
      setView(book);
      if (book.book.records.some(record => record.pending)) setDetailTab("review");
      const last = localStorage.getItem(`canonwright.book.${project.id}`);
      setSectionId(book.book.sections.find((section) => section.id === last)?.id ?? book.book.sections[0]?.id ?? null);
    }).catch((cause: unknown) => { if (live) setError(String(cause)); });
    return () => { live = false; };
  }, [api, project.id]);

  useEffect(() => {
    setLoaded(null);
    setDraft("");
    setManual(null);
    if (sectionId === null) return;
    const currentId = sectionId;
    let live = true;
    void api.section(currentId).then((section) => {
      if (!live) return;
      setLoaded(section);
      setDraft(section.text);
      localStorage.setItem(`canonwright.book.${project.id}`, currentId);
    }).catch((cause: unknown) => { if (live) setError(String(cause)); });
    return () => { live = false; };
  }, [api, project.id, sectionId]);

  useEffect(() => {
    if (!dirty) return;
    function guard(event: BeforeUnloadEvent) { event.preventDefault(); }
    window.addEventListener("beforeunload", guard);
    return () => window.removeEventListener("beforeunload", guard);
  }, [dirty]);

  async function act(label: string, work: () => Promise<void>) {
    setBusy(label);
    setError("");
    setNotice("");
    try { await work(); }
    catch (cause) { setError(String(cause)); }
    finally { setBusy(""); }
  }

  async function save() {
    if (!sectionId || !loaded || !dirty) return;
    const updated = await api.save(sectionId, loaded.revision, draft);
    setLoaded(updated);
    setManuscript(null);
    return updated;
  }

  function leave(next: () => void) {
    if (!dirty) { next(); return; }
    void act("Saving section…", async () => { await save(); next(); });
  }

  function select(id: string) {
    leave(() => {
      onClosePreview();
      setRecordId(null);
      setSectionId(id);
      setEditingPdf(false);
      setManuscript(null);
      setSurface("draft");
    });
  }

  async function importSource(folder: boolean) {
    const path = folder ? await pickDirectory("Choose a manuscript folder") : await pickManuscript();
    if (path === null) return;
    await save();
    const count = await api.import(path);
    const book = await api.book();
    setView(book);
    setSectionId(book.book.sections.at(-count)?.id ?? null);
    setManuscript(null);
    setNotice(`Imported ${count} ${count === 1 ? "section" : "sections"}. Review the reading order in the outline.`);
  }

  function move(index: number, offset: number) {
    const ids = view?.book.sections.map(({ id }) => id) ?? [];
    const neighbor = index + offset;
    if (neighbor < 0 || neighbor >= ids.length) return;
    const id = ids[index];
    if (!id) return;
    ids.splice(index, 1);
    ids.splice(neighbor, 0, id);
    void act("Reordering…", async () => {
      await api.order(ids);
      setView(await api.book());
      setManuscript(null);
    });
  }

  useImperativeHandle(ref, () => ({
    showAgents: () => openTab("agents"),
    async prepare() {
      const field = editor.current;
      const selection = field ? field.value.slice(field.selectionStart, field.selectionEnd) : "";
      await save();
      const activeRecord = !preview && !manuscript && surface === "records"
        ? view?.book.records.find(record => record.id === recordId) : undefined;
      return { projectId: project.id, sectionId: !preview && !manuscript && surface === "draft" ? sectionId : null,
        recordId: activeRecord?.id ?? null, area: activeRecord?.finding.area ?? (detailsOpen && detailTab === "canon" ? selectedArea : null), selection };

    },
  }));

  useEffect(() => {
    const incoming = actions.slice(consumedActions.current);
    consumedActions.current = actions.length;
    for (const action of incoming) {
      if (action.book_skill) setSkillDraft(action.book_skill);
      if (action.book_show === "manuscript") {
        onClosePreview();
        setRecordId(null);
        void api.compiled().then(setManuscript).catch((cause: unknown) => setError(String(cause)));
      } else if (action.book_show) {
        onClosePreview();
        setRecordId(null);
        setEditingPdf(false);
        setSectionId(action.book_show);
        setManuscript(null);
        setSurface("draft");
      }
      if (action.book_reference) {
        setDetailsOpen(true);
        onClosePreview();
        setSurface("records");
        setDetailTab(action.book_reference.review ? "review" : "canon");
        setManuscript(null);
        setArea(action.book_reference.area);
        setRecordId(action.book_reference.id);
      }
      if (action.book_changed) {
        if (sectionId) {
          void api.section(sectionId).then((section) => {
            setLoaded(section);
            setDraft(section.text);
          }).catch((cause: unknown) => setError(String(cause)));
        }
      }
    }
    if (incoming.some(action => action.book_changed || action.book_records_changed)) {
      const generation = ++refreshGeneration.current;
      void api.book().then(book => {
        if (generation === refreshGeneration.current) setView(book);
      }).catch((cause: unknown) => setError(String(cause)));
    }
  }, [actions, api, onClosePreview, sectionId]);

  const disabled = engineBusy || busy !== "";
  const current = view?.book.sections.find(({ id }) => id === sectionId);
  const pdfArtifact = useMemo(() => current?.original_pdf ? {kind:"file" as const,title:current.title,uri:current.original_pdf} : null,[current?.original_pdf,current?.title]);
  const selectedArea = view?.areas.includes(area) ? area : view?.areas[0] ?? "";
  const reviewRecords = view?.book.records.filter(r =>
    !["issue_resolved", "referral_closed"].includes(r.editorial?.decision ?? "") &&
    (r.pending || r.finding.area === "Review" || r.editorial?.decision === "manuscript_issue" || (r.finding.kind === "question" && !r.answer))
  ) ?? [];
  const jobs = view?.book.review_jobs ?? [];
  const tracked = new Set(jobs.map(j => j.id));
  const unqueued = view?.book.records.filter(r => r.pending && !tracked.has(r.id)) ?? [];
  const unchecked = unqueued.filter(r => !r.editorial).length;
  const decided = unqueued.filter(r => r.editorial && ["approve","duplicate","merge","revise"].includes(r.editorial.decision)).length;
  const failedReviews = jobs.filter(j => j.state === "failed" && !j.work && !j.sync && !j.editorial);
  useEffect(() => {
    let live = true;
    let timer: number | undefined;
    let unlisten: (() => void) | undefined;
    void listen<string>("canonwright:book-updated", event => {
      if (!live || event.payload !== project.path) return;
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        const generation = ++refreshGeneration.current;
        void api.book().then(book => {
          if (live && generation === refreshGeneration.current) setView(book);
        }).catch((cause: unknown) => { if (live) setError(String(cause)); });
      }, 100);
    }).then(stop => { if (live) unlisten = stop; else stop(); }).catch(() => {});
    return () => { live = false; window.clearTimeout(timer); unlisten?.(); };
  }, [api, project.path]);
  const issues = view?.book.records.filter(r => r.editorial?.decision === "manuscript_issue").length ?? 0;
  const sourceRecords = detailTab === "canon" ? view?.book.records.filter(r =>
    !r.pending && !["issue_resolved", "referral_closed"].includes(r.editorial?.decision ?? "") &&
    (r.finding.kind !== "question" || r.answer) && r.finding.area === selectedArea
  ) ?? [] : reviewRecords;
  const records = sourceRecords.filter(r => r.finding.title.toLowerCase().includes(query.trim().toLowerCase()));

  const completedReviews = jobs.filter(j => j.state === "done" && (
    (!j.work && !j.sync && !j.editorial) || ["canon_reviewer","editorial_reviewer"].includes(view?.job_roles[j.id] ?? "")
  )).sort((a,b) => (b.updated_at ?? 0) - (a.updated_at ?? 0)).map(j => {
    const target = view?.book.records.find(r => r.id === (j.result_id ?? j.editorial?.issue ?? j.id));
    const title = j.reviewed_title ?? target?.finding.title ?? j.work?.request ?? j.instructions ?? j.id;
    return {job:j, target, title};
  }).filter(item => item.title.toLowerCase().includes(query.trim().toLowerCase()));

  if (detailTab === "canon") records.sort(compareCanonTitles);

  function resize(pane: Pane, requested: number) {
    const host = columns.current;
    if (!host) return;
    const other = Array.from(host.querySelectorAll<HTMLElement>("[data-pane]"))
      .filter(el => el.dataset.pane !== pane).reduce((sum, el) => sum + el.offsetWidth, 0);
    const width = Math.max(125, Math.min(requested, host.clientWidth - other - 230));
    setWidths(old => {
      const updated = { ...old, [pane]: width };
      localStorage.setItem("canonwright.panes", JSON.stringify(updated));
      return updated;
    });
  }

  function openTab(tab: DetailTab) {
    setDetailsOpen(true);
    setDetailTab(tab);
    setQuery("");
    if (tab === "review" || tab === "canon") {
      setArea(tab === "review" ? "" : view?.areas[0] ?? "");
      setManual(null);
    }
  }

  function divider(pane: Pane) {
    const dir = pane === "outline" ? 1 : -1;
    const width = () => columns.current?.querySelector<HTMLElement>(`[data-pane="${pane}"]`)?.offsetWidth ?? 0;
    return <div className="studio-divider" role="separator" aria-orientation="vertical" tabIndex={0}
      aria-label={`Resize ${pane} pane`} aria-valuenow={widths[pane] ?? width()}
      onPointerDown={event => {
        event.preventDefault();
        const el = event.currentTarget;
        el.focus();
        const start = event.clientX;
        const initial = width();
        el.setPointerCapture(event.pointerId);
        el.onpointermove = move => resize(pane, initial + (move.clientX - start) * dir);
        el.onlostpointercapture = () => { el.onpointermove = null; };
      }}
      onKeyDown={event => {
        if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
        event.preventDefault();
        resize(pane, width() + (event.key === "ArrowLeft" ? -16 : 16) * dir);
      }} />;
  }

  return (
    <div className="book-studio">
      <header className="studio-top">
        <button type="button" onClick={() => leave(onLibrary)} disabled={disabled}><ArrowLeft size={15} /> Library</button>
        <span className="studio-brand">CanonWright</span>
        <strong className="studio-book-title">{project.name}</strong>
        <button type="button" disabled={disabled || !view?.book.sections.length}
          onClick={() => void act("Opening manuscript…", async () => {
            await save();
            onClosePreview();
            setRecordId(null);
            setManuscript(await api.compiled());
          })}>View manuscript</button>
        <button type="button" className="studio-primary" disabled={disabled || !view?.book.sections.length}
          onClick={() => void act("Building manuscript…", async () => {
            await save();
            onClosePreview();
            setRecordId(null);
            setManuscript(await api.build());
          })}><BookOpen size={16} /> Build manuscript</button>
        {!detailsOpen && <button onClick={() => setDetailsOpen(true)}>Show details</button>}
      </header>

      {(busy || error || notice) && (
        <div className={`studio-status ${error ? "studio-error" : ""}`} role={error ? "alert" : "status"}>
          {error || busy || notice}
        </div>
      )}

      <div className="studio-columns" ref={columns} style={Object.fromEntries(
        Object.entries(widths).map(([pane, width]) => [`--${pane}-width`, `${width}px`])
      ) as CSSProperties}>
        <aside data-pane="outline" className="studio-outline" aria-label="Book outline">
          {navigation(openTab, detailsOpen ? detailTab : null, view?.areas ?? [], selectedArea, name => {openTab("canon"); setArea(name);})}
        </aside>

        {divider("outline")}
        <section className="studio-assistant" aria-label="Writing assistant">
          {chat}
        </section>

        {(preview || manuscript || (surface === "draft" && current) || view?.book.records.some(record => record.id === recordId)) && divider("preview")}
        {preview && <div data-pane="preview" className="studio-preview studio-engine">{preview}</div>}

        {!preview && (manuscript || (surface === "draft" && current) || view?.book.records.some(record => record.id === recordId)) && (
          <aside data-pane="preview" className="studio-book-preview" aria-label="Book preview">
          {manuscript ? (
            <>
              <div className="studio-page-header"><div><p className="studio-label">Compiled manuscript</p><h1>{project.name}</h1></div>
                <button type="button" onClick={() => { setManuscript(null); if (surface === "draft") setSurface("overview"); }}>Close preview</button>
              </div>
              {manuscript.path && <div className="studio-file-path">
                <button type="button" onClick={() => void act("Opening build folder…", async () => {
                  await revealPath(project.path, manuscript.path);
                })}>Show manuscript file</button>
              </div>}
              <pre className="studio-compiled">{manuscript.text}</pre>
            </>
          ) : surface === "draft" && current && recordId === null ? (
            <>
              <div className="studio-page-header">
                <div><p className="studio-label">Draft</p><h1>{current.title}</h1></div>
                <button type="button" onClick={() => leave(() => setSurface("overview"))}>Close preview</button>
                {pdfArtifact ? <button onClick={() => leave(() => setEditingPdf(!editingPdf))}>{editingPdf ? "View original PDF" : "Edit manuscript text"}</button>
                 : <button disabled={disabled} onClick={() => void act("Attaching PDF…",async () => {
                    const path=await pickManuscript(true);if(!path)return;
                    await save();await api.attachPdf(current.id,path);setView(await api.book());setEditingPdf(false);
                  })}>Attach original PDF…</button>}
                {(!pdfArtifact || editingPdf) && <button type="button" disabled={disabled || !dirty} onClick={() => void act("Saving section…", async () => {
                  await save();
                  setNotice("Section saved");
                })}><Save size={15} /> Save</button>}
              </div>
              {pdfArtifact && !editingPdf ? <div className="min-h-0 flex-1"><ArtifactViewer artifact={pdfArtifact} workspacePath={project.path} revision={0} /></div> : <>
              <textarea ref={editor} className="studio-editor" aria-label="Section Markdown" placeholder="Start writing…"
                disabled={disabled || loaded === null} value={draft} onChange={(event) => setDraft(event.target.value)}
                onKeyDown={(event) => {
                  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
                    event.preventDefault();
                    if (dirty) void act("Saving section…", async () => { await save(); });
                  }
                }} />
              <footer>{dirty ? "Unsaved changes" : loaded ? "Saved locally" : "Loading section…"} · {draft.trim().split(/\s+/).filter(Boolean).length} words</footer></>}
            </>
          ) : (
              <>
                <div className="studio-page-header">
                  <h2>{view?.book.records.find(record => record.id === recordId)?.pending ? "Review finding" : "Reference entry"}</h2>
                  <button type="button" onClick={() => setRecordId(null)}>Close preview</button>
                </div>
              {view?.book.records.filter((record) => recordId === record.id).map((record) => (
                <article key={record.id} className="studio-card">
                  <p className="studio-label">{record.finding.kind === "question" ? "Question" : record.finding.area}</p>
                  <h3>{record.finding.title}</h3>
                  {record.unverified_sections?.map(id => <p key={id}>Source changed in {view.book.sections.find(s => s.id === id)?.title ?? "this chapter"}. This entry needs confirmation.</p>)}
                  <p>{record.finding.detail}</p>
                  <dl className="studio-entry-fields">{view.templates[record.finding.area]?.fields.map(field => {
                    const value = record.finding.fields[field.key];
                    if (!value) return null;
                    return <div key={field.key}><dt>{field.label}</dt><dd>{Array.isArray(value)
                      ? <ul>{value.map((item, index) => {
                          if (typeof item === "string") return <li key={index}>{item}</li>;
                          const target = view.book.records.find(entry => entry.id === item.target_id);
                          return <li key={index}>{target ? <button type="button" className="studio-canon-link"
                            onClick={() => setRecordId(target.id)}>{target.finding.title}</button> : <span>{item.name} (unresolved)</span>}
                            {target?.pending && " (in review)"} · {item.context}</li>;
                        })}</ul> : value}</dd></div>;
                  })}</dl>
                  <RecordLinks record={record} entries={view.book.records} open={setRecordId} />
                  {record.research_sources?.map(c => <p key={JSON.stringify(c)}><strong>Research · {c.basis === "checked_source" ? "Source consulted" : "Unverified lead"}</strong><br/>{c.source}</p>)}
                  {record.author_sources?.map(c => <div key={JSON.stringify(c)}><strong>Author statement</strong><blockquote>{c.quote}</blockquote><small>Chat message {c.message}</small></div>)}
                  {record.finding.evidence && <blockquote>{record.finding.evidence}</blockquote>}
                  {record.citations?.map(cite => <div key={cite.section + cite.evidence}><blockquote>{cite.evidence}</blockquote>
                    <button type="button" onClick={() => select(cite.section)}>Open cited section</button></div>)}
                  {record.section && <button type="button" disabled={disabled} onClick={() => select(record.section!)}>Open source section</button>}
                  {record.editorial && <p><strong>{record.editorial.decision === "source_conflict" ? "Referred to Editorial" : record.editorial.decision === "manuscript_issue" ? "Manuscript issue" : record.editorial.decision === "revise" ? "Returned for revision" : record.editorial.decision === "ask_author" ? "Author decision" : "Editorial review"}</strong> {record.editorial.reason}</p>}
                  {record.editorial?.decision === "manuscript_issue" && !jobs.some(j=>j.editorial?.issue===record.id && ["ready","running"].includes(j.state)) && <button type="button" disabled={disabled} onClick={() => onReview(`Queue Writer work on manuscript issue ${record.id}. Use its saved criteria and cited section, verify the revised passage and close the issue.`)}>Work on manuscript issue</button>}
                  {record.pending && (!record.editorial || record.editorial.decision === "revise") && <button type="button" disabled={disabled} onClick={() => onReview(`Queue finding ${record.id} once using book_editorial_review. Report its queue status.`)}>{record.editorial ? "Apply saved decision" : "Queue canon review"}</button>}
                  {record.pending && !["source_conflict","referral_closed","issue_resolved","manuscript_issue"].includes(record.editorial?.decision ?? "") && [true, false].map(accept => <button key={String(accept)} disabled={disabled}
                    onClick={() => void act("Updating review…", async () => {
                      await api.decide(record.id, accept);
                      setView(await api.book());
                      setRecordId(null);
                    })}>{accept ? (record.finding.kind === "entry" ? "Accept entry" : "Keep in review") : "Dismiss"}</button>)}
                  {record.finding.kind === "question" && !["source_conflict","referral_closed","issue_resolved","manuscript_issue"].includes(record.editorial?.decision ?? "") && <>
                    {record.answer && <p><strong>Answer</strong> {record.answer}</p>}
                    {answer?.id === record.id ? <form className="studio-form" onSubmit={(event) => {
                      event.preventDefault();
                      void act("Saving answer…", async () => {
                        await api.answer(record.id, answer.text);
                        setView(await api.book());
                        setAnswer(null);
                      });
                    }}>
                      <label>Your answer<textarea required value={answer.text}
                        onChange={(event) => setAnswer({ id: record.id, text: event.target.value })} /></label>
                      <button type="submit" disabled={disabled}>Save answer</button>
                    </form> : <button type="button" disabled={disabled}
                      onClick={() => setAnswer({ id: record.id, text: record.answer ?? "" })}>
                      {record.answer ? "Edit answer" : "Answer question"}
                    </button>}
                  </>}
                  {record.github_url ? <button type="button" onClick={() => void act("Opening issue…", async () => { await openExternal(record.github_url!); })}>View GitHub issue</button>
                    : !area && view?.book.repository && <button type="button" disabled={disabled} onClick={() => {
                      if (!window.confirm(`Share the title and finding details with ${view.book.repository} as a GitHub issue?`)) return;
                      void act("Opening GitHub issue…", async () => {
                        await api.openIssue(record.id);
                        setView(await api.book());
                      });
                    }}>Open GitHub issue</button>}
                </article>
              ))}
              </>
            )}
          </aside>
        )}

        {detailsOpen && <>{divider("details")}<aside data-pane="details" className="studio-writing" aria-label="Book details">
          <header className="studio-detail-heading">
            <button className="studio-close-details" aria-label="Close details" onClick={() => setDetailsOpen(false)}>×</button>
            <h2>{DETAIL_TABS.find(([tab]) => tab === detailTab)?.[1]}</h2>
            <p>{project.name}{view ? ` · ${KIND_LABEL[view.book.kind]}` : ""}</p>
          </header>
          <nav className="studio-detail-tabs" aria-label="Book detail areas">
            {DETAIL_TABS.map(([tab, label]) => (
              <button type="button" key={tab} aria-pressed={detailTab === tab} onClick={() => openTab(tab)}>{label}{tab === "review" && reviewRecords.length > 0 ? ` (${reviewRecords.length})` : ""}</button>
            ))}
          </nav>
          {detailTab === "files" ? <div className="studio-detail-files studio-engine">{files}</div> :
          detailTab === "agents" ? (view && <AgentsPanel view={view} />) :
          detailTab === "manuscript" ? <div className="studio-detail-body">
          <ol className="studio-sections">
            {view?.book.sections.map((section, index) => (
              <li key={section.id} className={surface === "draft" && section.id === sectionId && manuscript === null ? "studio-selected" : ""}>
                <button type="button" className="studio-section" title={section.title} disabled={disabled} onClick={() => select(section.id)}>
                  <FileText size={14} /><span>{section.title}</span>
                </button>
                <button type="button" className="studio-icon" aria-label={`Move ${section.title} up`} disabled={disabled || index === 0} onClick={() => move(index, -1)}><ArrowUp size={13} /></button>
                <button type="button" className="studio-icon" aria-label={`Move ${section.title} down`} disabled={disabled || index === view.book.sections.length - 1} onClick={() => move(index, 1)}><ArrowDown size={13} /></button>
              </li>
            ))}
          </ol>
          {adding ? (
            <form className="studio-form" onSubmit={(event) => {
              event.preventDefault();
              void act("Adding section…", async () => {
                await save();
                const id = await api.add(sectionTitle);
                setView(await api.book());
                setSectionId(id);
                setSurface("draft");
                setManuscript(null);
                setSectionTitle("");
                setAdding(false);
              });
            }}>
              <label>Section title<input required value={sectionTitle} onChange={(event) => setSectionTitle(event.target.value)} autoFocus /></label>
              <button type="submit" disabled={disabled}>Add section</button>
              <button type="button" onClick={() => setAdding(false)}>Cancel</button>
            </form>
          ) : <button type="button" className="studio-quiet" disabled={disabled || !view} onClick={() => setAdding(true)}><Plus size={15} /> Add section</button>}
          <div className="studio-import">
            <button type="button" disabled={disabled || !view} onClick={() => void act("Importing manuscript…", () => importSource(false))}>Import file…</button>
            <button type="button" disabled={disabled || !view} onClick={() => void act("Importing manuscript…", () => importSource(true))}>Import folder…</button>
          </div>
          </div> : detailTab === "canon" || detailTab === "review" ? (
            <div className="studio-stack studio-reference studio-detail-body">
              {detailTab === "canon" && <>
          <select aria-label="Reference area" value={selectedArea} onChange={event => {setArea(event.target.value); setManual(null); setQuery("");}}>
            {view?.areas.map(name => <option key={name}>{name}</option>)}
          </select>
          {view?.templates[selectedArea] && <details key={selectedArea} className="studio-template-guide">
            <summary>Entry template</summary>
            <dl className="studio-entry-fields">{view.templates[selectedArea].fields.map(field =>
              <div key={field.key}><dt>{field.label}</dt><dd>{field.guidance}</dd></div>)}</dl>
          </details>}
              </>}
              {detailTab === "review" && issues > 0 && <p>{issues} open manuscript {issues === 1 ? "issue" : "issues"}</p>}
              {detailTab === "review" && <>
                {failedReviews.length > 0 && <button disabled={disabled} onClick={() => void act("Retrying failed reviews…", async () => {
                  await api.retryFailedReviews(failedReviews.map(job => job.id));
                })}>Retry failed reviews · {failedReviews.length}</button>}
                {decided > 0 && <button disabled={disabled} onClick={() => onReview("Queue saved canon decisions by calling book_editorial_review once with scope decided.")}>Apply saved decisions · {decided}</button>}
                {unchecked > 0 && <button disabled={disabled} onClick={() => onReview("Queue first reviews by calling book_editorial_review once with scope unchecked.")}>Review unchecked · {unchecked}</button>}
              </>}
              <input type="search" aria-label="Filter records" placeholder={detailTab === "canon" ? "Find an entry…" : "Find a review item…"} value={query} onChange={event => setQuery(event.target.value)} />
              {detailTab === "canon" && selectedArea && <button type="button" disabled={disabled || !loaded || !!manuscript} onClick={() => {
                const field = editor.current;
                const evidence = field ? draft.slice(field.selectionStart, field.selectionEnd) : "";
                setManual({ fields: {}, kind: "entry", area: selectedArea, title: "", detail: "", evidence });
              }}><Plus size={15} /> Add entry</button>}
              {manual && <form className="studio-form" onSubmit={(event) => {
                event.preventDefault();
                if (!sectionId || !loaded) return;
                void act("Saving entry…", async () => {
                  const updated = await save();
                  const fields: Finding["fields"] = {};
                  for (const [key, value] of Object.entries(manual.fields)) {
                    const clean = Array.isArray(value) ? value.map(item => typeof item === "string" ? item.trim() : item).filter(Boolean) : value.trim();
                    if (clean.length) fields[key] = clean;
                  }
                  await api.record(sectionId, updated?.revision ?? loaded.revision, {...manual, fields});
                  setView(await api.book());
                  setManual(null);
                });
              }}>
                <label>Name<input required value={manual.title} onChange={(event) => setManual({ ...manual, title: event.target.value })} /></label>
                <label>Summary<textarea required value={manual.detail} onChange={(event) => setManual({ ...manual, detail: event.target.value })} /></label>
                {view?.templates[manual.area]?.fields.map(field => {
                  const value = manual.fields[field.key] ?? "";
                  if (field.format === "links") return <LinkEditor key={field.key} label={field.label}
                    value={Array.isArray(value) ? value.filter((v): v is CanonLink => typeof v !== "string") : []}
                    entries={view.book.records} onChange={links => setManual({...manual, fields:{...manual.fields,[field.key]:links}})} />;
                  return <label key={field.key}>{field.label}{field.format === "list" ? " (one per line)" : ""}
                    <textarea placeholder={field.guidance} value={Array.isArray(value) ? value.join("\n") : value}
                      onChange={event => setManual({...manual, fields: {...manual.fields,
                        [field.key]: field.format === "list" ? event.target.value.split("\n") : event.target.value}})} />
                  </label>;
                })}
                <label>Exact quote from open section<textarea required value={manual.evidence} onChange={(event) => setManual({ ...manual, evidence: event.target.value })} /></label>
                <button type="submit" className="studio-primary" disabled={disabled}>Save entry</button>
                <button type="button" onClick={() => setManual(null)}>Cancel</button>
              </form>}
              {records.length === 0 && <p className="studio-muted">{query ? "No matching records." : detailTab === "canon" ? "No entries in this area yet." : "No open review items."}</p>}
              {records.map((record) => (
                <button type="button" className="studio-record-row" key={record.id} aria-pressed={recordId === record.id}
                  onClick={() => {
                    leave(() => {
                      onClosePreview();
                      setManuscript(null);
                      setSurface("records");
                      setRecordId(record.id);
                    });
                  }}><span>{record.finding.title}</span>{detailTab === "review" && <small>{record.pending ? record.editorial?.decision === "manuscript_issue" ? "Manuscript issue" : record.editorial?.decision === "revise" ? "Needs revision" : record.editorial?.decision === "ask_author" ? "Author decision" : record.finding.area : record.finding.kind}</small>}</button>
              ))}
              {detailTab === "review" && completedReviews.length > 0 && <details className="studio-completed-reviews">
                <summary>Completed reviews ({completedReviews.length})</summary>
                {completedReviews.map(({job, title, target}) => <div key={job.id}>
                  <button type="button" className="studio-record-row" disabled={!target} onClick={() => target && leave(() => {
                    onClosePreview(); setManuscript(null); setSurface("records"); setRecordId(target.id);
                  })}><span>{title}</span><small>{job.author_decision ?? job.review_decision?.replaceAll("_"," ") ?? "Completed"}</small></button>
                </div>)}
              </details>}
            </div>
          ) : (
            <div className="studio-stack studio-reference studio-detail-body">
              {view?.skills.map((skill) => <p key={skill.id}>{skill.title}</p>)}
            </div>
          )}
          {skillDraft && <div className="studio-proposals">
            {skillDraft && <article className="studio-card">
              <p className="studio-label">Skill draft</p>
              <h3>{skillDraft.title}</h3>
              <p className="studio-skill-instructions">{skillDraft.instructions}</p>
              <button type="button" disabled={disabled} onClick={() => void act("Saving skill…", async () => {
                await api.saveSkill(skillDraft);
                setSkillDraft(null);
                setView(await api.book());
                setNotice("Skill saved to this book");
              })}>Save skill</button>
              <button type="button" onClick={() => setSkillDraft(null)}>Dismiss</button>
            </article>}
          </div>}
        </aside></>}
      </div>
    </div>
  );
}
