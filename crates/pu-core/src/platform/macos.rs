use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::models::{
    Advisory, AdvisoryKind, Confidence, InstallMethod, InstalledApp, Platform, RemovalPlan,
    TraceCandidate, TraceCategory,
};
use crate::platform::PlatformBackend;
use crate::trace_search::{find_traces, Query, SearchRoot};
use crate::util::measure;

pub struct MacosBackend;

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/root"))
}

fn h(rel: &str) -> PathBuf {
    home().join(rel)
}

fn root(path: PathBuf, category: TraceCategory, depth: usize, label: &'static str) -> SearchRoot {
    SearchRoot { path, category, depth, label }
}

/// Every directory macOS apps commonly leave data in.
fn search_roots() -> Vec<SearchRoot> {
    vec![
        root(h("Library/Preferences"), TraceCategory::Preference, 0, "~/Library/Preferences"),
        root(
            h("Library/Preferences/ByHost"),
            TraceCategory::Preference,
            0,
            "~/Library/Preferences/ByHost",
        ),
        root(
            h("Library/Application Support"),
            TraceCategory::Support,
            1,
            "~/Library/Application Support",
        ),
        root(h("Library/Caches"), TraceCategory::Cache, 0, "~/Library/Caches"),
        root(h("Library/Logs"), TraceCategory::Log, 0, "~/Library/Logs"),
        root(
            h("Library/Logs/DiagnosticReports"),
            TraceCategory::CrashReport,
            0,
            "~/Library/Logs/DiagnosticReports",
        ),
        root(
            h("Library/Application Support/CrashReporter"),
            TraceCategory::CrashReport,
            0,
            "~/Library/Application Support/CrashReporter",
        ),
        root(
            PathBuf::from("/Library/Logs/DiagnosticReports"),
            TraceCategory::CrashReport,
            0,
            "/Library/Logs/DiagnosticReports",
        ),
        root(h("Library/Containers"), TraceCategory::Container, 1, "~/Library/Containers"),
        root(
            h("Library/Group Containers"),
            TraceCategory::Container,
            1,
            "~/Library/Group Containers",
        ),
        root(h("Library/Application Scripts"), TraceCategory::Container, 0, "~/Library/Application Scripts"),
        root(
            h("Library/Saved Application State"),
            TraceCategory::SavedState,
            0,
            "~/Library/Saved Application State",
        ),
        root(h("Library/HTTPStorages"), TraceCategory::HttpStorage, 0, "~/Library/HTTPStorages"),
        root(h("Library/WebKit"), TraceCategory::WebKit, 0, "~/Library/WebKit"),
        root(h("Library/LaunchAgents"), TraceCategory::LaunchAgent, 0, "~/Library/LaunchAgents"),
        root(
            h("Library/Application Support/Steam/steamapps/common"),
            TraceCategory::Support,
            0,
            "~/Library/Application Support/Steam/steamapps/common",
        ),
        root(
            PathBuf::from("/Library/Application Support"),
            TraceCategory::Support,
            1,
            "/Library/Application Support",
        ),
        root(PathBuf::from("/Library/Caches"), TraceCategory::Cache, 0, "/Library/Caches"),
        root(PathBuf::from("/Library/Logs"), TraceCategory::Log, 0, "/Library/Logs"),
        root(PathBuf::from("/Library/Preferences"), TraceCategory::Preference, 0, "/Library/Preferences"),
        root(PathBuf::from("/Library/LaunchAgents"), TraceCategory::LaunchAgent, 0, "/Library/LaunchAgents"),
        root(PathBuf::from("/Library/LaunchDaemons"), TraceCategory::LaunchDaemon, 0, "/Library/LaunchDaemons"),
        root(
            PathBuf::from("/Library/PrivilegedHelperTools"),
            TraceCategory::PrivilegedHelper,
            0,
            "/Library/PrivilegedHelperTools",
        ),
        root(PathBuf::from("/var/db/receipts"), TraceCategory::Receipt, 0, "/var/db/receipts"),
    ]
}

#[derive(Default)]
struct BundleInfo {
    identifier: Option<String>,
    name: Option<String>,
    version: Option<String>,
}

