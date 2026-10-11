import type { RemovalPlan, TraceCandidate, TraceCategory } from "../lib/types";
import { categoryLabel, formatBytes, shortenPath } from "../lib/types";

type Props = {
  stage: "review" | "final";
  plan: RemovalPlan;
  selected: TraceCandidate[];
  ack: boolean;
  typed: string;
  onAck: (v: boolean) => void;
  onTyped: (v: string) => void;
  onCancel: () => void;
  onBack: () => void;
  onContinue: () => void;
  onConfirm: () => void;
  onClose: () => void;
};

function group(items: TraceCandidate[]) {
  const out: { category: TraceCategory; items: TraceCandidate[] }[] = [];
  for (const c of items) {
    const found = out.find((g) => g.category === c.category);
    if (found) found.items.push(c);
    else out.push({ category: c.category, items: [c] });
  }
  return out;
}

export function ConfirmDialog({
  stage,
  plan,
  selected,
  ack,
  typed,
  onAck,
  onTyped,
  onCancel,
  onBack,
  onContinue,
  onConfirm,
  onClose,
}: Props) {
  const bytes = selected.reduce((s, c) => s + c.bytes, 0);
  const matches =
    typed.trim().toLowerCase() === plan.app.name.trim().toLowerCase();
  const canDelete = ack && matches && selected.length > 0;

  return (
    <div className="scrim" onClick={onClose}>
      {stage === "review" ? (
        <div
          className="modal modal--wide"
          onClick={(e) => e.stopPropagation()}
        >
          <div className="modal__body">
            <div className="modal__eyebrow" style={{ color: "var(--accent-hi)" }}>
              Step 1 of 2 · Review
            </div>
            <div className="modal__title">
              Remove everything for “{plan.app.name}”?
            </div>
            <div className="row mt-12" style={{ gap: 8 }}>
              <span className="badge badge--accent">
                {selected.length} items
              </span>
              <span className="badge badge--accent">{formatBytes(bytes)}</span>
            </div>
            <div className="modal__text">
              These are the exact paths that will be permanently deleted:
            </div>
            <div className="modal__list">
              {group(selected).map((g) => (
                <div key={g.category} style={{ padding: "8px 0" }}>
                  <div
                    className="row tiny"
                    style={{ padding: "0 12px 4px", gap: 8 }}
                  >
                    <strong>{categoryLabel(g.category)}</strong>
                    <span className="faint">{g.items.length}</span>
                  </div>
                  {g.items.map((c) => (
                    <div
                      key={c.path}
                      className="tiny mono faint"
                      style={{
                        padding: "2px 12px",
                        whiteSpace: "nowrap",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                      }}
                    >
                      {shortenPath(c.path, 70)}
                    </div>
                  ))}
                </div>
              ))}
            </div>
          </div>
          <div className="modal__foot">
            <button className="btn" onClick={onCancel}>
              Cancel
            </button>
            <div className="spacer" />
            <button className="btn btn--primary" onClick={onContinue}>
              Continue →
            </button>
          </div>
        </div>
      ) : (
        <div className="modal" onClick={(e) => e.stopPropagation()}>
          <div className="modal__body">
            <div className="modal__eyebrow" style={{ color: "var(--danger)" }}>
              Step 2 of 2 · Final confirmation
            </div>
            <div className="modal__title">This cannot be undone</div>
            <div className="modal__text">
              You are about to permanently delete <b>{selected.length}</b> item
              {selected.length === 1 ? "" : "s"} ({formatBytes(bytes)}) for “
              {plan.app.name}”. Files are removed immediately — there is no
              trash and no undo. A JSONL audit log and an HTML/JSON report will
              be kept.
            </div>

            <label className="check-row">
              <span
                className={`check${ack ? " is-on" : ""}`}
                onClick={() => onAck(!ack)}
              >
                {ack ? <CheckMark /> : null}
              </span>
              <span>
                I understand this permanently deletes the listed items and
                cannot be undone.
              </span>
            </label>

            <div className="modal__text" style={{ marginTop: 14 }}>
              Type “{plan.app.name}” below to confirm:
            </div>
            <input
              className="typing-input"
              value={typed}
              spellCheck={false}
              autoFocus
              placeholder={plan.app.name}
              onChange={(e) => onTyped(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && canDelete) onConfirm();
              }}
            />
          </div>
          <div className="modal__foot">
            <button className="btn" onClick={onCancel}>
              Cancel
            </button>
            <button className="btn" onClick={onBack}>
              ← Back
            </button>
            <div className="spacer" />
            <button
              className="btn btn--danger"
              disabled={!canDelete}
              onClick={onConfirm}
            >
              Delete forever
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function CheckMark() {
  return (
    <svg viewBox="0 0 12 12" fill="none" style={{ width: 11, height: 11 }}>
      <path
        d="M2 6.2 L4.6 8.8 L10 3.2"
        stroke="#fff"
        strokeWidth="1.9"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}