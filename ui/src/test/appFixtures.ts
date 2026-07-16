import { fireEvent, screen } from "@testing-library/react";
import type { ProjectWorkspace } from "../api/types.ts";

export async function selectProject(currentName: string, targetName: string): Promise<void> {
  fireEvent.click(screen.getByRole("button", { name: new RegExp(currentName) }));
  fireEvent.click(await screen.findByRole("button", { name: targetName }));
}

export function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

export function workspace(id: string, name: string, text: string): ProjectWorkspace {
  return {
    project: {
      id,
      name,
      revision: "1",
      created_at_epoch_ms: "0",
      updated_at_epoch_ms: "0",
    },
    current_workflow_summary: null,
    workflow_head: {
      project_id: id,
      revision: 1,
      workflow: {
        version: "1.0",
        project_id: id,
        nodes: [
          {
            id: `${id}-prompt`,
            type: "TextPrompt",
            params: { text },
            inputs: {},
            position: [100, 100],
          },
        ],
      },
    },
  };
}
