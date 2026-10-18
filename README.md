# PerfectUninstaller

<p align="center">
  <img src="app/src-tauri/icons/icon.png" alt="PerfectUninstaller app icon" width="128" />
</p>

PerfectUninstaller scans for installed applications and related files, then lets you review each selected path before removal. It writes a JSONL audit log and exports HTML and JSON reports after an uninstall run.

Removal is permanent, so nothing is deleted without review and without passing the safety guards described below. Keep backups of important data.

## Interface

The desktop app is [Tauri 2](https://tauri.app) with a React 19 + TypeScript front end; the removal engine is plain Rust (`crates/pu-core`) with no UI dependencies.

- Opaque window with the native title bar, and a conventional light **or** dark theme. The app follows your OS appearance by default; the button in the header cycles system → light → dark.
- Sidebar lists discovered apps with their real bundle artwork (extracted from the app's `Info.plist` icon resource, falling back to a letter avatar when a bundle ships legacy or no icon); selecting one shows everything the app left behind, grouped by category.
- Every path is selectable individually or per category, with a confidence level and the reason it matched. Selecting a category header toggles all of its paths at once.
- Deletion is two-step: a review screen listing the exact paths, then a final confirmation requiring you to tick a box and type the app name.
- Progress is reported live while files are being removed, and the result screen shows what was removed, freed space, and anything that failed or was refused by the safety guards.
- System-configuration findings (shell profile edits, firewall rules, browser extensions, login items, scheduled tasks, kernel extensions, permissions) are reported as advisories with a suggested command; they are never changed automatically.

Shortcuts: `⌘R` rescan, `⌘F` focus search, `Esc` go back.

## Project layout

```
crates/pu-core/      # platform-agnostic engine: discovery, trace search, removal, reporting
  src/platform/      # per-OS discovery + trace roots (macos.rs, windows.rs, linux.rs)
  src/safety.rs      # path guards; every candidate must pass before deletion
  tests/live.rs      # #[ignore]d checks that scan this machine
app/                 # Tauri 2 desktop app
  src/               # React + TypeScript front end
  src-tauri/src/     # commands.rs (IPC) and icons.rs (bundle artwork extraction)
```

## Supported platforms

The project contains discovery and analysis backends for macOS, Windows, and Linux. Their coverage differs:

- **macOS:** discovers application bundles, Homebrew casks (`/opt/homebrew` and `/usr/local` Caskroom), and installer package receipts.
- **Windows:** discovers entries from the per-user and machine uninstall registry keys (including the `WOW6432Node` 32-bit view). MSI packages should be uninstalled through Windows Settings; PerfectUninstaller removes only the selected files and does not invoke vendor uninstall commands.
- **Linux:** discovers freedesktop `.desktop` entries, Flatpak apps, and Steam libraries. System packages, Snap packages, and Flatpak applications should be removed with their package managers; the app focuses on related user data.

Windows and Linux have never been run against a real machine — they compile and pass unit tests, but treat them as untested. macOS is the developed path.

Trace matching is heuristic. Review the confidence, reason, warnings, and path for every selected item.

Found traces are grouped into these categories: application, preferences, caches, application support, logs, sandbox container, saved state, HTTP storage, WebKit data, launch agents, launch daemons, privileged helpers, install receipts, shader caches, crash reports, registry, services, and other.

The default selection is the app's primary location plus every high- and medium-confidence match; low-confidence items are shown but left unchecked until you opt in.

## Build and run

Requires a recent stable Rust toolchain (with `rustfmt` and `clippy`) **and** Node.js 20+ / npm. The Rust build embeds the compiled front end, so build the UI first:

```sh
cd app && npm install && npm run build && cd ..

cargo run -p perfect-uninstaller
```

`app/src-tauri/build.rs` watches `app/dist`, so changing front-end code and running `npm run build` is enough — `cargo build` picks up the new assets on its own. Without that, a `dist`-only change would not recompile anything and the app would keep serving the previous UI.

Checks:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
(cd app && npm run build)   # type-checks and builds the front end
```

Platform-specific builds should be compiled and exercised on each target operating system before release.

## Testing notes

`cargo test --workspace` runs everything except the checks that touch the real machine. Two groups are ignored by default:

```sh
# Scans this machine's installed apps and checks every candidate passes the guards.
cargo test -p pu-core --test live -- --ignored --nocapture

# Resolves real bundle artwork from /Applications and asserts the payload stays small.
cargo test -p perfect-uninstaller -- --ignored --nocapture
```

Run both only where scanning the host is appropriate. The icon check is worth running after any change to `app/src-tauri/src/icons.rs`: on the development machine it resolves artwork for 85 of the 89 apps in `/Applications`, and the rest fall back to a letter avatar.

## Working on the UI

The front end runs in a plain browser against mock data, so you can iterate on layout and styling without touching the filesystem:

```sh
cd app
npm run dev
```

Then open `http://localhost:1420`. Append `&stage=plan`, `&stage=final`, or `&stage=report` to jump straight to a screen, and `&theme=light` or `&theme=dark` to preview a specific theme. This mock is only enabled in development builds and is excluded from production bundles. Remember to run `npm run build` before `cargo build` to see your changes in the packaged app.

A few flags exist to reproduce specific states that are otherwise hard to reach:

| Flag | Reproduces |
|------|-------------|
| `&stage=race` | clicks two apps back to back, so a slow analysis races a fast one |
| `&slowFirst=1` | makes the first analysis resolve last, exposing out-of-order results |
| `&exportFails=1` | an automatic report export that fails, showing the retry affordance |

## Reports

Every run writes two things below `.perfectuninstaller` in the current user's home directory (resolved from `HOME`, or the Windows profile variables):

- `audit/uninstall-<timestamp>.jsonl` — one line per path (`removed`, `already_gone`, `refused`, `failed`) plus a plan header and a summary, written before deletion starts.
- `reports/<app>-<timestamp>/` — `report.html`, `report.json`, and a copy of the audit log, exported automatically at the end of a run. You can re-export from the result screen.

The report lists the planned candidates, the results, any warnings, and any advisory findings. After a removal the app offers to open the exported `report.html`, the containing folder, or the audit log.

## Safety model

Removal is permanent, so deletion is guarded before anything is touched:

- Every candidate must be an absolute, normalized path with no `..` components.
- Anything under a protected system location (`/System`, `/usr`, `/etc`, `C:\Windows`, `C:\Program Files`, …) is refused, with a narrow exception for macOS package receipts.
- A path must sit inside a known trace root or one of the app's own install paths, so a fuzzy match cannot reach an unrelated directory.
- Symlinks in a path's parent chain are rejected, so a link cannot be used to step outside the allowed roots. A symlink may be selected as the final component, because deleting it removes the link and not its target.

Refused paths are reported rather than deleted, and always appear in the audit log.

## License

See [LICENSE](LICENSE).
