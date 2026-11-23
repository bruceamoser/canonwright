import { useCallback, useEffect, useState } from "react";

import { githubStatus } from "../backend";
import type { WorkspaceSummary } from "../types";
import MainScreen from "./MainScreen";
import NewProjectScreen from "./NewProjectScreen";
import SetupScreen from "./SetupScreen";

type Stage = "checking" | "setup" | "main" | "new-project";

const SKIP_KEY = "canonwright.setup.dismissed";

type Props = {
  projects: WorkspaceSummary[];
  onOpen: (project: WorkspaceSummary) => void;
  onProjectsChanged: () => void;
};

/**
 * Everything before a project is open: sign-in, the home screen, and creating a
 * project. Once a project is chosen the app hands over to the project views.
 */
export default function ProductShell({ projects, onOpen, onProjectsChanged }: Props) {
  const [stage, setStage] = useState<Stage>("checking");

  useEffect(() => {
    let live = true;
    const dismissed = window.localStorage.getItem(SKIP_KEY) === "1";
    void githubStatus()
      .then((status) => {
        if (!live) {
          return;
        }
        setStage(status.authenticated || dismissed ? "main" : "setup");
      })
      .catch(() => {
        if (live) {
          setStage(dismissed ? "main" : "setup");
        }
      });
    return () => {
      live = false;
    };
  }, []);

  const finishSetup = useCallback(() => {
    window.localStorage.setItem(SKIP_KEY, "1");
    setStage("main");
  }, []);

  const skipSetup = useCallback(() => {
    window.localStorage.setItem(SKIP_KEY, "1");
    setStage("main");
  }, []);

  if (stage === "checking") {
    return (
      <div className="flex h-full items-center justify-center bg-obd-bg text-[11px] text-obd-muted">
        Checking GitHub…
      </div>
    );
  }

  if (stage === "setup") {
    return <SetupScreen onDone={finishSetup} onSkip={skipSetup} />;
  }

  if (stage === "new-project") {
    return (
      <NewProjectScreen
        onCancel={() => setStage("main")}
        onCreated={(project) => {
          onProjectsChanged();
          onOpen(project);
        }}
      />
    );
  }

  return (
    <MainScreen
      projects={projects}
      onNewProject={() => setStage("new-project")}
      onOpen={onOpen}
    />
  );
}
