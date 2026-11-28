import { useState } from "react";
import { FileText, FolderOpen, GitBranch, Loader2, PenLine } from "lucide-react";

import { createProject, pickDirectory, pickManuscript } from "../backend";
import type { StoryStartRequest, WorkspaceSummary } from "../types";
import RepoPicker from "./RepoPicker";

type Kind = "narrative" | "rules" | "hybrid";
const KINDS = { narrative: "Novel", rules: "Core rulebook", hybrid: "TTRPG content book" };

type Props = {
  onCancel: () => void;
  onCreated: (project: WorkspaceSummary) => void;
};

function baseName(path: string): string {
  const trimmed = path.replace(/[\\/]+$/, "");
  const parts = trimmed.split(/[\\/]/);
  return parts[parts.length - 1] ?? "Untitled";
}

/**
 * Creating a project configures it and nothing else.
 *
 * Each button is the action: Start blank, Open a file…, Open a folder…,
 * Connect a repository… There is no separate Create step to confirm.
 */
export default function NewProjectScreen({ onCancel, onCreated }: Props) {
  const [name, setName] = useState("");
  const [kind, setKind] = useState<Kind>("narrative");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pickingRepo, setPickingRepo] = useState(false);

  async function create(title: string, story: StoryStartRequest) {
    const finalName = title.trim().length > 0 ? title.trim() : "Untitled";
    setBusy(story.mode);
    setError(null);
    try {
      const project = await createProject(finalName, { ...story, studio: true });
      onCreated(project);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(null);
    }
  }

  async function openFile() {
    try {
      const path = await pickManuscript();
      if (path === null) {
        return;
      }
      await create(name.length > 0 ? name : baseName(path), {
        mode: "existing",
        kind,
        manuscript: path,
        answers: {},
      });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function openFolder() {
    try {
      const path = await pickDirectory("Choose a folder");
      if (path === null) {
        return;
      }
      await create(name.length > 0 ? name : baseName(path), {
        mode: "existing",
        kind,
        manuscript: path,
        answers: {},
      });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  if (pickingRepo) {
    return (
      <RepoPicker
        onBack={() => setPickingRepo(false)}
        onPick={(repo) => {
          setPickingRepo(false);
          void create(name.length > 0 ? name : baseName(repo), {
            mode: "github",
            kind,
            repo,
            answers: {},
          });
        }}
      />
    );
  }

  const actions = [
    {
      key: "new",
      label: "Start blank",
      hint: "No manuscript yet",
      icon: <PenLine size={16} />,
      run: () => void create(name, { mode: "new", kind, answers: {} }),
    },
    {
      key: "file",
      label: "Open a file…",
      hint: "docx, md, epub, pdf",
      icon: <FileText size={16} />,
      run: () => void openFile(),
    },
    {
      key: "folder",
      label: "Open a folder…",
      hint: "A directory of drafts",
      icon: <FolderOpen size={16} />,
      run: () => void openFolder(),
    },
    {
      key: "github",
      label: "Connect a repository…",
      hint: "An existing repository",
      icon: <GitBranch size={16} />,
      run: () => setPickingRepo(true),
    },
  ];

  return (
    <div className="h-full overflow-y-auto bg-obd-bg px-10 py-10 text-obd-text">
      <div className="max-w-2xl">
        <h1 className="text-xl font-semibold tracking-tight">New book</h1>

        <div className="mt-7 text-[10px] uppercase tracking-wide text-obd-muted">Name</div>
        <input
          value={name}
          onChange={(event) => setName(event.target.value)}
          placeholder="The Salt Road"
          className="mt-1.5 w-full max-w-sm rounded-lg border border-obd-line bg-obd-surface px-3 py-2 text-[13px] text-obd-text outline-none focus:border-obd-accent"
        />

        <div className="mt-6 text-[10px] uppercase tracking-wide text-obd-muted">Type</div>
        <div className="mt-1.5 flex gap-2">
          {(Object.keys(KINDS) as Kind[]).map((option) => (
            <button
              key={option}
              type="button"
              onClick={() => setKind(option)}
              className={`rounded-full border px-3 py-1 text-[11px] ${
                kind === option
                  ? "border-obd-accent bg-obd-raised text-obd-accent"
                  : "border-obd-line text-obd-muted hover:bg-obd-raised"
              }`}
            >
              {KINDS[option]}
            </button>
          ))}
        </div>

        <div className="mt-7 text-[10px] uppercase tracking-wide text-obd-muted">Start from</div>
        <div className="mt-2 space-y-2">
          {actions.map((action) => (
            <button
              key={action.key}
              type="button"
              onClick={action.run}
              disabled={busy !== null}
              className="flex w-full max-w-xl items-center gap-3 rounded-xl border border-obd-line bg-obd-surface px-4 py-3 text-left hover:border-obd-accent disabled:opacity-60"
            >
              <span className="text-obd-muted">{action.icon}</span>
              <span className="flex-1">
                <span className="block text-[13px] font-medium text-obd-text">{action.label}</span>
                <span className="block text-[10px] text-obd-muted">{action.hint}</span>
              </span>
              {busy === action.key && <Loader2 className="animate-spin text-obd-muted" size={14} />}
            </button>
          ))}
        </div>

        {error !== null && (
          <p className="mt-4 max-w-xl rounded-lg border border-obd-danger/40 bg-obd-raised px-3 py-2 text-[11px] text-obd-danger">
            {error}
          </p>
        )}

        <button
          type="button"
          onClick={onCancel}
          className="mt-8 rounded-lg border border-obd-line px-4 py-2 text-[11px] text-obd-muted hover:bg-obd-raised"
        >
          Cancel
        </button>
      </div>
    </div>
  );
}
