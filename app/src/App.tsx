import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  analyze,
  appIcons,
  defaultSelection,
  iconsById,
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
import { ThemeToggle } from "./components/ThemeToggle";

type Busy = "none" | "scan" | "analyze" | "uninstall";

export default function App() {
  const [apps, setApps] = useState<InstalledApp[]>([]);
  const [icons, setIcons] = useState<Map<string, string | null>>(new Map());
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

  // Async work (scan / analyze / uninstall) resolves out of order if the user
  // clicks around quickly. Each run takes a ticket; only the newest ticket may
  // commit its result, so a slow `analyze` for app A can never overwrite the
  // plan of app B that the user selected in the meantime.
  const opSeq = useRef(0);
  // Mirrors `busy` for callbacks that must read it without stale-closure risk.
  const busyRef = useRef<Busy>("none");

  const beginOp = useCallback((next: Busy) => {
    opSeq.current += 1;
    busyRef.current = next;
    setBusy(next);
    return opSeq.current;
  }, []);

  const finishOp = useCallback((ticket: number) => {
    if (ticket !== opSeq.current) return;
    busyRef.current = "none";
    setBusy("none");
  }, []);

  const runScan = useCallback(async () => {
    if (busyRef.current === "uninstall") return;
    const ticket = beginOp("scan");
    setError(null);
    try {
      const found = await scanApps();
      // Artwork is a nice-to-have: a failure here must not break the scan.
      const artwork = await appIcons(found).catch(() => []);
      setApps(found);
      setIcons(iconsById(artwork));
    } catch (e) {
      if (ticket === opSeq.current) setError(String(e));
    } finally {
      finishOp(ticket);
    }
  }, [beginOp, finishOp]);

  useEffect(() => {
    void runScan();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const selectApp = useCallback(
    async (app: InstalledApp) => {
      const ticket = beginOp("analyze");
      setSelectedId(app.id);
      setPlan(null);
      setReport(null);
      setChecked(new Set());
      setError(null);
      try {
        const p = await analyze(app);
        const recommended = await defaultSelection(p);
        // Discard the result if another app (or another run) was started while
        // this analysis was in flight.
        if (ticket !== opSeq.current) return;
        setPlan(p);
        setChecked(new Set(recommended));
      } catch (e) {
        if (ticket !== opSeq.current) return;
        setError(String(e));
        setPlan(null);
      } finally {
        finishOp(ticket);
      }
    },
    [beginOp, finishOp],
  );

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
    // Belt and braces: the UI disables the button while a run is in flight, but
    // a second concurrent run would delete paths twice.
    if (!plan || selectedCandidates.length === 0) return;
    if (busyRef.current === "uninstall") return;
    const ticket = beginOp("uninstall");
    setConfirm(null);
    setProgress({ path: "", done: 0, total: selectedCandidates.length });
    try {
      const result = await uninstall(
        plan,
        selectedCandidates.map((c) => c.path),
        (p) => setProgress(p),
      );
      if (ticket !== opSeq.current) return;
      setReport(result);
      setChecked(new Set());
      setSelectedId(null);
      // The plan is intentionally kept so the result screen can re-export the
      // report (the exporter needs it) if the automatic export failed.
    } catch (e) {
      if (ticket === opSeq.current) setError(String(e));
    } finally {
      setProgress(null);
      finishOp(ticket);
    }
  }, [plan, selectedCandidates, beginOp, finishOp]);

  const goBack = useCallback(() => {
    if (confirm) return;
    if (report) {
      setReport(null);
      setPlan(null);
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
          icon={selectedApp ? icons.get(selectedApp.id) : null}
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
    <div className="shell">
      <header className="titlebar">
        <div className="titlebar__brand">
          <span className="brand-mark">◆</span>
          <span className="titlebar__title">PerfectUninstaller</span>
        </div>
        <span className="badge badge--accent">{apps.length} installed</span>
        <div className="titlebar__spacer" />
        <button
          className="btn"
          disabled={busy === "uninstall"}
          onClick={runScan}
        >
          {busy === "scan" ? "Scanning…" : "Scan"}
        </button>
        <ThemeToggle />
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
          icons={icons}
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