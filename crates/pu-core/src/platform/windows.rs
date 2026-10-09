use std::path::PathBuf;

use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

use crate::error::Result;
use crate::models::{
    Confidence, InstallMethod, InstalledApp, Platform, RemovalPlan, TraceCandidate, TraceCategory,
};
use crate::platform::PlatformBackend;
use crate::trace_search::{find_traces, Query, SearchRoot};
use crate::util::estimate_size;

pub struct WindowsBackend;

const UNINSTALL_PATH: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
const UNINSTALL_PATH_WOW64: &str =
    r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn search_roots() -> Vec<SearchRoot> {
    let mut roots = Vec::new();
    if let Some(roaming) = env_path("APPDATA") {
        roots.push(SearchRoot {
            path: roaming,
            category: TraceCategory::Support,
            depth: 1,
            label: "%APPDATA%",
        });
    }
    if let Some(local) = env_path("LOCALAPPDATA") {
        roots.push(SearchRoot {
            path: local,
            category: TraceCategory::Cache,
            depth: 1,
            label: "%LOCALAPPDATA%",
        });
    }
    if let Some(program_data) = env_path("PROGRAMDATA") {
        roots.push(SearchRoot {
            path: program_data,
            category: TraceCategory::Support,
            depth: 1,
            label: "%PROGRAMDATA%",
        });
    }
    roots
}

fn collect_uninstall_key(hive: RegKey, path: &str, out: &mut Vec<InstalledApp>) {
    let Ok(key) = hive.open_subkey(path) else {
        return;
    };
    for name in key.enum_keys().flatten() {
        let Ok(sub) = key.open_subkey(&name) else {
            continue;
        };
        let display: String = sub.get_value("DisplayName").unwrap_or_default();
        if display.trim().is_empty() {
            continue;
        }
        if sub.get_value::<u32, _>("SystemComponent").unwrap_or(0) == 1 {
            continue;
        }
        if sub.get_value::<String, _>("ParentKeyName").is_ok() {
            continue;
        }

        let uninstall: String = sub.get_value("UninstallString").unwrap_or_default();
        let method = if uninstall.to_lowercase().contains("msiexec") {
            InstallMethod::Msi
        } else {
            InstallMethod::Unknown
        };

        let mut install_paths = Vec::new();
        if let Ok(location) = sub.get_value::<String, _>("InstallLocation") {
            let location = location.trim().trim_matches('"').to_string();
            if !location.is_empty() {
                let path = PathBuf::from(&location);
                if path.is_dir() {
                    install_paths.push(path);
                }
            }
        }
        if let Ok(icon) = sub.get_value::<String, _>("DisplayIcon") {
            let icon_path = icon
                .split(',')
                .next()
                .unwrap_or(&icon)
                .trim()
                .trim_matches('"');
            let path = PathBuf::from(icon_path);
            if let Some(parent) = path.parent() {
                if parent.is_dir() && !install_paths.contains(&parent.to_path_buf()) {
                    install_paths.push(parent.to_path_buf());
                }
            }
        }

        out.push(InstalledApp {
            id: name,
            name: display.trim().to_string(),
            identifier: sub.get_value("BundleIdentifier").ok(),
            version: sub.get_value("DisplayVersion").ok(),
            install_paths,
            method,
            platform: Platform::Windows,
            install_bytes: 0,
        });
    }
}

impl PlatformBackend for WindowsBackend {
    fn platform(&self) -> Platform {
        Platform::Windows
    }

    fn discover(&self) -> Result<Vec<InstalledApp>> {
        let mut apps = Vec::new();
        collect_uninstall_key(
            RegKey::predef(HKEY_LOCAL_MACHINE),
            UNINSTALL_PATH,
            &mut apps,
        );
        collect_uninstall_key(
            RegKey::predef(HKEY_LOCAL_MACHINE),
            UNINSTALL_PATH_WOW64,
            &mut apps,
        );
        collect_uninstall_key(RegKey::predef(HKEY_CURRENT_USER), UNINSTALL_PATH, &mut apps);
        apps.sort_by_key(|a| a.name.to_lowercase());
        Ok(apps)
    }

    fn analyze(&self, app: &InstalledApp) -> Result<RemovalPlan> {
        let mut candidates: Vec<TraceCandidate> = app
            .install_paths
            .iter()
            .filter(|p| std::fs::symlink_metadata(p).is_ok())
            .map(|p| TraceCandidate {
                path: p.clone(),
                category: TraceCategory::Application,
                confidence: Confidence::High,
                bytes: estimate_size(p),
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
        candidates.sort_by(|a, b| b.primary.cmp(&a.primary).then_with(|| a.path.cmp(&b.path)));

        let mut warnings = Vec::new();
        if app.identifier.is_none() {
            warnings.push(
                "No package identifier — matches are name based. \
                 Review low-confidence items carefully."
                    .into(),
            );
        }
        if app.method == InstallMethod::Msi {
            warnings.push(
                "MSI-installed: its registry uninstall entry is left alone. \
                 Use Settings > Apps for a fully registered uninstall."
                    .into(),
            );
        }
        let low_confidence = candidates
            .iter()
            .filter(|c| c.confidence == Confidence::Low)
            .count();
        if low_confidence > 0 {
            warnings.push(format!(
                "{low_confidence} item(s) matched only by name (low confidence) \
                 and are not selected by default."
            ));
        }

        let total_bytes = candidates.iter().map(|c| c.bytes).sum();
        Ok(RemovalPlan {
            app: app.clone(),
            candidates,
            advisories: Vec::new(),
            total_bytes,
            warnings,
        })
    }

    fn trace_roots(&self) -> Vec<PathBuf> {
        search_roots().into_iter().map(|r| r.path).collect()
    }
}