fn read_info_plist(bundle: &Path) -> BundleInfo {
    let path = bundle.join("Contents/Info.plist");
    let Ok(value) = plist::Value::from_file(&path) else {
        return BundleInfo::default();
    };
    let Some(dict) = value.as_dictionary() else {
        return BundleInfo::default();
    };
    let get = |key: &str| -> Option<String> {
        dict.get(key)
            .and_then(|v| v.as_string())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    BundleInfo {
        identifier: get("CFBundleIdentifier"),
        name: get("CFBundleDisplayName").or_else(|| get("CFBundleName")),
        version: get("CFBundleShortVersionString"),
    }
}

/// Collects `*.app` bundles, never descending into a bundle itself.
fn find_bundles(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("app")) {
            out.push(path);
            continue;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir && depth > 0 {
            find_bundles(&path, depth - 1, out);
        }
    }
}

fn app_from_bundle(path: PathBuf) -> InstalledApp {
    let info = read_info_plist(&path);
    let fallback_name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".into());
    let name = info.name.unwrap_or(fallback_name);
    let id = info
        .identifier
        .clone()
        .unwrap_or_else(|| path.to_string_lossy().to_string());
    InstalledApp {
        id,
        name,
        identifier: info.identifier,
        version: info.version,
        install_paths: vec![path],
        method: InstallMethod::AppBundle,
        platform: Platform::Macos,
        install_bytes: 0,
    }
}

fn discover_casks(out: &mut Vec<InstalledApp>) {
    for cask_root in ["/opt/homebrew/Caskroom", "/usr/local/Caskroom"] {
        let Ok(read) = fs::read_dir(cask_root) else {
            continue;
        };
        for cask in read.flatten() {
            let cask_dir = cask.path();
            let cask_name = cask.file_name().to_string_lossy().to_string();

            let mut bundles = Vec::new();
            find_bundles(&cask_dir, 3, &mut bundles);

            let (name, identifier, version) = match bundles.first() {
                Some(bundle) => {
                    let info = read_info_plist(bundle);
                    let name = info
                        .name
                        .clone()
                        .unwrap_or_else(|| cask_name.clone());
                    (name, info.identifier, info.version)
                }
                None => {
                    let version = fs::read_dir(&cask_dir)
                        .ok()
                        .map(|r| r.flatten())
                        .and_then(|mut r| r.next())
                        .map(|e| e.file_name().to_string_lossy().to_string());
                    (cask_name.clone(), None, version)
                }
            };

            out.push(InstalledApp {
                id: format!("cask:{cask_name}"),
                name,
                identifier,
                version,
                install_paths: vec![cask_dir],
                method: InstallMethod::Caskroom,
                platform: Platform::Macos,
                install_bytes: 0,
            });
        }
    }
}

fn discover_receipts(apps: &mut Vec<InstalledApp>) {
    let dir = Path::new("/var/db/receipts");
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let file_name = entry.file_name().to_string_lossy().to_string();
        let Some(pkg) = file_name.strip_suffix(".bom") else {
            continue;
        };
        if pkg.starts_with("com.apple.") || pkg.starts_with("org.opensource.") {
            continue;
        }

        let mut paths = vec![entry.path()];
        let plist = dir.join(format!("{pkg}.plist"));
        if plist.exists() {
            paths.push(plist);
        }

        let belongs_to = apps.iter().position(|app| {
            app.identifier
                .as_deref()
                .is_some_and(|id| pkg == id || pkg.starts_with(&format!("{id}.")))
        });
        if let Some(index) = belongs_to {
            apps[index].install_paths.extend(paths);
            continue;
        }

        apps.push(InstalledApp {
            id: pkg.to_string(),
            name: pkg.to_string(),
            identifier: Some(pkg.to_string()),
            version: None,
            install_paths: paths,
            method: InstallMethod::PkgReceipt,
            platform: Platform::Macos,
            install_bytes: 0,
        });
    }
}

