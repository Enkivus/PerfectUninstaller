import { useMemo, useState } from "react";
import type { RemovalPlan, TraceCandidate, TraceCategory } from "../lib/types";
import {
  advisoryLabel,
  categoryLabel,
  formatBytes,
  methodLabel,
  shortenPath,
} from "../lib/types";
import { CaretIcon, CheckIcon, MinusIcon } from "./icons";
import { Avatar } from "./Avatar";

type Group = { category: TraceCategory; items: TraceCandidate[] };

function group(plan: RemovalPlan): Group[] {
  const out: Group[] = [];
  for (const c of plan.candidates) {
    const found = out.find((g) => g.category === c.category);
    if (found) found.items.push(c);
    else out.push({ category: c.category, items: [c] });
  }
  return out;
}

function Confidence({ level }: { level: TraceCandidate["confidence"] }) {
  const label = level.toLowerCase();
  return (
    <span className={`conf conf--${label}`}>
      <span className="conf__dot" />
      {label}
    </span>
  );
}

function Check({
  on,
  mixed,
  onClick,
  title,
}: {
  on: boolean;
  mixed?: boolean;
  onClick: () => void;
  title?: string;
}) {
  return (
    <span
      className={`check${on ? " is-on" : ""}${mixed ? " is-mixed" : ""}`}
      onClick={onClick}
      role="checkbox"
      aria-checked={mixed ? "mixed" : on}
      aria-label={title ?? "select"}
    >
      {mixed ? <MinusIcon /> : on ? <CheckIcon /> : null}
    </span>
  );
}

