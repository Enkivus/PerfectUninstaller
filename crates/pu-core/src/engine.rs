use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::error::{Error, Result};
use crate::models::{Confidence, InstalledApp, Platform, RemovalPlan, RemovalReport};
use crate::platform::{self, PlatformBackend};
use crate::safety::{ensure_safe, ensure_within_roots};
use crate::util::measure;

/// High level API: scan → analyze → uninstall.
pub struct Engine {
    backend: Box<dyn PlatformBackend>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            backend: platform::backend(),
        }
    }

    pub fn platform(&self) -> Platform {
        self.backend.platform()
    }

    pub fn scan(&self) -> Result<Vec<InstalledApp>> {
        let mut apps = self.backend.discover()?;
        apps.sort_by_key(|a| a.name.to_lowercase());
        Ok(apps)
    }

    pub fn analyze(&self, app: &InstalledApp) -> Result<RemovalPlan> {
        self.backend.analyze(app)
    }

    /// What gets ticked by default: the install locations plus every
    /// high/medium confidence trace. Low confidence needs an explicit opt-in.
    pub fn default_selection(plan: &RemovalPlan) -> Vec<PathBuf> {
        plan.candidates
            .iter()
            .filter(|c| c.primary || c.confidence >= Confidence::Medium)
            .map(|c| c.path.clone())
            .collect()
    }

    /// Permanently deletes every selected path.
    ///
    /// Refuses anything that is not explicitly inside the platform's known
    /// trace roots or the app's install paths, and writes a JSONL audit log
    /// before touching anything.
    pub fn uninstall(
        &self,
        plan: &RemovalPlan,
        selected: &[PathBuf],
        progress: &mut dyn FnMut(&Path, usize, usize),
    ) -> Result<RemovalReport> {
        let started = Instant::now();

        let mut roots = self.backend.trace_roots();
        roots.extend(plan.app.install_paths.iter().cloned());

        let (audit_path, mut audit) = create_audit_log(plan, selected)?;

        let mut report = RemovalReport {
            removed: Vec::new(),
            bytes_freed: 0,
            failed: Vec::new(),
            refused: Vec::new(),
            audit_log: Some(audit_path),
            report_dir: None,
            elapsed_ms: 0,
        };

        let mut ordered: Vec<PathBuf> = selected.to_vec();
        ordered.sort();
        let total = ordered.len();

        for (index, path) in ordered.iter().enumerate() {
            progress(path, index + 1, total);

            if let Err(err) = ensure_safe(path).and_then(|_| ensure_within_roots(path, &roots)) {
                report.refused.push(path.clone());
                log_line(&mut audit, &serde_json::json!({
                    "type": "delete",
                    "path": path.display().to_string(),
                    "status": "refused",
                    "error": err.to_string(),
                }));
                continue;
            }

            let bytes = measure(path);
            match remove_path(path) {
                Ok(()) => {
                    report.removed.push(path.clone());
                    report.bytes_freed += bytes;
                    log_line(&mut audit, &serde_json::json!({
                        "type": "delete",
                        "path": path.display().to_string(),
                        "bytes": bytes,
                        "status": "removed",
                    }));
                }
                Err(message) if message.contains("No such file") => {
                    log_line(&mut audit, &serde_json::json!({
                        "type": "delete",
                        "path": path.display().to_string(),
                        "status": "already_gone",
                    }));
                }
                Err(message) => {
                    report.failed.push(crate::models::RemovalFailure {
                        path: path.clone(),
                        message: message.clone(),
                    });
                    log_line(&mut audit, &serde_json::json!({
                        "type": "delete",
                        "path": path.display().to_string(),
                        "status": "failed",
                        "error": message,
                    }));
                }
            }
        }

        log_line(
            &mut audit,
            &serde_json::json!({
                "type": "summary",
                "removed": report.removed.len(),
                "refused": report.refused.len(),
                "failed": report.failed.len(),
                "bytes_freed": report.bytes_freed,
                "elapsed_ms": started.elapsed().as_millis(),
            }),
        );
        let _ = audit.flush();

        report.elapsed_ms = started.elapsed().as_millis();

        // Best-effort: export the HTML/JSON/JSONL report next to the audit log.
        if let Ok(dir) = crate::report::default_report_dir(&plan.app.name) {
            if let Ok(exported) = crate::report::export(&dir, plan, &report) {
                report.report_dir = Some(exported.dir);
            }
        }

        Ok(report)
    }

    /// Re-exports (or exports on demand) the report for a finished run.
    pub fn export_report(
        &self,
        plan: &RemovalPlan,
        report: &RemovalReport,
    ) -> Result<crate::report::ExportedReport> {
        let dir = crate::report::default_report_dir(&plan.app.name)?;
        crate::report::export(&dir, plan, report)
    }
}

fn remove_path(path: &Path) -> std::result::Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if meta.is_dir() {
        fs::remove_dir_all(path).map_err(|e| e.to_string())
    } else {
        fs::remove_file(path).map_err(|e| e.to_string())
    }
}