/// Apps discovered twice (bundle + cask, bundle + receipt) are merged so one
/// entry owns every install location.
fn merge_duplicates(apps: &mut Vec<InstalledApp>) {
    let mut merged: Vec<InstalledApp> = Vec::new();
    for app in apps.drain(..) {
        let existing = app.identifier.as_deref().and_then(|id| {
            merged.iter_mut().find(|other| {
                other.identifier.as_deref() == Some(id)
                    && (other.method == InstallMethod::AppBundle
                        || app.method == InstallMethod::AppBundle)
            })
        });
        match existing {
            Some(target) => {
                if target.method == InstallMethod::Caskroom && app.method == InstallMethod::AppBundle {
                    target.method = InstallMethod::AppBundle;
                    target.name = app.name.clone();
                    target.version = app.version.clone();
                    target.id = app.id.clone();
                }
                for path in app.install_paths {
                    if !target.install_paths.contains(&path) {
                        target.install_paths.push(path);
                    }
                }
            }
            None => merged.push(app),
        }
    }
    *apps = merged;
}

impl PlatformBackend for MacosBackend {
    fn platform(&self) -> Platform {
        Platform::Macos
    }

    fn discover(&self) -> Result<Vec<InstalledApp>> {
        let mut apps = Vec::new();

        let mut bundles = Vec::new();
        find_bundles(Path::new("/Applications"), 4, &mut bundles);
        let user_apps = h("Applications");
        find_bundles(&user_apps, 4, &mut bundles);

        for bundle in bundles {
            apps.push(app_from_bundle(bundle));
        }

        discover_casks(&mut apps);
        discover_receipts(&mut apps);
        merge_duplicates(&mut apps);

        apps.sort_by_key(|a| a.name.to_lowercase());
        Ok(apps)
    }

    fn analyze(&self, app: &InstalledApp) -> Result<RemovalPlan> {
        let mut candidates: Vec<TraceCandidate> = app
            .install_paths
            .iter()
            .filter(|p| fs::symlink_metadata(p).is_ok())
            .map(|p| TraceCandidate {
                path: p.clone(),
                category: TraceCategory::Application,
                confidence: Confidence::High,
                bytes: measure(p),
                reason: "primary install location".into(),
                primary: true,
            })
            .collect();

        let query = Query {
            identifier: app.identifier.as_deref(),
            name: Some(&app.name),
        };
        let mut traces = find_traces(&search_roots(), &query, &app.install_paths);
        candidates.append(&mut traces);

        candidates.sort_by(|a, b| {
            b.primary
                .cmp(&a.primary)
                .then_with(|| a.path.cmp(&b.path))
        });

        let mut warnings = Vec::new();
        if app.identifier.is_none() && app.method != InstallMethod::PkgReceipt {
            warnings.push(
                "No bundle identifier found — matches are name based. \
                 Review low-confidence items carefully."
                    .into(),
            );
        }

        let home = home();
        let outside_home = candidates
            .iter()
            .any(|c| !c.path.starts_with(&home) && !c.primary);
        if outside_home {
            warnings.push(
                "Some traces are outside your home folder — deleting them may \
                 require an administrator password."
                    .into(),
            );
        }

        let low_confidence = candidates.iter().filter(|c| c.confidence == Confidence::Low).count();
        if low_confidence > 0 {
            warnings.push(format!(
                "{low_confidence} item(s) matched only by name (low confidence) \
                 and are not selected by default."
            ));
        }

        if app.method == InstallMethod::Caskroom {
            warnings.push(
                "Installed via Homebrew — the cask may also need `brew uninstall --cask <name>`."
                    .into(),
            );
        }
        if app.method == InstallMethod::PkgReceipt {
            warnings.push(
                "This is an install receipt. Removing it clears install history; \
                 package contents are not tracked here."
                    .into(),
            );
        }

        let advisories = self.detect(app);
        if !advisories.is_empty() {
            warnings.push(format!(
                "{} system-configuration finding(s) are reported below but are \
                 never changed automatically — review and undo them yourself.",
                advisories.len()
            ));
        }

        let total_bytes = candidates.iter().map(|c| c.bytes).sum();
        Ok(RemovalPlan {
            app: app.clone(),
            candidates,
            advisories,
            total_bytes,
            warnings,
        })
    }

