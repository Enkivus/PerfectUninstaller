use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::models::{
    Confidence, InstallMethod, InstalledApp, Platform, RemovalPlan, TraceCandidate, TraceCategory,
};
use crate::platform::PlatformBackend;
use crate::safety::ensure_safe;
use crate::trace_search::{find_traces, Query, SearchRoot};
use crate::util::measure;

pub struct LinuxBackend;

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn h(rel: &str) -> PathBuf {
    home().join(rel)
}

fn search_roots() -> Vec<SearchRoot> {
    vec![
        SearchRoot {
            path: h(".config"),
            category: TraceCategory::Preference,
            depth: 1,
            label: "~/.config",
        },
        SearchRoot {
            path: h(".local/share"),
            category: TraceCategory::Support,
            depth: 1,
            label: "~/.local/share",
        },
        SearchRoot {
            path: h(".cache"),
            category: TraceCategory::Cache,
            depth: 1,
            label: "~/.cache",
        },
        SearchRoot {
            path: h(".var/app"),
            category: TraceCategory::Container,
            depth: 0,
            label: "~/.var/app",
        },
        SearchRoot {
            path: h(".local/share/flatpak/app"),
            category: TraceCategory::Support,
            depth: 0,
            label: "~/.local/share/flatpak/app",
        },
        SearchRoot {
            path: h(".local/share/Steam/steamapps/common"),
            category: TraceCategory::Support,
            depth: 0,
            label: "~/.local/share/Steam/steamapps/common",
        },
        SearchRoot {
            path: PathBuf::from("/var/tmp"),
            category: TraceCategory::Cache,
            depth: 1,
            label: "/var/tmp",
        },
    ]
}

/// Parses a freedesktop `.desktop` entry.
struct DesktopEntry {
    name: String,
    exec: Option<String>,
    hidden: bool,
}

fn parse_desktop(path: &Path) -> Option<DesktopEntry> {
    let text = fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    let mut name = None;
    let mut exec = None;
    let mut hidden = false;

    for line in text.lines() {
        let line = line.trim_end();
        if line.starts_with('[') {
            if in_entry {
                break;
            }
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some(v) = line.strip_prefix("Name=") {
            name.get_or_insert(v.to_string());
        } else if let Some(v) = line.strip_prefix("Exec=") {
            exec.get_or_insert(v.to_string());
        } else if let Some(v) = line.strip_prefix("Hidden=") {
            hidden = v.eq_ignore_ascii_case("true");
        }
    }

    Some(DesktopEntry { name: name?, exec, hidden })
}

/// Absolute executable from an Exec= line (`/opt/foo/bin/app --flag %U`).
fn exec_path(exec: &str) -> Option<PathBuf> {
    let token = exec.split_whitespace().next()?;
    let token = token.trim_matches('"');
    if !token.starts_with('/') {
        return None;
    }
    let path = PathBuf::from(token);
    path.exists().then_some(path)
}

fn discover_desktop_files(dir: &Path, method: InstallMethod, out: &mut Vec<InstalledApp>) {
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "desktop") {
            continue;
        }
        let Some(parsed) = parse_desktop(&path) else {
            continue;
        };
        if parsed.hidden {
            continue;
        }
        let mut install_paths = Vec::new();
        if let Some(bin) = parsed.exec.as_deref().and_then(exec_path) {
            if ensure_safe(&bin).is_ok() {
                install_paths.push(bin);
            }
        }
        out.push(InstalledApp {
            id: path.to_string_lossy().to_string(),
            name: parsed.name,
            identifier: None,
            version: None,
            install_paths,
            method,
            platform: Platform::Linux,
            install_bytes: 0,
        });
    }
}

fn discover_flatpak(out: &mut Vec<InstalledApp>) {
    for root in [h(".local/share/flatpak/app"), PathBuf::from("/var/lib/flatpak/app")] {
        let Ok(read) = fs::read_dir(&root) else {
            continue;
        };
        for entry in read.flatten() {
            let id = entry.file_name().to_string_lossy().to_string();
            if !id.contains('.') {
                continue;
            }
            let name = id.rsplit('.').next().unwrap_or(&id).to_string();
            out.push(InstalledApp {
                id: format!("flatpak:{id}"),
                name,
                identifier: Some(id.clone()),
                version: None,
                install_paths: Vec::new(),
                method: InstallMethod::Flatpak,
                platform: Platform::Linux,
                install_bytes: 0,
            });
        }
    }
}

impl PlatformBackend for LinuxBackend {
    fn platform(&self) -> Platform {
        Platform::Linux
    }

    fn discover(&self) -> Result<Vec<InstalledApp>> {
        let mut apps = Vec::new();
        for (dir, method) in [
            (h(".local/share/applications"), InstallMethod::Manual),
            (PathBuf::from("/usr/share/applications"), InstallMethod::Package),
            (PathBuf::from("/usr/local/share/applications"), InstallMethod::Package),
            (PathBuf::from("/var/lib/snapd/desktop/applications"), InstallMethod::Snap),
        ] {
            discover_desktop_files(&dir, method, &mut apps);
        }
        discover_flatpak(&mut apps);
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
        candidates.sort_by(|a, b| b.primary.cmp(&a.primary).then_with(|| a.path.cmp(&b.path)));

        let mut warnings = Vec::new();
        match app.method {
            InstallMethod::Package => warnings.push(
                "Installed as a system package — use your package manager \
                 (apt/dnf/pacman) to remove files under /usr."
                    .into(),
            ),
            InstallMethod::Flatpak => warnings.push(
                "Flatpak app — `flatpak uninstall <id>` removes the app itself; \
                 residual user data is listed below."
                    .into(),
            ),
            InstallMethod::Snap => warnings.push(
                "Snap app — `snap remove <name>` removes the app itself; \
                 residual user data is listed below."
                    .into(),
            ),
            _ => {}
        }
        if app.identifier.is_none() {
            warnings.push(
                "No desktop/app identifier — matches are name based. \
                 Review low-confidence items carefully."
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

        let total_bytes = candidates.iter().map(|c| c.bytes).sum();
        Ok(RemovalPlan {
            app: app.clone(),
            candidates,
            total_bytes,
            warnings,
        })
    }

    fn trace_roots(&self) -> Vec<PathBuf> {
        search_roots().into_iter().map(|r| r.path).collect()
    }
}