fn audit_dir() -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| Error::Other("HOME is not set".into()))?;
    let dir = home.join(".perfectuninstaller").join("audit");
    fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
    Ok(dir)
}

fn create_audit_log(plan: &RemovalPlan, selected: &[PathBuf]) -> Result<(PathBuf, BufWriter<File>)> {
    let dir = audit_dir()?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S-%f");
    let path = dir.join(format!("uninstall-{stamp}.jsonl"));
    let mut file = BufWriter::new(File::create(&path).map_err(|e| Error::io(&path, e))?);
    writeln!(
        file,
        "{}",
        serde_json::json!({
            "type": "plan",
            "timestamp": chrono::Local::now().to_rfc3339(),
            "app": plan.app.name,
            "identifier": plan.app.identifier,
            "method": plan.app.method.label(),
            "selected": selected.len(),
            "planned_paths": selected.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
        })
    )
    .map_err(|e| Error::io(&path, e))?;
    let _ = file.flush();
    Ok((path, file))
}

fn log_line<W: Write>(writer: &mut W, value: &serde_json::Value) {
    if let Ok(line) = serde_json::to_string(value) {
        let _ = writeln!(writer, "{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{InstallMethod, TraceCategory};

    fn fake_plan(root: &Path) -> RemovalPlan {
        let app = InstalledApp {
            id: "test".into(),
            name: "Test".into(),
            identifier: Some("com.example.test".into()),
            version: None,
            install_paths: vec![root.join("Test.app")],
            method: InstallMethod::AppBundle,
            platform: Platform::Macos,
            install_bytes: 0,
        };
        RemovalPlan {
            app,
            candidates: vec![crate::models::TraceCandidate {
                path: root.join("Test.app"),
                category: TraceCategory::Application,
                confidence: Confidence::High,
                bytes: 0,
                reason: "primary".into(),
                primary: true,
            }],
            advisories: vec![],
            total_bytes: 0,
            warnings: vec![],
        }
    }

    #[test]
    fn default_selection_skips_low_confidence() {
        let plan = RemovalPlan {
            app: InstalledApp {
                id: "a".into(),
                name: "A".into(),
                identifier: None,
                version: None,
                install_paths: vec![],
                method: InstallMethod::Unknown,
                platform: Platform::Linux,
                install_bytes: 0,
            },
            candidates: vec![
                crate::models::TraceCandidate {
                    path: PathBuf::from("/a"),
                    category: TraceCategory::Application,
                    confidence: Confidence::High,
                    bytes: 0,
                    reason: String::new(),
                    primary: true,
                },
                crate::models::TraceCandidate {
                    path: PathBuf::from("/b"),
                    category: TraceCategory::Cache,
                    confidence: Confidence::Medium,
                    bytes: 0,
                    reason: String::new(),
                    primary: false,
                },
                crate::models::TraceCandidate {
                    path: PathBuf::from("/c"),
                    category: TraceCategory::Cache,
                    confidence: Confidence::Low,
                    bytes: 0,
                    reason: String::new(),
                    primary: false,
                },
            ],
            advisories: vec![],
            total_bytes: 0,
            warnings: vec![],
        };
        let selection = Engine::default_selection(&plan);
        assert!(selection.contains(&PathBuf::from("/a")));
        assert!(selection.contains(&PathBuf::from("/b")));
        assert!(!selection.contains(&PathBuf::from("/c")));
    }

    #[test]
    fn refuses_paths_outside_known_roots() {
        let tmp = std::env::temp_dir().join(format!("pu-engine-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let plan = fake_plan(&tmp);
        let engine = Engine::new();
        let escape = PathBuf::from("/etc/hosts");
        let report = engine
            .uninstall(&plan, std::slice::from_ref(&escape), &mut |_, _, _| {})
            .expect("uninstall runs and reports");
        assert!(report.removed.is_empty());
        assert_eq!(report.refused.len(), 1, "unsafe path must be refused");
        assert!(Path::new("/etc/hosts").exists(), "system file untouched");

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn deletes_selected_paths_and_audits() {
        let tmp = std::env::temp_dir().join(format!("pu-engine-del-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let victim = tmp.join("Test.app");
        fs::create_dir_all(victim.join("Contents")).unwrap();
        fs::write(victim.join("Contents/binary"), "data").unwrap();

        let plan = fake_plan(&tmp);
        let engine = Engine::new();
        let mut seen = Vec::new();
        let report = engine
            .uninstall(&plan, std::slice::from_ref(&victim), &mut |p, i, t| {
                seen.push((p.to_path_buf(), i, t));
            })
            .unwrap();

        assert!(!victim.exists(), "victim must be gone");
        assert_eq!(report.removed.len(), 1);
        assert!(report.failed.is_empty(), "failures: {:?}", report.failed);
        assert_eq!(seen.len(), 1);

        let audit = report.audit_log.expect("audit log written");
        let contents = fs::read_to_string(&audit).unwrap();
        assert!(contents.contains("\"type\":\"plan\""));
        assert!(contents.contains("\"type\":\"summary\""));
        let _ = fs::remove_dir_all(&tmp);
        let _ = fs::remove_file(audit);
    }
}
