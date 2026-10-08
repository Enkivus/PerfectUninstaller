//! Export a human-readable and machine-readable report of an uninstall run.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::models::{RemovalPlan, RemovalReport};
use crate::util::format_bytes;

/// Files produced by [`export`].
#[derive(Debug, Clone)]
pub struct ExportedReport {
    pub dir: PathBuf,
    pub html: PathBuf,
    pub json: PathBuf,
    pub audit: Option<PathBuf>,
}

/// `~/.perfectuninstaller/reports/<app-slug>-<timestamp>`.
pub fn default_report_dir(app_name: &str) -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| Error::Other("HOME is not set".into()))?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    Ok(home
        .join(".perfectuninstaller")
        .join("reports")
        .join(format!("{}-{}", slug(app_name), stamp)))
}

fn slug(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_') { c } else { '_' })
        .collect();
    out.truncate(40);
    if out.is_empty() {
        out.push_str("app");
    }
    out
}

/// Writes `report.html`, `report.json` and a copy of the audit log into `dir`.
pub fn export(dir: &Path, plan: &RemovalPlan, report: &RemovalReport) -> Result<ExportedReport> {
    fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;

    let json_path = dir.join("report.json");
    let document = serde_json::json!({
        "generated_at": chrono::Local::now().to_rfc3339(),
        "app": plan.app,
        "advisories": plan.advisories,
        "warnings": plan.warnings,
        "planned_candidates": plan.candidates,
        "planned_bytes": plan.total_bytes,
        "result": report,
    });
    let json = serde_json::to_string_pretty(&document)
        .map_err(|e| Error::Other(format!("failed to serialize report: {e}")))?;
    fs::write(&json_path, json).map_err(|e| Error::io(&json_path, e))?;

    let html_path = dir.join("report.html");
    fs::write(&html_path, render_html(plan, report)).map_err(|e| Error::io(&html_path, e))?;

    let mut audit = None;
    if let Some(source) = &report.audit_log {
        if source.exists() {
            let dest = dir.join("audit.jsonl");
            if fs::copy(source, &dest).is_ok() {
                audit = Some(dest);
            }
        }
    }

    Ok(ExportedReport {
        dir: dir.to_path_buf(),
        html: html_path,
        json: json_path,
        audit,
    })
}

pub fn render_html(plan: &RemovalPlan, report: &RemovalReport) -> String {
    let mut html = String::with_capacity(16 * 1024);
    let app = &plan.app;

    writeln!(html, "<!doctype html>").ok();
    writeln!(html, "<html lang=\"en\"><head><meta charset=\"utf-8\">").ok();
    writeln!(
        html,
        "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">"
    )
    .ok();
    writeln!(
        html,
        "<title>PerfectUninstaller report — {}</title>",
        escape(&app.name)
    )
    .ok();
    writeln!(html, "<style>{}</style></head><body>", STYLE).ok();

    writeln!(html, "<h1>PerfectUninstaller report</h1>").ok();
    writeln!(
        html,
        "<p class=\"meta\"><strong>{}</strong> {} &middot; {} &middot; {} &middot; generated {}</p>",
        escape(&app.name),
        app.version
            .as_deref()
            .map(escape)
            .unwrap_or_default(),
        app.identifier
            .as_deref()
            .map(|i| format!("<code>{}</code>", escape(i)))
            .unwrap_or_else(|| "no identifier".into()),
        escape(app.method.label()),
        chrono::Local::now().to_rfc3339(),
    )
    .ok();

    writeln!(html, "<section class=\"cards\">").ok();
    card(&mut html, "Removed", &report.removed.len().to_string());
    card(&mut html, "Freed", &format_bytes(report.bytes_freed));
    card(&mut html, "Refused", &report.refused.len().to_string());
    card(&mut html, "Failed", &report.failed.len().to_string());
    card(&mut html, "Elapsed", &format!("{} ms", report.elapsed_ms));
    writeln!(html, "</section>").ok();

    if !plan.warnings.is_empty() {
        writeln!(html, "<h2>Warnings</h2><ul>").ok();
        for warning in &plan.warnings {
            writeln!(html, "<li>{}</li>", escape(warning)).ok();
        }
        writeln!(html, "</ul>").ok();
    }

    if !plan.advisories.is_empty() {
        writeln!(html, "<h2>System configuration findings (not changed)</h2>").ok();
        writeln!(html, "<p class=\"note\">These changes were detected but left in place on purpose. \
            Run the suggested commands yourself if you want them gone.</p>").ok();
        writeln!(
            html,
            "<table><thead><tr><th>Kind</th><th>Finding</th><th>Detail</th><th>Suggested command</th></tr></thead><tbody>"
        )
        .ok();
        for advisory in &plan.advisories {
            writeln!(
                html,
                "<tr><td>{}</td><td>{}<br><span class=\"path\">{}</span></td><td><pre>{}</pre></td><td>{}</td></tr>",
                escape(advisory.kind.label()),
                escape(&advisory.summary),
                escape(
                    &advisory
                        .path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default()
                ),
                escape(&advisory.detail),
                advisory
                    .command
                    .as_deref()
                    .map(|c| format!("<code>{}</code>", escape(c)))
                    .unwrap_or_else(|| "—".into()),
            )
            .ok();
        }
        writeln!(html, "</tbody></table>").ok();
    }

    let removed: std::collections::HashMap<&Path, &crate::models::TraceCandidate> =
        plan.candidates.iter().map(|c| (c.path.as_path(), c)).collect();

    writeln!(html, "<h2>Removed ({} items)</h2>", report.removed.len()).ok();
    writeln!(
        html,
        "<table><thead><tr><th>Path</th><th>Type</th><th>Confidence</th><th>Size</th><th>Reason</th></tr></thead><tbody>"
    )
    .ok();
    for path in &report.removed {
        let (category, confidence, bytes, reason) = match removed.get(path.as_path()) {
            Some(c) => (
                c.category.label(),
                c.confidence.label(),
                format_bytes(c.bytes),
                c.reason.clone(),
            ),
            None => ("—", "—", "—".into(), String::new()),
        };
        writeln!(
            html,
            "<tr><td class=\"path\">{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape(&path.display().to_string()),
            escape(category),
            escape(confidence),
            escape(&bytes),
            escape(&reason),
        )
        .ok();
    }
    writeln!(html, "</tbody></table>").ok();

    if !report.refused.is_empty() {
        writeln!(html, "<h2>Refused by safety guards ({})</h2><ul>", report.refused.len()).ok();
        for path in &report.refused {
            writeln!(html, "<li class=\"path\">{}</li>", escape(&path.display().to_string())).ok();
        }
        writeln!(html, "</ul>").ok();
    }

    if !report.failed.is_empty() {
        writeln!(html, "<h2>Failed ({})</h2><ul>", report.failed.len()).ok();
        for failure in &report.failed {
            writeln!(
                html,
                "<li class=\"path\">{} — {}</li>",
                escape(&failure.path.display().to_string()),
                escape(&failure.message)
            )
            .ok();
        }
        writeln!(html, "</ul>").ok();
    }

    if let Some(audit) = &report.audit_log {
        writeln!(
            html,
            "<h2>Audit log</h2><p class=\"path\">{}</p>",
            escape(&audit.display().to_string())
        )
        .ok();
    }

    writeln!(html, "</body></html>").ok();
    html
}

