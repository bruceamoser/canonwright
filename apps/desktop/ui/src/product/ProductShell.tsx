import { useState } from "react";

import type { WorkspaceSummary } from "../types";
import MainScreen from "./MainScreen";
import NewProjectScreen from "./NewProjectScreen";
import SetupScreen from "./SetupScreen";

type Props = {
  projects: WorkspaceSummary[];
  onOpen: (project: WorkspaceSummary) => void;
  onProjectsChanged: () => void;
};

export default function ProductShell({ projects, onOpen, onProjectsChanged }: Props) {
  const [stage, setStage] = useState<"library" | "new-book" | "github">("library");
  const library = () => setStage("library");

  if (stage === "github") {
    return <SetupScreen onDone={library} onSkip={library} />;
  }
  if (stage === "new-book") {
    return <NewProjectScreen onCancel={library} onCreated={(project) => {
      onProjectsChanged();
      onOpen(project);
    }} />;
  }
  return <MainScreen projects={projects} onNewProject={() => setStage("new-book")}
    onConnect={() => setStage("github")} onOpen={onOpen} />;
}
