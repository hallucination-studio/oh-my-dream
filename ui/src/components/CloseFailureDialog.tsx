import { useEffect, useRef } from "react";
import { failureCopy } from "../workflow/failureCopy.ts";

export function CloseFailureDialog({
  error,
  onKeepEditing,
  onDiscardAndClose,
}: {
  error: unknown;
  onKeepEditing: () => void;
  onDiscardAndClose: () => void;
}) {
  const keepButton = useRef<HTMLButtonElement>(null);
  useEffect(() => keepButton.current?.focus(), []);
  return (
    <div className="scrim" role="presentation">
      <section className="close-dialog" role="dialog" aria-modal="true" aria-labelledby="close-dialog-title">
        <h2 id="close-dialog-title">Changes could not be saved</h2>
        <p>{failureCopy("Save the workflow", error)}</p>
        <p>Keep editing to try saving again, or discard the unsaved changes and close.</p>
        <details>
          <summary>Diagnostics</summary>
          <code>{String(error)}</code>
        </details>
        <div className="close-dialog__actions">
          <button ref={keepButton} onClick={onKeepEditing}>Keep Editing</button>
          <button className="close-dialog__discard" onClick={onDiscardAndClose}>
            Discard and Close
          </button>
        </div>
      </section>
    </div>
  );
}
