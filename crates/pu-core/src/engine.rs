use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::error::{Error, Result};
use crate::models::{Confidence, InstalledApp, Platform, RemovalPlan, RemovalReport};
use crate::platform::{self, PlatformBackend};
use crate::safety::{
    ensure_no_symlink_escape, ensure_safe_install_target, ensure_within_roots, same_path,
};
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
        let mut plan = self.backend.analyze(app)?;
        let roots = self.allowed_roots(&plan);
        let before = plan.candidates.len();
        plan.candidates.retain(|candidate| {
            ensure_safe_install_target(&candidate.path, &plan.app.install_paths)
                .and_then(|_| ensure_within_roots(&candidate.path, &roots))
                .and_then(|_| ensure_no_symlink_escape(&candidate.path, &roots))
                .is_ok()
        });
        let refused = before - plan.candidates.len();
        if refused > 0 {
            plan.warnings.push(format!(
                "{refused} potential item(s) were omitted because they did not pass the removal safety checks."
            ));
        }
        plan.total_bytes = plan
            .candidates
            .iter()
            .map(|candidate| candidate.bytes)
            .sum();
        Ok(plan)
    }

    fn allowed_roots(&self, plan: &RemovalPlan) -> Vec<PathBuf> {
        let mut roots = self.backend.trace_roots();
        roots.extend(plan.app.install_paths.iter().cloned());
        roots
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

        let roots = self.allowed_roots(plan);

        let (audit_path, mut audit) = create_audit_log(plan, selected)?;

        let mut report = RemovalReport {
            removed: Vec::new(),
            bytes_freed: 0,
            failed: Vec::new(),
            refused: Vec::new(),
            audit_log: Some(audit_path.clone()),
            report_dir: None,
            elapsed_ms: 0,
        };

        let mut ordered: Vec<PathBuf> = selected.to_vec();
        ordered.sort();
        let total = ordered.len();

        for (index, path) in ordered.iter().enumerate() {
            progress(path, index + 1, total);

            let planned = plan
                .candidates
                .iter()
                .any(|candidate| same_path(&candidate.path, path));
            let guard = if planned {
                ensure_safe_install_target(path, &plan.app.install_paths)
                    .and_then(|_| ensure_within_roots(path, &roots))
                    .and_then(|_| ensure_no_symlink_escape(path, &roots))
            } else {
                Err(Error::UnsafePath(path.clone()))
            };
            if let Err(err) = guard {
                report.refused.push(path.clone());
                log_line(
                    &mut audit,
                    &serde_json::json!({
                        "type": "delete",
                        "path": path.display().to_string(),
                        "status": "refused",
                        "error": err.to_string(),
                    }),
                )
                .and_then(|_| audit.flush())
                .map_err(|error| Error::io(&audit_path, error))?;
                continue;
            }

            let bytes = measure(path);
            match remove_path(path) {
                Ok(()) => {
                    report.removed.push(path.clone());
                    report.bytes_freed += bytes;
                    log_line(
                        &mut audit,
                        &serde_json::json!({
                            "type": "delete",
                            "path": path.display().to_string(),
                            "bytes": bytes,
                            "status": "removed",
                        }),
                    )
                    .and_then(|_| audit.flush())
                    .map_err(|error| Error::io(&audit_path, error))?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    log_line(
                        &mut audit,
                        &serde_json::json!({
                            "type": "delete",
                            "path": path.display().to_string(),
                            "status": "already_gone",
                        }),
                    )
                    .and_then(|_| audit.flush())
                    .map_err(|error| Error::io(&audit_path, error))?;
                }
                Err(error) => {
                    let message = error.to_string();
                    report.failed.push(crate::models::RemovalFailure {
                        path: path.clone(),
                        message: message.clone(),
                    });
                    log_line(
                        &mut audit,
                        &serde_json::json!({
                            "type": "delete",
                            "path": path.display().to_string(),
                            "status": "failed",
                            "error": message,
                        }),
                    )
                    .and_then(|_| audit.flush())
                    .map_err(|error| Error::io(&audit_path, error))?;
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
        )
        .and_then(|_| audit.flush())
        .map_err(|error| Error::io(&audit_path, error))?;

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

fn remove_path(path: &Path) -> std::io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

fn audit_dir() -> Result<PathBuf> {
    let home = crate::util::home_dir()
        .ok_or_else(|| Error::Other("user home directory is not set".into()))?;
    let dir = home.join(".perfectuninstaller").join("audit");
    fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
    Ok(dir)
}

fn create_audit_log(
    plan: &RemovalPlan,
    selected: &[PathBuf],
) -> Result<(PathBuf, BufWriter<File>)> {
    let dir = audit_dir()?;
    let (path, file) = loop {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S-%f");
        let path = dir.join(format!("uninstall-{stamp}.jsonl"));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => break (path, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(Error::io(&path, error)),
        }
    };
    let mut file = BufWriter::new(file);
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
    file.flush().map_err(|error| Error::io(&path, error))?;
    Ok((path, file))
}

fn log_line<W: Write>(writer: &mut W, value: &serde_json::Value) -> std::io::Result<()> {
    let line = serde_json::to_string(value).map_err(std::io::Error::other)?;
    writeln!(writer, "{line}")
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

        let tmp = fs::canonicalize(tmp).unwrap();
        let plan = fake_plan(&tmp);
        let engine = Engine::new();
        let escape = PathBuf::from("/etc/hosts");
        let report = engine
            .uninstall(&plan, std::slice::from_ref(&escape), &mut |_, _, _| {})
            .expect("uninstall runs and reports");
        assert!(report.removed.is_empty());
        assert_eq!(report.refused.len(), 1, "unsafe path must be refused");
        assert!(!report.success(), "refused removal is not a full success");
        assert!(Path::new("/etc/hosts").exists(), "system file untouched");

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn refuses_paths_that_are_not_in_the_plan() {
        let tmp = std::env::temp_dir().join(format!("pu-engine-plan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let victim = tmp.join("Test.app/Contents/unplanned");
        fs::create_dir_all(victim.parent().unwrap()).unwrap();
        fs::write(&victim, "keep me").unwrap();

        let tmp = fs::canonicalize(tmp).unwrap();
        let victim = tmp.join("Test.app/Contents/unplanned");
        let plan = fake_plan(&tmp);
        let engine = Engine::new();
        let report = engine
            .uninstall(&plan, std::slice::from_ref(&victim), &mut |_, _, _| {})
            .expect("uninstall reports refused selections");

        assert_eq!(report.refused, vec![victim.clone()]);
        assert!(victim.exists(), "unplanned path must remain untouched");
        if let Some(audit) = report.audit_log {
            let _ = fs::remove_file(audit);
        }
        let _ = fs::remove_dir_all(tmp);
    }

    #[test]
    fn deletes_selected_paths_and_audits() {
        let tmp = std::env::temp_dir().join(format!("pu-engine-del-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let victim = tmp.join("Test.app");
        fs::create_dir_all(victim.join("Contents")).unwrap();
        fs::write(victim.join("Contents/binary"), "data").unwrap();

        let tmp = fs::canonicalize(tmp).unwrap();
        let victim = tmp.join("Test.app");
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
        assert!(report.success());
        assert_eq!(seen.len(), 1);

        let audit = report.audit_log.expect("audit log written");
        let contents = fs::read_to_string(&audit).unwrap();
        assert!(contents.contains("\"type\":\"plan\""));
        assert!(contents.contains("\"type\":\"summary\""));
        let _ = fs::remove_dir_all(&tmp);
        let _ = fs::remove_file(audit);
    }
}