fn card(html: &mut String, label: &str, value: &str) {
    writeln!(
        html,
        "<div class=\"card\"><span class=\"value\">{}</span><span class=\"label\">{}</span></div>",
        escape(value),
        escape(label)
    )
    .ok();
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

const STYLE: &str = "
body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;margin:2rem;color:#1c1c1e;background:#fafafa}
h1{margin:0 0 .25rem}h2{margin-top:2rem}
.meta{color:#555;margin:.25rem 0 1.5rem}
.cards{display:flex;gap:1rem;flex-wrap:wrap}
.card{background:#fff;border:1px solid #e2e2e4;border-radius:10px;padding:.75rem 1.25rem;display:flex;flex-direction:column;min-width:6rem}
.card .value{font-size:1.5rem;font-weight:600}.card .label{color:#666;font-size:.8rem;text-transform:uppercase;letter-spacing:.04em}
table{border-collapse:collapse;width:100%;background:#fff;border:1px solid #e2e2e4;border-radius:10px;overflow:hidden}
th,td{text-align:left;padding:.5rem .75rem;border-bottom:1px solid #eee;vertical-align:top;font-size:.9rem}
th{background:#f2f2f4;font-weight:600}
tr:last-child td{border-bottom:none}
.path,pre,code{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:.82rem;word-break:break-all}
pre{white-space:pre-wrap;margin:0}
.note{color:#8a5a00;background:#fff7e6;border:1px solid #ffd591;border-radius:8px;padding:.5rem .75rem}
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        Confidence, InstallMethod, InstalledApp, RemovalFailure, RemovalPlan, RemovalReport,
        TraceCandidate, TraceCategory,
    };

    fn sample() -> (RemovalPlan, RemovalReport) {
        let app = InstalledApp {
            id: "com.example.cool".into(),
            name: "Cool <App> & Co".into(),
            identifier: Some("com.example.cool".into()),
            version: Some("1.0".into()),
            install_paths: vec![PathBuf::from("/Applications/Cool.app")],
            method: InstallMethod::AppBundle,
            platform: crate::models::Platform::Macos,
            install_bytes: 0,
        };
        let plan = RemovalPlan {
            app,
            candidates: vec![TraceCandidate {
                path: PathBuf::from("/Applications/Cool.app"),
                category: TraceCategory::Application,
                confidence: Confidence::High,
                bytes: 1024,
                reason: "primary install location".into(),
                primary: true,
            }],
            advisories: vec![],
            total_bytes: 1024,
            warnings: vec!["example warning".into()],
        };
        let report = RemovalReport {
            removed: vec![PathBuf::from("/Applications/Cool.app")],
            bytes_freed: 1024,
            failed: vec![RemovalFailure {
                path: PathBuf::from("/Library/thing"),
                message: "permission denied".into(),
            }],
            refused: vec![],
            audit_log: None,
            report_dir: None,
            elapsed_ms: 12,
        };
        (plan, report)
    }

    #[test]
    fn html_escapes_user_content() {
        let (plan, report) = sample();
        let html = render_html(&plan, &report);
        assert!(html.contains("Cool &lt;App&gt; &amp; Co"));
        assert!(!html.contains("Cool <App>"));
        assert!(html.contains("/Applications/Cool.app"));
        assert!(html.contains("permission denied"));
    }

    #[test]
    fn export_writes_html_and_json() {
        let dir = std::env::temp_dir().join(format!("pu-report-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let (plan, report) = sample();
        let exported = export(&dir, &plan, &report).expect("export");

        assert!(exported.html.exists());
        assert!(exported.json.exists());
        let json = fs::read_to_string(&exported.json).unwrap();
        assert!(json.contains("\"app\""));
        assert!(json.contains("\"result\""));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn slug_is_filesystem_safe() {
        assert_eq!(slug("Cool App 2.0 / Beta"), "Cool_App_2.0___Beta");
        assert_eq!(slug(""), "app");
    }
}
