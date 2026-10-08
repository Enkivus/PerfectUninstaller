use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::models::{
    Confidence, InstallMethod, InstalledApp, Platform, RemovalPlan, TraceCandidate,
    TraceCategory,
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

        let total_bytes = candidates.iter().map(|c| c.bytes).sum();
        Ok(RemovalPlan { app: app.clone(), candidates, total_bytes, warnings })
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

#[cfg(test)]
mod tests {
    use super::*;

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
