import { useEffect, useMemo, useState } from "react";
import { ArrowLeft, Check, Loader2 } from "lucide-react";

import { githubRepos } from "../backend";

type Props = {
  onBack: () => void;
  onPick: (repo: string) => void;
};

/** The repository that already holds the manuscript. */
export default function RepoPicker({ onBack, onPick }: Props) {
  const [repos, setRepos] = useState<string[] | null>(null);
  const [filter, setFilter] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    void githubRepos()
      .then((next) => {
        if (live) {
          setRepos(next);
          setSelected(next[0] ?? null);
        }
      })
      .catch((cause: unknown) => {
        if (live) {
          setError(cause instanceof Error ? cause.message : String(cause));
          setRepos([]);
        }
      });
    return () => {
      live = false;
    };
  }, []);

  const shown = useMemo(() => {
    const term = filter.trim().toLowerCase();
    if (term.length === 0) {
      return repos ?? [];
    }
    return (repos ?? []).filter((repo) => repo.toLowerCase().includes(term));
  }, [repos, filter]);

  return (
    <div className="h-full overflow-y-auto bg-obd-bg px-10 py-10 text-obd-text">
      <div className="max-w-2xl">
        <h1 className="text-xl font-semibold tracking-tight">Connect a repository</h1>

        <input
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
          placeholder="Filter repositories"
          className="mt-6 w-full rounded-lg border border-obd-line bg-obd-surface px-3 py-2 text-[12px] text-obd-text outline-none focus:border-obd-accent"
        />

        <div className="mt-3 space-y-1.5">
          {repos === null && (
            <div className="flex items-center gap-2 px-1 py-3 text-[11px] text-obd-muted">
              <Loader2 className="animate-spin" size={13} />
              Loading repositories…
            </div>
          )}
          {repos !== null && shown.length === 0 && (
            <p className="px-1 py-3 text-[11px] text-obd-subtle">No repositories match.</p>
          )}
          {shown.map((repo) => (
            <button
              key={repo}
              type="button"
              onClick={() => setSelected(repo)}
              className={`flex w-full items-center gap-3 rounded-lg border px-3 py-2.5 text-left font-mono text-[12px] ${
                selected === repo
                  ? "border-obd-accent bg-obd-raised text-obd-text"
                  : "border-obd-line bg-obd-surface text-obd-body hover:bg-obd-raised"
              }`}
            >
              <span className="flex-1 truncate">{repo}</span>
              {selected === repo && <Check size={13} className="text-obd-accent" />}
            </button>
          ))}
        </div>

        {error !== null && (
          <p className="mt-4 rounded-lg border border-obd-danger/40 bg-obd-raised px-3 py-2 text-[11px] text-obd-danger">
            {error}
          </p>
        )}

        <div className="mt-8 flex items-center gap-3">
          <button
            type="button"
            onClick={onBack}
            className="flex items-center gap-2 rounded-lg border border-obd-line px-4 py-2 text-[11px] text-obd-muted hover:bg-obd-raised"
          >
            <ArrowLeft size={13} />
            Back
          </button>
          <button
            type="button"
            disabled={selected === null}
            onClick={() => {
              if (selected !== null) {
                onPick(selected);
              }
            }}
            className="rounded-lg bg-obd-accent px-4 py-2 text-[12px] font-semibold text-obd-on-accent disabled:opacity-50"
          >
            Connect repository
          </button>
        </div>
      </div>
    </div>
  );
}
