// Project switcher popover — lists projects and creates a new one. Opened from
// the top bar's project button.

import { useEffect, useRef, useState } from "react";
import { api, type Project } from "../api/index.ts";
import "./projectSwitcher.css";

export function ProjectSwitcher({
  current,
  open,
  onClose,
  onOpenProject,
  onProjectRenamed,
}: {
  current: Project | null;
  open: boolean;
  onClose: () => void;
  onOpenProject: (id: string) => void;
  onProjectRenamed: (project: Project) => void;
}) {
  const [projects, setProjects] = useState<Project[]>([]);
  const [name, setName] = useState("");
  const [renameName, setRenameName] = useState("");
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open) {
      void refreshProjects(setProjects);
    }
  }, [open]);

  useEffect(() => {
    if (!open) {
      return;
    }
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [open, onClose]);

  if (!open) {
    return null;
  }

  const create = () => {
    const trimmed = name.trim();
    if (!trimmed) {
      return;
    }
    api.createProject(trimmed).then((p) => {
      setName("");
      setProjects((currentProjects) => deduplicateProjects([p, ...currentProjects]));
      onOpenProject(p.id);
    });
  };

  const rename = () => {
    const trimmed = renameName.trim();
    if (!current || !trimmed || trimmed === current.name) return;
    void api.renameProject(current, trimmed).then(async (renamed) => {
      setRenameName("");
      onProjectRenamed(renamed);
      setProjects(await api.listProjects().then(deduplicateProjects));
    });
  };

  return (
    <div className="psw" ref={ref}>
      <div className="psw__head">Projects</div>
      <div className="psw__list">
        {projects.map((p) => (
          <button
            key={p.id}
            className={`psw__item${p.id === current?.id ? " is-on" : ""}`}
            onClick={() => onOpenProject(p.id)}
          >
            <span className="psw__dot" />
            {p.name}
          </button>
        ))}
      </div>
      <div className="psw__new">
        <input
          aria-label="New project name"
          name="project-name"
          value={name}
          placeholder="New project…"
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && create()}
        />
        <button className="psw__add" onClick={create}>Create</button>
      </div>
      {current ? (
        <div className="psw__new">
          <input
            aria-label="Rename current project"
            name="project-rename"
            value={renameName}
            placeholder={`Rename ${current.name}…`}
            onChange={(event) => setRenameName(event.target.value)}
            onKeyDown={(event) => event.key === "Enter" && rename()}
          />
          <button className="psw__add" onClick={rename}>Rename</button>
        </div>
      ) : null}
    </div>
  );
}

async function refreshProjects(setProjects: (projects: Project[]) => void) {
  try {
    setProjects(deduplicateProjects(await api.listProjects()));
  } catch {
    setProjects([]);
  }
}

function deduplicateProjects(projects: Project[]): Project[] {
  return [...new Map(projects.map((project) => [project.id, project])).values()];
}
