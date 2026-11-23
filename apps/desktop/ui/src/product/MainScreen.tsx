import { ArrowRight, FolderOpen, MessageSquarePlus, Plus } from "lucide-react";

import type { WorkspaceSummary } from "../types";

type Props = {
  projects: WorkspaceSummary[];
  onNewProject: () => void;
  onOpen: (project: WorkspaceSummary) => void;
};

/**
 * The home screen: projects on the left, the selected project's shape in the
 * middle. New project and New discussion are the only two entry points.
 */
export default function MainScreen({ projects, onNewProject, onOpen }: Props) {
  const first = projects[0] ?? null;

  return (
    <div className="flex h-full bg-obd-bg text-obd-text">
      <aside className="flex w-[300px] shrink-0 flex-col border-r border-obd-line bg-obd-surface">
        <div className="space-y-2 p-4">
          <button
            type="button"
            onClick={onNewProject}
            className="flex w-full items-center justify-center gap-2 rounded-lg bg-obd-accent px-3 py-2.5 text-[12px] font-semibold text-obd-on-accent"
          >
            <Plus size={14} />
            New project
          </button>
          <button
            type="button"
            onClick={() => {
              if (first !== null) {
                onOpen(first);
              }
            }}
            disabled={first === null}
            className="flex w-full items-center justify-center gap-2 rounded-lg border border-obd-line px-3 py-2.5 text-[12px] text-obd-body hover:bg-obd-raised disabled:opacity-50"
          >
            <MessageSquarePlus size={14} />
            New discussion
          </button>
        </div>

        <div className="px-4 pt-2 text-[10px] uppercase tracking-wide text-obd-muted">Projects</div>
        <div className="mt-1 flex-1 overflow-y-auto px-2">
          {projects.length === 0 ? (
            <p className="px-2 py-2 text-[11px] text-obd-subtle">No projects yet.</p>
          ) : (
            projects.map((project) => (
              <button
                key={project.id}
                type="button"
                onClick={() => onOpen(project)}
                className="mb-0.5 block w-full rounded-md px-2.5 py-2 text-left hover:bg-obd-raised"
              >
                <div className="truncate text-[12px] font-medium text-obd-text">{project.name}</div>
                <div className="truncate font-mono text-[10px] text-obd-muted">{project.path}</div>
              </button>
            ))
          )}
        </div>

        <div className="border-t border-obd-line p-4 text-[10px] text-obd-muted">
          <div>GitHub: connected</div>
        </div>
      </aside>

      <main className="flex-1 overflow-y-auto px-10 py-10">
        {first === null ? (
          <div className="max-w-xl">
            <h1 className="text-xl font-semibold tracking-tight">No projects yet</h1>
            <button
              type="button"
              onClick={onNewProject}
              className="mt-4 flex items-center gap-2 rounded-lg bg-obd-accent px-4 py-2.5 text-[12px] font-semibold text-obd-on-accent"
            >
              New project
              <ArrowRight size={14} />
            </button>
          </div>
        ) : (
          <>
            <h1 className="text-xl font-semibold tracking-tight">{first.name}</h1>
            <div className="mt-1 font-mono text-[11px] text-obd-muted">{first.path}</div>

            <div className="mt-6 max-w-xl rounded-xl border border-obd-line bg-obd-surface p-5">
              <div className="flex items-center gap-2 text-[12px] text-obd-body">
                <FolderOpen size={14} className="text-obd-muted" />
                Open it to see the manuscript, the canon and the discussion.
              </div>
              <button
                type="button"
                onClick={() => onOpen(first)}
                className="mt-4 flex items-center gap-2 rounded-lg bg-obd-accent px-4 py-2.5 text-[12px] font-semibold text-obd-on-accent"
              >
                Open project
                <ArrowRight size={14} />
              </button>
            </div>
          </>
        )}
      </main>
    </div>
  );
}
