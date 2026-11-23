import { useCallback, useEffect, useState } from "react";
import {
  ArrowDownToLine,
  ArrowUpFromLine,
  Check,
  CircleAlert,
  GitBranch as GitBranchIcon,
  Loader2,
  Plus,
  RefreshCw,
  Tag,
} from "lucide-react";

import {
  gitCommit,
  gitFetch,
  gitInit,
  gitLog,
  gitPull,
  gitPush,
  gitRepoStatus,
  gitStage,
  gitTag,
  gitUnstage,
} from "./backend";
import type { GitCommit as GitCommitRecord, GitRepoStatus, WorkspaceSummary } from "./types";

type Props = {
  project: WorkspaceSummary;
};

/**
 * Source control for the manuscript, in the workspace pane.
 *
 * The same operations the assistant has as tools, with a human deciding when to
 * press them: stage, commit, push, pull, fetch, tag. Nothing here is novel, and
 * nothing here is a git porcelain the writer does not need.
 */
export default function SourceControlPanel({ project }: Props) {
  const [status, setStatus] = useState<GitRepoStatus | null>(null);
  const [history, setHistory] = useState<GitCommitRecord[]>([]);
  const [message, setMessage] = useState("");
  const [tagName, setTagName] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const next = await gitRepoStatus(project.path, project.path);
      setStatus(next);
      if (next.isRepo && next.hasCommits) {
        setHistory(await gitLog(project.path, project.path, 8));
      } else {
        setHistory([]);
      }
      setError(next.error ?? null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }, [project.path]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = useCallback(
    async (label: string, action: () => Promise<unknown>) => {
      setBusy(label);
      setError(null);
      setNotice(null);
      try {
        await action();
        await refresh();
      } catch (cause) {
        setError(cause instanceof Error ? cause.message : String(cause));
      } finally {
        setBusy(null);
      }
    },
    [refresh],
  );

  const staged = status?.files.filter((file) => file.staged) ?? [];
  const changed = status?.files.filter((file) => !file.staged) ?? [];

  if (status && !status.isRepo) {
    return (
      <div className="space-y-3 p-4">
        <p className="text-[11px] leading-5 text-obd-muted">
          This project is not a git repository yet. A manuscript kept in git has a
          history, a diff, and a copy on GitHub.
        </p>
        <button
          type="button"
          onClick={() => void run("init", () => gitInit(project.path, project.path))}
          disabled={busy !== null}
          className="rounded-md bg-obd-accent px-3 py-1.5 text-[11px] font-medium text-obd-on-accent disabled:opacity-40"
        >
          {busy === "init" ? "Initialising…" : "Initialise repository"}
        </button>
        {error && <p className="text-[11px] text-obd-danger">{error}</p>}
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-2 border-b border-obd-line px-3 py-2">
        <GitBranchIcon size={13} className="text-obd-subtle" />
        <span className="truncate text-[11px] font-medium text-obd-text">
          {status?.branch ?? "…"}
        </span>
        {status?.ahead ? (
          <span className="text-[10px] text-obd-subtle">↑{status.ahead}</span>
        ) : null}
        {status?.behind ? (
          <span className="text-[10px] text-obd-subtle">↓{status.behind}</span>
        ) : null}
        <div className="ml-auto flex items-center gap-1">
          <IconButton
            label="Fetch"
            busy={busy === "fetch"}
            onClick={() => void run("fetch", () => gitFetch(project.path, project.path))}
          >
            <RefreshCw size={12} />
          </IconButton>
          <IconButton
            label="Pull"
            busy={busy === "pull"}
            onClick={() => void run("pull", () => gitPull(project.path, project.path, "ff-only"))}
          >
            <ArrowDownToLine size={12} />
          </IconButton>
          <IconButton
            label="Push"
            busy={busy === "push"}
            onClick={() => void run("push", () => gitPush(project.path, project.path, {}))}
          >
            <ArrowUpFromLine size={12} />
          </IconButton>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        <Section
          title={`Changes (${changed.length})`}
          action={
            changed.length > 0 ? (
              <button
                type="button"
                onClick={() =>
                  void run("stage-all", () =>
                    gitStage(
                      project.path,
                      project.path,
                      changed.map((file) => file.path),
                    ),
                  )
                }
                className="flex items-center gap-1 text-[10px] text-obd-muted hover:text-obd-text"
              >
                <Plus size={11} /> all
              </button>
            ) : null
          }
        >
          {changed.length === 0 ? (
            <p className="px-3 py-2 text-[10px] text-obd-subtle">Nothing modified.</p>
          ) : (
            changed.map((file) => (
              <FileRow
                key={file.path}
                path={file.path}
                badge={file.untracked ? "U" : file.worktreeStatus}
                onStage={() =>
                  void run(file.path, () => gitStage(project.path, project.path, [file.path]))
                }
              />
            ))
          )}
        </Section>

        <Section title={`Staged (${staged.length})`}>
          {staged.length === 0 ? (
            <p className="px-3 py-2 text-[10px] text-obd-subtle">Nothing staged.</p>
          ) : (
            staged.map((file) => (
              <FileRow
                key={file.path}
                path={file.path}
                badge={file.indexStatus}
                staged
                onStage={() =>
                  void run(file.path, () => gitUnstage(project.path, project.path, [file.path]))
                }
              />
            ))
          )}
        </Section>

        <div className="space-y-2 border-t border-obd-line p-3">
          <textarea
            value={message}
            onChange={(event) => setMessage(event.target.value)}
            placeholder="What changed?"
            rows={2}
            className="w-full resize-none rounded-md border border-obd-line bg-obd-inset px-2 py-1.5 text-[11px] text-obd-text outline-none focus:border-obd-accent/40"
          />
          <div className="flex items-center gap-2">
            <button
              type="button"
              disabled={message.trim().length === 0 || busy !== null}
              onClick={() =>
                void run("commit", async () => {
                  await gitCommit(
                    project.path,
                    project.path,
                    message.trim(),
                    staged.length === 0,
                  );
                  setMessage("");
                  setNotice("Committed.");
                })
              }
              className="rounded-md bg-obd-accent px-3 py-1.5 text-[11px] font-medium text-obd-on-accent disabled:opacity-30"
            >
              {busy === "commit" ? "Committing…" : "Commit"}
            </button>
            <span className="text-[10px] text-obd-subtle">
              {staged.length === 0 ? "stages everything" : `${staged.length} staged`}
            </span>
          </div>
        </div>

        <Section title="Branches">
          {(status?.branches ?? []).filter((branch) => !branch.remote).length === 0 ? (
            <p className="px-3 py-2 text-[10px] text-obd-subtle">No branches yet.</p>
          ) : (
            (status?.branches ?? [])
              .filter((branch) => !branch.remote)
              .map((branch) => (
                <div key={branch.name} className="flex items-center gap-2 px-3 py-1 text-[11px]">
                  {branch.current ? (
                    <Check size={11} className="text-obd-accent" />
                  ) : (
                    <GitBranchIcon size={11} className="text-obd-subtle" />
                  )}
                  <span className={branch.current ? "text-obd-text" : "text-obd-muted"}>
                    {branch.name}
                  </span>
                </div>
              ))
          )}
        </Section>

        <Section title="Tag a draft">
          <div className="flex items-center gap-1 px-3 py-1.5">
            <input
              value={tagName}
              onChange={(event) => setTagName(event.target.value)}
              placeholder="draft-2"
              className="min-w-0 flex-1 rounded-md border border-obd-line bg-obd-inset px-2 py-1 text-[10px] text-obd-text outline-none"
            />
            <IconButton
              label="Create and push the tag"
              busy={busy === "tag"}
              disabled={tagName.trim().length === 0}
              onClick={() =>
                void run("tag", async () => {
                  await gitTag(project.path, project.path, tagName.trim(), null, true);
                  setTagName("");
                  setNotice(`Tagged ${tagName.trim()}.`);
                })
              }
            >
              <Tag size={12} />
            </IconButton>
          </div>
        </Section>

        <Section title="Recent commits">
          {history.length === 0 ? (
            <p className="px-3 py-2 text-[10px] text-obd-subtle">No commits yet.</p>
          ) : (
            history.map((commit) => (
              <div key={commit.hash} className="px-3 py-1 text-[10px]">
                <span className="font-mono text-obd-subtle">{commit.shortHash}</span>{" "}
                <span className="text-obd-muted">{commit.subject}</span>
              </div>
            ))
          )}
        </Section>
      </div>

      {(error || notice) && (
        <div className="border-t border-obd-line px-3 py-2">
          {error ? (
            <p className="flex items-start gap-1 text-[10px] leading-4 text-obd-danger">
              <CircleAlert size={11} className="mt-0.5 shrink-0" /> {error}
            </p>
          ) : (
            <p className="text-[10px] text-obd-subtle">{notice}</p>
          )}
        </div>
      )}
    </div>
  );
}

function Section({
  title,
  action,
  children,
}: {
  title: string;
  action?: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="border-b border-obd-line/60 py-1">
      <div className="flex items-center justify-between px-3 py-1">
        <p className="text-[9px] uppercase tracking-wider text-obd-subtle">{title}</p>
        {action}
      </div>
      {children}
    </div>
  );
}

function FileRow({
  path,
  badge,
  staged,
  onStage,
}: {
  path: string;
  badge: string;
  staged?: boolean;
  onStage: () => void;
}) {
  return (
    <div className="group flex items-center gap-2 px-3 py-0.5 text-[11px]">
      <span className="w-3 font-mono text-[9px] text-obd-subtle">{badge || " "}</span>
      <span className="min-w-0 flex-1 truncate text-obd-muted" title={path}>
        {path}
      </span>
      <button
        type="button"
        onClick={onStage}
        className="opacity-0 transition-opacity group-hover:opacity-100"
        aria-label={staged ? `Unstage ${path}` : `Stage ${path}`}
      >
        {staged ? (
          <ArrowDownToLine size={11} className="text-obd-subtle" />
        ) : (
          <Plus size={11} className="text-obd-subtle" />
        )}
      </button>
    </div>
  );
}

function IconButton({
  label,
  busy,
  disabled,
  onClick,
  children,
}: {
  label: string;
  busy?: boolean;
  disabled?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      onClick={onClick}
      disabled={busy || disabled}
      className="rounded p-1 text-obd-subtle hover:bg-obd-raised hover:text-obd-body disabled:opacity-40"
    >
      {busy ? <Loader2 size={12} className="animate-spin" /> : children}
    </button>
  );
}
