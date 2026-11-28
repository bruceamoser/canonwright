import { ArrowRight, BookOpen, Plus } from "lucide-react";
import type { WorkspaceSummary } from "../types";
import "./studio.css";

type Props = {
  projects: WorkspaceSummary[];
  onNewProject: () => void;
  onConnect: () => void;
  onOpen: (project: WorkspaceSummary) => void;
};

export default function MainScreen({ projects, onNewProject, onConnect, onOpen }: Props) {
  return (
    <div className="book-studio">
      <header className="studio-top">
        <span className="studio-brand"><BookOpen size={20} /> CanonWright</span>
        <span className="studio-book-title" />
        <button type="button" onClick={onConnect}>GitHub connection</button>
      </header>
      <main className="studio-library">
        <div className="studio-library-heading">
          <h1>Your books</h1>
          <button type="button" className="studio-primary" onClick={onNewProject}><Plus size={16} /> New book</button>
        </div>
        {projects.length === 0 && <p className="studio-muted">Start a book or import the manuscript you already have.</p>}
        <ul className="studio-library-grid">
          {projects.map((project) => (
            <li key={project.id}>
              <BookOpen size={24} />
              <h2>{project.name}</h2>
              <button type="button" onClick={() => onOpen(project)}>Open book <ArrowRight size={15} /></button>
            </li>
          ))}
        </ul>
      </main>
    </div>
  );
}
