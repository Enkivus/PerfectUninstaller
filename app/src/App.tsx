import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  analyze,
  defaultSelection,
  scanApps,
  uninstall,
} from "./lib/api";
import type {
  InstalledApp,
  Progress,
  RemovalPlan,
  RemovalReport,
} from "./lib/types";
import { Sidebar } from "./components/Sidebar";
import { PlanView } from "./components/PlanView";
import { ReportView } from "./components/ReportView";
import { Analyzing, Welcome } from "./components/Stages";
import { ConfirmDialog } from "./components/ConfirmDialog";
import { AboutDialog, ProgressToast } from "./components/Dialogs";

type Busy = "none" | "scan" | "analyze" | "uninstall";

const IS_MAC = /Mac/i.test(navigator.userAgent);

export default function App() {
  const [apps, setApps] = useState<InstalledApp[]>([]);
  const [filter, setFilter] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [busy, setBusy] = useState<Busy>("none");
  const [plan, setPlan] = useState<RemovalPlan | null>(null);
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [report, setReport] = useState<RemovalReport | null>(null);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<"review" | "final" | null>(null);
  const [ack, setAck] = useState(false);
  const [typed, setTyped] = useState("");
  const [about, setAbout] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);

  const runScan = useCallback(async () => {
    if (busy === "uninstall") return;
    setBusy("scan");
    setError(null);
    try {
      const found = await scanApps();
      setApps(found);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy("none");
    }
  }, [busy]);

  useEffect(() => {
    void runScan();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const selectApp = useCallback(async (app: InstalledApp) => {
    setSelectedId(app.id);
    setPlan(null);
    setReport(null);
    setChecked(new Set());
    setBusy("analyze");
    setError(null);
    try {
      const p = await analyze(app);
      setPlan(p);
      const recommended = await defaultSelection(p);
      setChecked(new Set(recommended));
    } catch (e) {
      setError(String(e));
      setPlan(null);
    } finally {
      setBusy("none");
    }
  }, []);

  const selectedApp = useMemo(
    () => apps.find((a) => a.id === selectedId) ?? null,
    [apps, selectedId],
  );

  const toggle = useCallback((path: string) => {
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }, []);

  const toggleGroup = useCallback((paths: string[], on: boolean) => {
    setChecked((prev) => {
      const next = new Set(prev);
      for (const p of paths) {
        if (on) next.add(p);
        else next.delete(p);
      }
      return next;
    });
  }, []);

  const selectedCandidates = useMemo(
    () => plan?.candidates.filter((c) => checked.has(c.path)) ?? [],
    [plan, checked],
  );

  const runUninstall = useCallback(async () => {
    if (!plan || selectedCandidates.length === 0) return;
    setConfirm(null);
    setBusy("uninstall");
    setProgress({ path: "", done: 0, total: selectedCandidates.length });
    try {
      const result = await uninstall(
        plan,
        selectedCandidates.map((c) => c.path),
        (p) => setProgress(p),
      );
      setReport(result);
      setPlan(null);
      setChecked(new Set());
      setSelectedId(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setProgress(null);
      setBusy("none");
    }
  }, [plan, selectedCandidates]);

  const goBack = useCallback(() => {
    if (confirm) return;
    if (report) {
      setReport(null);
      void runScan();
      return;
    }
    if (plan) {
      setPlan(null);
      setChecked(new Set());
      setSelectedId(null);
    }
  }, [confirm, report, plan, runScan]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const meta = e.metaKey || e.ctrlKey;
      if (meta && e.key.toLowerCase() === "r") {
        e.preventDefault();
        void runScan();
      } else if (meta && e.key.toLowerCase() === "f") {
        e.preventDefault();
        searchRef.current?.focus();
      } else if (e.key === "Escape") {
        if (confirm || about) return;
        goBack();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [runScan, goBack, confirm, about]);

  const body = (
    <main className="main">
      {error && (
        <div className="notice notice--danger mb-14">
          <span className="notice__icon">✕</span>
          <div style={{ flex: 1 }}>{error}</div>
          <button className="btn btn--icon" onClick={() => setError(null)}>
            ✕
          </button>
        </div>
      )}

      {report ? (
        <ReportView report={report} plan={plan} onBack={goBack} />
      ) : busy === "analyze" ? (
        <Analyzing name={selectedApp?.name ?? ""} />
      ) : plan ? (
        <PlanView
          plan={plan}
          checked={checked}
          onToggle={toggle}
          onToggleGroup={toggleGroup}
          onSelectAll={() => setChecked(new Set(plan.candidates.map((c) => c.path)))}
          onSelectNone={() => setChecked(new Set())}
          onSelectRecommended={async () => {
            const recommended = await defaultSelection(plan);
            setChecked(new Set(recommended));
          }}
          onUninstall={() => {
            setAck(false);
            setTyped("");
            setConfirm("review");
          }}
          onBack={goBack}
          busy={busy === "uninstall"}
        />
      ) : (
        <Welcome onScan={runScan} installed={apps.length} />
      )}
    </main>
  );

  return (
    <div
      className="shell"
      style={{ ["--traffic-inset" as string]: IS_MAC ? "78px" : "16px" }}
    >
      <header className="titlebar" data-tauri-drag-region>
        <div className="titlebar__brand" data-tauri-drag-region>
          <span className="brand-mark" data-tauri-drag-region>
            ◆
          </span>
          <div className="col" data-tauri-drag-region>
            <span className="titlebar__title" data-tauri-drag-region>
              PerfectUninstaller
            </span>
          </div>
        </div>
        <span className="badge badge--accent">
          {apps.length} installed
        </span>
        <div className="titlebar__spacer" />
        <button
          className="btn"
          disabled={busy === "uninstall"}
          onClick={runScan}
        >
          {busy === "scan" ? "Scanning…" : "Scan"}
        </button>
        <button className="btn" onClick={() => setAbout(true)}>
          About
        </button>
      </header>

      <div className="body">
        <Sidebar
          apps={apps}
          filter={filter}
          onFilter={setFilter}
          selectedId={selectedId}
          onSelect={selectApp}
          scanning={busy === "scan"}
          onScan={runScan}
          searchRef={searchRef}
        />
        {body}
      </div>

      {progress && <ProgressToast progress={progress} />}

      {confirm && plan && (
        <ConfirmDialog
          stage={confirm}
          plan={plan}
          selected={selectedCandidates}
          ack={ack}
          typed={typed}
          onAck={setAck}
          onTyped={setTyped}
          onCancel={() => {
            setConfirm(null);
            setAck(false);
            setTyped("");
          }}
          onBack={() => setConfirm("review")}
          onContinue={() => setConfirm("final")}
          onConfirm={runUninstall}
          onClose={() => setConfirm(null)}
        />
      )}

      {about && <AboutDialog onClose={() => setAbout(false)} />}
    </div>
  );
}