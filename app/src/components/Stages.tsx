export function Welcome({ onScan, installed }: { onScan: () => void; installed: number }) {
  return (
    <div className="stage">
      <div className="stage__card">
        <div className="brand-mark">◆</div>
        <div className="stage__title">Nothing selected</div>
        <p className="stage__text">
          Choose an application on the left to discover everything it left
          behind — caches, preferences, containers, launch agents, receipts and
          crash reports — then decide exactly what to remove.
        </p>
        <div className="mt-16">
          {installed === 0 ? (
            <button className="btn btn--primary btn--lg" onClick={onScan}>
              Scan for installed software
            </button>
          ) : (
            <span className="tiny faint">
              Press <b>⌘R</b> to rescan, or pick an app from the list.
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

export function Analyzing({ name }: { name: string }) {
  return (
    <div className="stage">
      <div className="stage__card">
        <div className="spinner" />
        <div className="stage__title">Analyzing {name}</div>
        <p className="stage__text">
          Searching caches, preferences, containers, launch agents, receipts,
          crash reports and system configuration…
        </p>
        <div className="progress-track progress-track--indeterminate">
          <div className="progress-track__fill" />
        </div>
        <p className="tiny faint mt-12">This can take a few seconds.</p>
      </div>
    </div>
  );
}