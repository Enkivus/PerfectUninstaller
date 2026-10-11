import { useMemo, useRef } from "react";
import type { InstalledApp } from "../lib/types";
import { methodLabel } from "../lib/types";
import { SearchIcon } from "./icons";
import { Avatar } from "./Avatar";

type Props = {
  apps: InstalledApp[];
  filter: string;
  onFilter: (v: string) => void;
  selectedId: string | null;
  onSelect: (app: InstalledApp) => void;
  scanning: boolean;
  onScan: () => void;
  icons: Map<string, string | null>;
  searchRef?: React.RefObject<HTMLInputElement | null>;
};

export function Sidebar({
  apps,
  filter,
  onFilter,
  selectedId,
  onSelect,
  scanning,
  onScan,
  icons,
  searchRef,
}: Props) {
  const localRef = useRef<HTMLInputElement>(null);
  const inputRef = searchRef ?? localRef;

  const visible = useMemo(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return apps;
    return apps.filter(
      (a) =>
        a.name.toLowerCase().includes(q) ||
        (a.identifier ?? "").toLowerCase().includes(q),
    );
  }, [apps, filter]);

  return (
    <aside className="sidebar">
      <div className="sidebar__head">
        <span className="eyebrow">Applications</span>
        <span className="count-pill">{visible.length}</span>
      </div>

      <div className="search">
        <SearchIcon />
        <input
          ref={inputRef}
          value={filter}
          placeholder="Search name or identifier"
          spellCheck={false}
          onChange={(e) => onFilter(e.target.value)}
        />
      </div>

      <div className="app-list">
        {scanning && apps.length === 0 ? (
          <div className="sidebar__empty">Scanning…</div>
        ) : apps.length === 0 ? (
          <div className="sidebar__empty">
            <p>No software found.</p>
            <button className="btn mt-12" onClick={onScan}>
              Scan now
            </button>
          </div>
        ) : visible.length === 0 ? (
          <div className="sidebar__empty">No matches</div>
        ) : (
          visible.map((app) => (
            <button
              key={app.id}
              className={`app-row${selectedId === app.id ? " is-selected" : ""}`}
              onClick={() => onSelect(app)}
            >
              <Avatar name={app.name} icon={icons.get(app.id)} />
              <span className="app-row__body">
                <span className="app-row__name">{app.name}</span>
                <span className="app-row__meta">
                  {[app.version, methodLabel(app.method)]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
              </span>
            </button>
          ))
        )}
      </div>
    </aside>
  );
}