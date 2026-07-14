import { useEffect, useRef } from "react";

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
        <p>{String(error)}</p>
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
