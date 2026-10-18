import type { Progress } from "../lib/types";

export function ProgressToast({ progress }: { progress: Progress }) {
  const pct = progress.total
    ? Math.round((progress.done / progress.total) * 100)
    : 0;
  return (
    <div className="toast">
      <div className="toast__head">
        <span>Deleting {progress.path.split("/").pop() ?? progress.path}</span>
        <span className="dim">
          {progress.done} / {progress.total}
        </span>
      </div>
      <div className="progress-track">
        <div className="progress-track__fill" style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}

export function AboutDialog({ onClose }: { onClose: () => void }) {
  return (
    <div className="scrim" onClick={onClose}>
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label="About PerfectUninstaller"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="modal__body">
          <div className="row" style={{ gap: 12 }}>
            <div
              className="brand-mark"
              style={{ width: 40, height: 40, borderRadius: 12, fontSize: 15 }}
            >
              ◆
            </div>
            <div className="col">
              <span className="modal__title" style={{ fontSize: 17 }}>
                PerfectUninstaller
              </span>
              <span className="tiny dim">Complete software removal</span>
            </div>
          </div>
          <p className="modal__text">
            Permanently removes an app together with its caches, preferences,
            containers, launch agents, receipts, crash reports and other
            residual traces — with a full audit log and an exportable report.
          </p>
          <div className="mt-16 col" style={{ gap: 7 }}>
            <div className="row tiny">
              <span className="dim" style={{ width: 74 }}>
                Created by
              </span>
              <strong>Enkivus</strong>
            </div>
            <div className="row tiny">
              <span className="dim" style={{ width: 74 }}>
                Source
              </span>
              <span className="mono">
                github.com/Enkivus/PerfectUninstaller
              </span>
            </div>
            <div className="row tiny">
              <span className="dim" style={{ width: 74 }}>
                Shortcuts
              </span>
              <span className="dim">⌘R rescan · Esc back</span>
            </div>
          </div>
        </div>
        <div className="modal__foot">
          <div className="spacer" />
          <button className="btn btn--primary" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}