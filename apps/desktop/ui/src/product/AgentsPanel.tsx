import { useEffect, useRef, useState } from "react";
import { ArrowLeft, ChevronRight } from "lucide-react";
import type { BookView } from "./studio";

const roles: Record<string, string> = {
  writer: "Writer", canon_keeper: "Canon Keeper", canon_reviewer: "Canon Reviewer",
  editorial_reviewer: "Editorial Reviewer", process_manager: "Process Manager",
};
type Job = NonNullable<BookView["book"]["review_jobs"]>[number];
const active = (job: Job) => job.state === "running" || job.state === "cancelling";
const attention = (job: Job) => job.state === "failed" || job.state === "waiting_for_author";
const stateLabel = (job: Job) => job.state === "ready" ? "Queued" : job.state.replaceAll("_", " ").replace(/^./, c => c.toUpperCase());

function shortTitle(text: string) {
  const first = text.trim().split(/\n|(?<=[.!?])\s+/u, 1)[0];
  return first.length > 105 ? first.slice(0, 102).replace(/\s+\S*$/, "") + "…" : first || "Assignment";
}

export default function AgentsPanel({ view }: { view: BookView }) {
  const [selected, select] = useState<string | null>(null);
  const [historyOpen, setHistoryOpen] = useState(false);
  const back = useRef<HTMLButtonElement>(null);
  const lastRow = useRef<string | null>(null);
  const jobs = [...(view.book.review_jobs ?? [])].sort((a,b) => (b.updated_at ?? 0) - (a.updated_at ?? 0));
  const running = jobs.filter(active);
  const queued = jobs.filter(j => j.state === "ready").reverse();
  const blocked = jobs.filter(attention);
  const history = jobs.filter(j => !active(j) && j.state !== "ready" && !attention(j));
  const job = jobs.find(j => j.id === selected);
  useEffect(() => {
    if (selected) back.current?.focus();
    else if (lastRow.current) document.getElementById(lastRow.current)?.focus();
  }, [selected]);

  function assignment(j: Job) {
    return j.work?.request || j.reviewed_title || view.book.records.find(r => r.id === j.id || r.id === j.result_id)?.finding.title
      || view.book.sections.find(s => s.id === j.sync?.section)?.title || j.instructions || "Book assignment";
  }
  function role(j: Job) { return roles[view.job_roles[j.id]] ?? "Book agent"; }
  function row(j: Job) {
    const title = shortTitle(assignment(j));
    return <button key={j.id} id={'agent-job-' + j.id} className="studio-agent-row" onClick={() => {
      lastRow.current = 'agent-job-' + j.id;
      select(j.id);
    }}>
      <span className="studio-agent-copy"><strong>{title}</strong><small>{role(j)} · {stateLabel(j)}</small></span>
      <ChevronRight size={13} aria-hidden="true" />
    </button>;
  }

  if (job) return <div className="studio-detail-body studio-agents studio-agent-inspector">
    <button ref={back} onClick={() => { if (history.includes(job)) setHistoryOpen(true); select(null); }}><ArrowLeft size={13}/> All agents</button>
    <h3>{shortTitle(assignment(job))}</h3>
    <p>{role(job)} · {stateLabel(job)}</p>
    {job.write_in_progress && <p>Current tool · {job.write_in_progress}</p>}
    {job.error && <section><h4>Error</h4><p>{job.error}</p></section>}
    {job.report && <section><h4>Report</h4><p>{job.report}</p></section>}
    <details><summary>Full assignment</summary><p>{job.work?.request || job.instructions || assignment(job)}</p></details>
    <small>{job.updated_at ? new Date(job.updated_at).toLocaleString() + ' · ' : ''}{job.id}</small>
  </div>;

  return <div className="studio-detail-body studio-agents">
    <p className="studio-agent-counts" role="status">{running.length} running · {queued.length} queued{blocked.length > 0 && ' · ' + blocked.length + ' need attention'}</p>
    <ul className="studio-agent-roster">{Object.entries(roles).map(([key, label]) => {
      const current = running.find(j => view.job_roles[j.id] === key);
      const count = queued.filter(j => view.job_roles[j.id] === key).length;
      const state = current ? stateLabel(current) : count ? "Queued" : "Idle";
      return <li key={key}><strong>{label}</strong><span data-busy={Boolean(current)}>{state}{count > 0 && (current ? ' · ' + count + ' queued' : ' (' + count + ')')}</span></li>;
    })}</ul>
    {running.length + queued.length > 0 && <section><h4>Work queue</h4>{running.map(row)}{queued.map(row)}</section>}
    {blocked.length > 0 && <section><h4>Needs attention</h4>{blocked.map(row)}</section>}
    {history.length > 0 && <details open={historyOpen} onToggle={event => setHistoryOpen(event.currentTarget.open)}>
      <summary>History ({history.length})</summary>{history.map(row)}
    </details>}
  </div>;
}