    fn detect(&self, app: &InstalledApp) -> Vec<Advisory> {
        let scans: [fn(&InstalledApp) -> Vec<Advisory>; 7] = [
            scan_shell_env,
            scan_firewall,
            scan_browser_extensions,
            scan_login_items,
            scan_scheduled_tasks,
            scan_extensions,
            scan_permissions,
        ];

        let mut found: Vec<Advisory> = std::thread::scope(|scope| {
            let handles: Vec<_> = scans
                .iter()
                .map(|scan| scope.spawn(move || scan(app)))
                .collect();
            let mut merged = Vec::new();
            for handle in handles {
                if let Ok(mut items) = handle.join() {
                    merged.append(&mut items);
                }
            }
            merged
        });

        found.sort_by(|a, b| {
            a.kind
                .label()
                .cmp(b.kind.label())
                .then_with(|| a.path.cmp(&b.path))
        });
        found.dedup_by(|a, b| a.kind == b.kind && a.path == b.path && a.summary == b.summary);
        found
    }

    fn trace_roots(&self) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = search_roots().into_iter().map(|r| r.path).collect();
        roots.push(PathBuf::from("/Applications"));
        roots.push(h("Applications"));
        roots.push(PathBuf::from("/opt/homebrew/Caskroom"));
        roots.push(PathBuf::from("/usr/local/Caskroom"));
        roots.push(PathBuf::from("/var/db/receipts"));
        roots.sort();
        roots.dedup();
        roots
    }
}

// ---------------------------------------------------------------------------
// System-change detection (report only — never auto-edited)
// ---------------------------------------------------------------------------

/// Runs a read-only command, returning stdout on success. Returns `None` when
/// the binary is missing or the command fails.
fn run_read(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).to_string())
}

/// Lower-cased strings that identify the app inside config text.
fn app_needles(app: &InstalledApp) -> Vec<String> {
    let mut needles = vec![app.name.to_lowercase()];
    if let Some(id) = &app.identifier {
        needles.push(id.to_lowercase());
    }
    for path in &app.install_paths {
        needles.push(path.to_string_lossy().to_lowercase());
    }
    needles.retain(|n| !n.trim().is_empty());
    needles
}

fn references(haystack: &str, needles: &[String]) -> bool {
    let lower = haystack.to_lowercase();
    needles.iter().any(|n| lower.contains(n.as_str()))
}

/// Numbered lines from `text` that mention any needle (pure, unit-tested).
fn matching_lines(text: &str, needles: &[String]) -> Vec<String> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| references(line, needles))
        .map(|(i, line)| format!("{}: {}", i + 1, line.trim()))
        .take(20)
        .collect()
}

const SHELL_RC: &[&str] = &[
    ".zshrc",
    ".zprofile",
    ".zshenv",
    ".bashrc",
    ".bash_profile",
    ".profile",
];

fn scan_shell_env(app: &InstalledApp) -> Vec<Advisory> {
    let needles = app_needles(app);
    let mut found = Vec::new();

    for rel in SHELL_RC {
        let path = home().join(rel);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let hits = matching_lines(&text, &needles);
        if !hits.is_empty() {
            found.push(Advisory {
                kind: AdvisoryKind::EnvironmentVariable,
                summary: format!("{} line(s) in {rel} mention this app", hits.len()),
                detail: format!(
                    "Shell config still references this app (PATH entries, aliases, env vars):\n{}",
                    hits.join("\n")
                ),
                command: Some(format!("${{EDITOR:-vi}} \"{}\"", path.display())),
                path: Some(path),
            });
        }
    }

    for dir in ["/etc/paths.d", "/etc/profile.d"] {
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            if references(&text, &needles) {
                found.push(Advisory {
                    kind: AdvisoryKind::EnvironmentVariable,
                    summary: format!("System path entry {} references this app", path.display()),
                    detail: text.trim().to_string(),
                    command: Some(format!("sudo rm \"{}\"", path.display())),
                    path: Some(path),
                });
            }
        }
    }

    found
}

