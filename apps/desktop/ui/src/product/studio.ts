import { invoke } from "@tauri-apps/api/core";

export type Section = { original_pdf?: string; id: string; title: string };
export type SectionView = { text: string; revision: string };
export type TemplateField = { key: string; label: string; guidance: string; format: "text" | "list" | "links" };
export type CanonLink = { name: string; context: string; target_id?: string | null };
export type Finding = {
  fields: Record<string, string | (string | CanonLink)[]>;
  kind: "entry" | "note" | "question";
  area: string;
  title: string;
  detail: string;
  evidence: string;
};
export type BookView = {
  job_roles: Record<string,string>;
  book: {
    kind: "novel" | "rulebook" | "content_book";
    sections: Section[];
    review_jobs?: {author_decision?:string; review_decision?:string; reviewed_title?:string; attempts?:number; write_in_progress?:string; updated_at?:number; editorial?:{stage:"triage"|"repair"|"verify";issue?:string}; instructions?:string; sync?:{section:string}; work?:{role:string;request:string}|null; report?:string|null; id:string; state:string; result_id:string | null; error:string | null}[];
    records: { research_sources?: {source:string;basis:string}[]; author_sources?: {session:string;message:number;quote:string}[]; retained?: {key:string; label:string; value:Finding["fields"][string]}[]; unverified_sections?:string[]; id: string; section: string | null; pending: boolean; citations?: {section:string; evidence:string}[]; editorial?: {decision: "approve" | "revise" | "ask_author" | "duplicate" | "merge" | "source_conflict" | "referral_closed" | "manuscript_issue" | "issue_resolved"; reason: string}; finding: Finding; github_url: string | null; answer: string | null }[];
    repository: string | null;
  };
  areas: string[];
  templates: Record<string, { revision: number; fields: TemplateField[] }>;
  skills: { id: string; title: string }[];
};
const canonCollator = new Intl.Collator(undefined, { sensitivity: "base", numeric: true });
export function compareCanonTitles(a: { finding: Finding }, b: { finding: Finding }) {
  return canonCollator.compare(a.finding.title, b.finding.title);
}

export type SkillDraft = { title: string; instructions: string };
export type Manuscript = { path: string; text: string };
export type BookAction = {
  book_records_changed?: boolean;
  book_skill?: SkillDraft;
  book_changed?: boolean;
  book_built?: boolean;
  book_show?: string;
  book_reference?: { id: string; area: string; review?: boolean };
};
export type BookStudioHandle = {
  showAgents: () => void;
  prepare: () => Promise<{ projectId: string; sectionId: string | null; recordId: string | null; area: string | null; selection: string }>;
};

export function studioApi(projectId: string) {
  function call<T>(command: string, parameters: Record<string, unknown> = {}) {
    return invoke<T>(`studio_${command}`, { projectId, ...parameters });
  }
  return {
    retryFailedReviews: (ids: string[]) => call<number>("retry_failed_reviews", {ids}),
    attachPdf: (sectionId:string,path:string) => call<void>("attach_pdf",{sectionId,path}),
    book: () => call<BookView>("book"),
    section: (sectionId: string) => call<SectionView>("section", { sectionId }),
    save: (sectionId: string, revision: string, text: string) =>
      call<SectionView>("save_section", { sectionId, revision, text }),
    add: (title: string) => call<string>("add_section", { title }),
    import: (path: string) => call<number>("import", { path }),
    order: (sectionIds: string[]) => call<void>("order", { sectionIds }),
    build: () => call<Manuscript>("build"),
    compiled: () => call<Manuscript>("compiled"),
    record: (sectionId: string, revision: string, finding: Finding) =>
      call<void>("record", { sectionId, revision, finding }),
    decide: (recordId: string, accept: boolean) => call<void>("decide", { recordId, accept }),
    answer: (recordId: string, answer: string) => call<void>("answer", { recordId, answer }),
    saveSkill: (draft: SkillDraft) => call<void>("save_skill", { draft }),
    openIssue: (recordId: string) => call<string>("open_issue", { recordId }),
  };
}