export function PlanView({
  plan,
  checked,
  onToggle,
  onToggleGroup,
  onSelectAll,
  onSelectNone,
  onSelectRecommended,
  onUninstall,
  onBack,
  busy,
  icon,
}: {
  plan: RemovalPlan;
  checked: Set<string>;
  onToggle: (path: string) => void;
  onToggleGroup: (paths: string[], on: boolean) => void;
  onSelectAll: () => void;
  onSelectNone: () => void;
  onSelectRecommended: () => void;
  onUninstall: () => void;
  onBack: () => void;
  busy: boolean;
  icon?: string | null;
}) {
  const groups = useMemo(() => group(plan), [plan]);
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const [advOpen, setAdvOpen] = useState(false);

  const selectedBytes = useMemo(
    () =>
      plan.candidates
        .filter((c) => checked.has(c.path))
        .reduce((sum, c) => sum + c.bytes, 0),
    [plan, checked],
  );

  const isAll = checked.size === plan.candidates.length && checked.size > 0;
  const isNone = checked.size === 0;

  return (
    <>
      <div className="view-head">
        <button className="btn btn--icon" onClick={onBack} title="Back">
          ‹
        </button>
        <div className="view-head__avatar">
          <Avatar name={plan.app.name} icon={icon} size={42} />
        </div>
        <div className="col">
          <div className="row" style={{ gap: 9 }}>
            <span className="view-head__title">{plan.app.name}</span>
            <span className="badge">{methodLabel(plan.app.method)}</span>
          </div>
          {plan.app.identifier && (
            <span className="view-head__sub">{plan.app.identifier}</span>
          )}
        </div>
      </div>

      {plan.warnings.map((w) => (
        <div className="notice notice--warn" key={w}>
          <span className="notice__icon">⚠</span>
          <span>{w}</span>
        </div>
      ))}

      <div className="stats">
        <div className="stat">
          <div className="stat__label">Selected</div>
          <div className="stat__value" style={{ color: "var(--accent-hi)" }}>
            {checked.size}
          </div>
        </div>
        <div className="stat">
          <div className="stat__label">Selected size</div>
          <div className="stat__value" style={{ color: "var(--accent-hi)" }}>
            {formatBytes(selectedBytes)}
          </div>
        </div>
        <div className="stat">
          <div className="stat__label">Total found</div>
          <div className="stat__value">{formatBytes(plan.total_bytes)}</div>
        </div>
        <div className="stat">
          <div className="stat__label">Categories</div>
          <div className="stat__value">{groups.length}</div>
        </div>
      </div>

      {plan.advisories.length > 0 && (
        <div className={`group mb-14${advOpen ? " is-open" : ""}`}>
          <button
            className="group__header"
            onClick={() => setAdvOpen((v) => !v)}
          >
            <CaretIcon className="group__caret" />
            <div className="col" style={{ flex: 1 }}>
              <span className="group__name">System configuration</span>
              <span className="group__meta">
                {plan.advisories.length} finding
                {plan.advisories.length === 1 ? "" : "s"} found — never changed
                automatically
              </span>
            </div>
            <span className="badge badge--warn">review</span>
          </button>
          {advOpen && (
            <div className="group__items" style={{ padding: 10 }}>
              {plan.advisories.map((a, i) => (
                <div className="advisory" key={i}>
                  <div className="advisory__head">
                    <span className="advisory__summary">{a.summary}</span>
                    <span className="badge">{advisoryLabel(a.kind)}</span>
                  </div>
                  <div className="advisory__detail">{a.detail}</div>
                  {a.path && <div className="advisory__path">{a.path}</div>}
                  {a.command && (
                    <div className="code">
                      <span className="code__text">{a.command}</span>
                      <button
                        className="btn"
                        onClick={() => navigator.clipboard?.writeText(a.command!)}
                      >
                        Copy
                      </button>
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      <div className="toolbar">
        <div className="segmented">
          <button
            className={isAll ? "is-active" : ""}
            onClick={onSelectAll}
          >
            All
          </button>
          <button
            className={!isAll && !isNone ? "is-active" : ""}
            onClick={onSelectRecommended}
          >
            Recommended
          </button>
          <button className={isNone ? "is-active" : ""} onClick={onSelectNone}>
            None
          </button>
        </div>
        <span className="tiny dim">
          {checked.size} of {plan.candidates.length} items
        </span>
        <div className="toolbar__spacer" />
        <button
          className="btn btn--danger btn--lg"
          disabled={checked.size === 0 || busy}
          onClick={onUninstall}
        >
          Uninstall · {formatBytes(selectedBytes)}
        </button>
      </div>

      <div className="groups">
        {groups.map((g) => {
          const paths = g.items.map((c) => c.path);
          const selCount = paths.filter((p) => checked.has(p)).length;
          const bytes = g.items.reduce((s, c) => s + c.bytes, 0);
          const all = selCount === paths.length && selCount > 0;
          const mixed = selCount > 0 && !all;
          const key = g.category;
          const expanded = open[key] ?? true;

          return (
            <div className={`group${expanded ? " is-open" : ""}`} key={key}>
              <button
                className="group__header"
                onClick={() => setOpen((o) => ({ ...o, [key]: !expanded }))}
              >
                <CaretIcon className="group__caret" />
                <Check
                  on={all}
                  mixed={mixed}
                  onClick={() => onToggleGroup(paths, !all)}
                  title={`Toggle ${categoryLabel(g.category)}`}
                />
                <div className="col" style={{ flex: 1 }}>
                  <span className="group__name">{categoryLabel(g.category)}</span>
                  <span className="group__meta">
                    {selCount} of {g.items.length} selected
                  </span>
                </div>
                <span className="group__size">{formatBytes(bytes)}</span>
              </button>

              {expanded && (
                <div className="group__items">
                  {g.items.map((c) => (
                    <div className="item" key={c.path} title={c.reason}>
                      <Check
                        on={checked.has(c.path)}
                        onClick={() => onToggle(c.path)}
                        title={c.path}
                      />
                      <Confidence level={c.confidence} />
                      <span className="item__path">{shortenPath(c.path)}</span>
                      <span className="item__size">{formatBytes(c.bytes)}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </>
  );
}