fn scan_firewall(app: &InstalledApp) -> Vec<Advisory> {
    let needles = app_needles(app);
    let mut found = Vec::new();

    if let Some(listing) = run_read(
        "/usr/libexec/ApplicationFirewall/socketfilterfw",
        &["--listapps"],
    ) {
        for line in listing.lines() {
            if let Some(app_path) = line.trim().strip_prefix("ALF: Application:") {
                let app_path = app_path.trim();
                if references(app_path, &needles) {
                    found.push(Advisory {
                        kind: AdvisoryKind::FirewallRule,
                        summary: format!("Application firewall entry for {app_path}"),
                        detail: "The macOS application firewall has an explicit allow/deny rule for this app."
                            .into(),
                        command: Some(format!(
                            "sudo /usr/libexec/ApplicationFirewall/socketfilterfw --remove \"{app_path}\""
                        )),
                        path: Some(PathBuf::from(app_path)),
                    });
                }
            }
        }
    }

    if let Ok(text) = fs::read_to_string("/etc/pf.conf") {
        let hits = matching_lines(&text, &needles);
        if !hits.is_empty() {
            found.push(Advisory {
                kind: AdvisoryKind::FirewallRule,
                summary: "Referenced in the pf firewall config".into(),
                detail: hits.join("\n"),
                command: Some("sudo \"${EDITOR:-vi}\" /etc/pf.conf && sudo pfctl -f /etc/pf.conf".into()),
                path: Some(PathBuf::from("/etc/pf.conf")),
            });
        }
    }

    found
}

const BROWSER_ROOTS: &[(&str, &str)] = &[
    ("Library/Application Support/Google/Chrome", "Chrome"),
    ("Library/Application Support/Chromium", "Chromium"),
    ("Library/Application Support/BraveSoftware/Brave-Browser", "Brave"),
    ("Library/Application Support/Microsoft Edge", "Edge"),
];

fn scan_browser_extensions(app: &InstalledApp) -> Vec<Advisory> {
    let needles = app_needles(app);
    let mut found = Vec::new();

    for (rel, browser) in BROWSER_ROOTS {
        let root = h(rel);
        if let Ok(profiles) = fs::read_dir(&root) {
            for profile in profiles.flatten() {
                let ext_root = profile.path().join("Extensions");
                let Ok(extensions) = fs::read_dir(&ext_root) else {
                    continue;
                };
                for extension in extensions.flatten() {
                    let Ok(versions) = fs::read_dir(extension.path()) else {
                        continue;
                    };
                    for version in versions.flatten() {
                        let manifest = version.path().join("manifest.json");
                        let Ok(text) = fs::read_to_string(&manifest) else {
                            continue;
                        };
                        if references(&text, &needles) {
                            found.push(Advisory {
                                kind: AdvisoryKind::BrowserExtension,
                                summary: format!(
                                    "{browser} extension under {} references this app",
                                    profile.file_name().to_string_lossy()
                                ),
                                detail: format!(
                                    "Extension: {}\n{}",
                                    extension.file_name().to_string_lossy(),
                                    text.chars().take(400).collect::<String>()
                                ),
                                command: None,
                                path: Some(manifest),
                            });
                        }
                    }
                }
            }
        }

        let native_hosts = root.join("NativeMessagingHosts");
        if let Ok(entries) = fs::read_dir(&native_hosts) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                if references(&text, &needles) || references(&name, &needles) {
                    found.push(Advisory {
                        kind: AdvisoryKind::BrowserExtension,
                        summary: format!("{browser} native messaging host {name}"),
                        detail: text.trim().to_string(),
                        command: Some(format!("rm \"{}\"", path.display())),
                        path: Some(path),
                    });
                }
            }
        }
    }

    // Firefox profiles keep extension dirs / xpi files by id.
    let profiles = h("Library/Application Support/Firefox/Profiles");
    if let Ok(entries) = fs::read_dir(&profiles) {
        for profile in entries.flatten() {
            let extensions = profile.path().join("extensions");
            let Ok(list) = fs::read_dir(&extensions) else {
                continue;
            };
            for extension in list.flatten() {
                let name = extension.file_name().to_string_lossy().to_string();
                if references(&name, &needles) {
                    found.push(Advisory {
                        kind: AdvisoryKind::BrowserExtension,
                        summary: format!("Firefox extension {name}"),
                        detail: format!(
                            "Extension in profile {}",
                            profile.file_name().to_string_lossy()
                        ),
                        command: Some(format!("rm -rf \"{}\"", extension.path().display())),
                        path: Some(extension.path()),
                    });
                }
            }
        }
    }

    found
}

