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
}: {
  current: Project | null;
  open: boolean;
  onClose: () => void;
  onOpenProject: (id: string) => void;
}) {
  const [projects, setProjects] = useState<Project[]>([]);
  const [name, setName] = useState("");
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open) {
      api.listProjects().then(setProjects).catch(() => setProjects([]));
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
      onOpenProject(p.id);
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
          value={name}
          placeholder="New project…"
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && create()}
        />
        <button className="psw__add" onClick={create}>Create</button>
      </div>
    </div>
  );
}
