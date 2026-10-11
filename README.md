# PerfectUninstaller

<p align="center">
  <img src="app/src-tauri/icons/icon.png" alt="PerfectUninstaller app icon" width="128" />
</p>

PerfectUninstaller scans for installed applications and related files, then lets you review each selected path before removal. It writes a JSONL audit log and exports HTML and JSON reports after an uninstall run.

Removal is permanent. Review the paths and warnings in the app before confirming. The safety checks are defense in depth; keep backups of important data.

## Interface

The desktop app is [Tauri 2](https://tauri.app) with a React 19 + TypeScript front end; the removal engine is plain Rust (`crates/pu-core`) with no UI dependencies.

- Opaque window with the native title bar, and a conventional light **or** dark theme. The app follows your OS appearance by default; the button in the header cycles system → light → dark.
- Sidebar lists discovered apps with their real bundle artwork (extracted from the app's `Info.plist` icon resource, falling back to a letter avatar); selecting one shows everything the app left behind, grouped by category.
- Every path is selectable individually or per category, with a confidence level and the reason it matched.
- Deletion is two-step: a review screen listing the exact paths, then a final confirmation requiring you to tick a box and type the app name.
- System-configuration findings (firewall rules, shell profiles, browser extensions, login items) are reported as advisories with a suggested command; they are never changed automatically.

Shortcuts: `⌘R` rescan, `⌘F` focus search, `Esc` go back.

## Supported platforms

The project contains discovery and analysis backends for macOS, Windows, and Linux. Their coverage differs:

- **macOS:** discovers application bundles, Homebrew casks, and package receipts. Package receipts are not a complete inventory of package-installed files.
- **Windows:** discovers entries from the per-user and machine uninstall registry keys. MSI packages should be uninstalled through Windows Settings; PerfectUninstaller removes only the selected files and does not invoke vendor uninstall commands.
- **Linux:** discovers desktop entries and Flatpak apps. System packages, Snap packages, and Flatpak applications should be removed with their package managers; the app focuses on related user data.

Trace matching is heuristic. Review the confidence, reason, warnings, and path for every selected item.

## Build and run

Requires a recent stable Rust toolchain (with `rustfmt` and `clippy`) **and** Node.js 20+ / npm. The Rust build embeds the compiled front end, so build the UI first:

```sh
cd app && npm install && npm run build && cd ..

cargo run -p perfect-uninstaller
```

Checks:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
(cd app && npm run build)   # type-checks and builds the front end
```

Platform-specific builds should be compiled and exercised on each target operating system before release. The live checks in `crates/pu-core/tests/live.rs` are ignored by default because they inspect the host's installed apps; run them only on a machine where that scan is appropriate:

```sh
cargo test -p pu-core --test live -- --ignored --nocapture
```

## Working on the UI

The front end runs in a plain browser against mock data, so you can iterate on layout and styling without touching the filesystem:

```sh
cd app
npm run dev
```

Then open `http://localhost:1420`. Append `&stage=plan`, `&stage=final`, or `&stage=report` to jump straight to a screen, and `&theme=light` or `&theme=dark` to preview a specific theme. This mock is only enabled in development builds and is excluded from production bundles. Remember to run `npm run build` before `cargo build` to see your changes in the packaged app.

## Reports

Uninstall audit logs and exported reports are written below `.perfectuninstaller` in the current user's home directory (resolved from `HOME`, or Windows profile variables). The report includes planned candidates, results, warnings, and any advisory findings. After a removal the app offers to open the exported `report.html`, the containing folder, or the audit log.

## License

See [LICENSE](LICENSE).