fn scan_login_items(app: &InstalledApp) -> Vec<Advisory> {
    let needles = app_needles(app);
    let Some(items) = run_read(
        "osascript",
        &[
            "-e",
            "tell application \"System Events\" to get the name of every login item",
        ],
    ) else {
        return Vec::new();
    };
    if !references(&items, &needles) {
        return Vec::new();
    }
    vec![Advisory {
        kind: AdvisoryKind::LoginItem,
        summary: "Registered as a login item".into(),
        detail: format!("Login items: {}", items.trim()),
        command: None,
        path: None,
    }]
}

fn scan_scheduled_tasks(app: &InstalledApp) -> Vec<Advisory> {
    let needles = app_needles(app);
    let Some(cron) = run_read("crontab", &["-l"]) else {
        return Vec::new();
    };
    let hits = matching_lines(&cron, &needles);
    if hits.is_empty() {
        return Vec::new();
    }
    vec![Advisory {
        kind: AdvisoryKind::ScheduledTask,
        summary: format!("{} cron entr(y/ies) reference this app", hits.len()),
        detail: hits.join("\n"),
        command: Some("crontab -e".into()),
        path: None,
    }]
}

fn scan_extensions(app: &InstalledApp) -> Vec<Advisory> {
    let needles = app_needles(app);
    let mut found = Vec::new();

    if let Some(list) = run_read("systemextensionsctl", &["list"]) {
        for line in matching_lines(&list, &needles) {
            found.push(Advisory {
                kind: AdvisoryKind::KernelExtension,
                summary: "System extension installed".into(),
                detail: line,
                command: Some("systemextensionsctl list".into()),
                path: None,
            });
        }
    }

    if let Some(list) = run_read("kmutil", &["showloaded"]) {
        for line in matching_lines(&list, &needles) {
            found.push(Advisory {
                kind: AdvisoryKind::KernelExtension,
                summary: "Legacy kernel extension loaded".into(),
                detail: line,
                command: Some("kmutil showloaded".into()),
                path: None,
            });
        }
    }

    found
}

fn scan_permissions(app: &InstalledApp) -> Vec<Advisory> {
    let mut found = Vec::new();
    for path in &app.install_paths {
        let Ok(target) = path.canonicalize() else {
            continue;
        };
        let Some(output) = run_read("ls", &["-lde", &target.to_string_lossy()]) else {
            continue;
        };
        let acl_lines: Vec<&str> = output
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                match trimmed.split_once(':') {
                    Some((number, _)) => {
                        !number.is_empty() && number.chars().all(|c| c.is_ascii_digit())
                    }
                    None => false,
                }
            })
            .collect();
        if !acl_lines.is_empty() {
            found.push(Advisory {
                kind: AdvisoryKind::Permission,
                summary: format!("Custom ACL on {}", target.display()),
                detail: acl_lines.join("\n"),
                command: Some(format!("chmod -N \"{}\"", target.display())),
                path: Some(target),
            });
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_lines_finds_and_numbers_references() {
        let text = "export PATH=/opt/CoolApp/bin:$PATH\nother=1\n# CoolApp alias\nalias ca=CoolApp\n";
        let needles = vec!["coolapp".to_string()];
        let hits = matching_lines(text, &needles);
        assert_eq!(hits.len(), 3);
        assert!(hits[0].starts_with("1:"));
        assert!(hits[2].contains("alias ca=CoolApp"));
    }

    #[test]
    fn matching_lines_ignores_unrelated_text() {
        let needles = vec!["coolapp".to_string()];
        assert!(matching_lines("nothing here\n", &needles).is_empty());
    }

    #[test]
    fn info_plist_is_read_from_a_real_bundle() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/Demo.app");
        if !fixtures.exists() {
            return;
        }
        let info = read_info_plist(&fixtures);
        assert_eq!(info.identifier.as_deref(), Some("com.example.demo"));
        assert_eq!(info.name.as_deref(), Some("Demo"));
        assert_eq!(info.version.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn discovery_finds_bundles_and_is_sorted() {
        let apps = MacosBackend.discover().expect("discovery should not error");
        assert!(!apps.is_empty(), "expected at least one app on this machine");
        let names: Vec<String> = apps.iter().map(|a| a.name.to_lowercase()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "discovery must return apps sorted by name");
    }
}
