import { useState } from "react";
import type { RemovalPlan, RemovalReport } from "../lib/types";
import { formatBytes } from "../lib/types";
import { exportReport, openPath } from "../lib/api";

export function ReportView({
  report,
  plan,
  onBack,
}: {
  report: RemovalReport;
  plan: RemovalPlan | null;
  onBack: () => void;
}) {
  // The automatic export can fail (read-only home, disk full). Keeping the plan
  // around lets the user retry it here rather than re-running the removal.
  const [dir, setDir] = useState(report.report_dir);
  const [retrying, setRetrying] = useState(false);
  const [retryError, setRetryError] = useState<string | null>(null);

  const reExport = async () => {
    if (!plan) return;
    setRetrying(true);
    setRetryError(null);
    try {
      setDir(await exportReport(plan, report));
    } catch (e) {
      setRetryError(String(e));
    } finally {
      setRetrying(false);
    }
  };

  return (
    <>
      <div className="view-head">
        <button className="btn btn--icon" onClick={onBack} title="Back">
          ‹
        </button>
        <div className="success-mark">
          <svg viewBox="0 0 24 24" fill="none" style={{ width: 22, height: 22 }}>
            <path
              d="M4 12.5 L9.5 18 L20 6.5"
              stroke="currentColor"
              strokeWidth="2.4"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </div>
        <div className="col">
          <span className="view-head__title">Uninstall complete</span>
          <span className="tiny dim">
            The selected items were permanently deleted.
          </span>
        </div>
      </div>

      <div className="stats">
        <div className="stat">
          <div className="stat__label">Items removed</div>
          <div className="stat__value" style={{ color: "var(--ok)" }}>
            {report.removed.length}
          </div>
        </div>
        <div className="stat">
          <div className="stat__label">Space freed</div>
          <div className="stat__value" style={{ color: "var(--ok)" }}>
            {formatBytes(report.bytes_freed)}
          </div>
        </div>
        <div className="stat">
          <div className="stat__label">Duration</div>
          <div className="stat__value">{report.elapsed_ms} ms</div>
        </div>
        <div className="stat">
          <div className="stat__label">Problems</div>
          <div
            className="stat__value"
            style={{ color: report.failed.length ? "var(--danger)" : "var(--ok)" }}
          >
            {report.failed.length}
          </div>
        </div>
      </div>

      {report.refused.length > 0 && (
        <div className="notice notice--warn">
          <span className="notice__icon">⚠</span>
          <div>
            {report.refused.length} path
            {report.refused.length === 1 ? "" : "s"} refused by the safety
            guards and left untouched.
            <div className="advisory__path mt-8">
              {report.refused.slice(0, 8).join("\n")}
            </div>
          </div>
        </div>
      )}

      {report.failed.length > 0 && (
        <div className="notice notice--danger">
          <span className="notice__icon">✕</span>
          <div>
            {report.failed.length} path
            {report.failed.length === 1 ? "" : "s"} could not be removed.
            {report.failed.slice(0, 6).map((f) => (
              <div className="advisory__path mt-8" key={f.path}>
                {f.path} — {f.message}
              </div>
            ))}
          </div>
        </div>
      )}

      {report.audit_log && (
        <div className="card mb-10" style={{ padding: 13 }}>
          <div className="row">
            <div className="col" style={{ flex: 1, minWidth: 0 }}>
              <span className="tiny dim">Audit log</span>
              <span className="mono faint">{report.audit_log}</span>
            </div>
            <button className="btn" onClick={() => openPath(report.audit_log!)}>
              Open
            </button>
          </div>
        </div>
      )}

      {dir && (
        <div className="card" style={{ padding: 13 }}>
          <div className="col mb-10">
            <span className="tiny dim">Exported report</span>
            <span className="mono faint">{dir}</span>
          </div>
          <div className="action-row">
            <button
              className="btn btn--primary"
              onClick={() => openPath(`${dir}/report.html`)}
            >
              Open HTML report
            </button>
            <button className="btn" onClick={() => openPath(dir)}>
              Open folder
            </button>
            <button
              className="btn"
              onClick={() => navigator.clipboard?.writeText(dir)}
            >
              Copy path
            </button>
          </div>
        </div>
      )}

      {!dir && plan && (
        <div className="notice notice--warn">
          <span className="notice__icon">⚠</span>
          <div style={{ flex: 1 }}>
            The report could not be written automatically. The removal itself is
            complete and the audit log was still written.
            {retryError && (
              <div className="advisory__path mt-8">{retryError}</div>
            )}
            <div className="mt-12">
              <button
                className="btn"
                disabled={retrying}
                onClick={reExport}
              >
                {retrying ? "Exporting…" : "Try exporting again"}
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}