# PerfectUninstaller

PerfectUninstaller scans for installed applications and related files, then lets you review each selected path before removal. It writes a JSONL audit log and exports HTML and JSON reports after an uninstall run.

Removal is permanent. Review the paths and warnings in the app before confirming. The safety checks are defense in depth; keep backups of important data.

## Supported platforms

The project contains discovery and analysis backends for macOS, Windows, and Linux. Their coverage differs:

- **macOS:** discovers application bundles, Homebrew casks, and package receipts. Package receipts are not a complete inventory of package-installed files.
- **Windows:** discovers entries from the per-user and machine uninstall registry keys. MSI packages should be uninstalled through Windows Settings; PerfectUninstaller removes only the selected files and does not invoke vendor uninstall commands.
- **Linux:** discovers desktop entries and Flatpak apps. System packages, Snap packages, and Flatpak applications should be removed with their package managers; the app focuses on related user data.

Trace matching is heuristic. Review the confidence, reason, warnings, and path for every selected item. System configuration findings are reported as advisories and are not changed automatically.

## Build and run

Install a recent stable Rust toolchain with the `rustfmt` and `clippy` components, then run:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p pu-gui
```

Platform-specific builds should be compiled and exercised on each target operating system before release. The live checks in `crates/pu-core/tests/live.rs` are ignored by default because they inspect the host's installed apps; run them only on a machine where that scan is appropriate:

```sh
cargo test -p pu-core --test live -- --ignored --nocapture
```

## Reports

Uninstall audit logs and exported reports are written below `.perfectuninstaller` in the current user's home directory (resolved from `HOME`, or Windows profile variables). The report includes planned candidates, results, warnings, and any advisory findings.

## License

See [LICENSE](LICENSE).
