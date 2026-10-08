//! Live checks against the host machine. Ignored by default because they
//! depend on what is installed here.
//!
//! Run manually: `cargo test -p pu-core --test live -- --ignored --nocapture`

use pu_core::Engine;

#[test]
#[ignore = "depends on the host machine's installed software"]
fn scan_and_analyze_a_real_app() {
    let engine = Engine::new();
    let apps = engine.scan().expect("scan");
    assert!(!apps.is_empty(), "no apps discovered on this machine");
    println!("discovered {} apps", apps.len());

    let app = apps
        .iter()
        .find(|a| a.identifier.is_some())
        .unwrap_or(&apps[0]);
    println!("analyzing: {} ({:?})", app.name, app.identifier);

    let plan = engine.analyze(app).expect("analyze");
    println!(
        "{} candidates, {} total",
        plan.candidates.len(),
        pu_core::util::format_bytes(plan.total_bytes)
    );
    for candidate in plan.candidates.iter().take(15) {
        println!(
            "  [{}] {} ({}) — {}",
            candidate.confidence.label(),
            candidate.path.display(),
            pu_core::util::format_bytes(candidate.bytes),
            candidate.reason
        );
    }
    for warning in &plan.warnings {
        println!("warning: {warning}");
    }

    println!("{} system-config advisories:", plan.advisories.len());
    for advisory in &plan.advisories {
        println!(
            "  [{}] {} — {}",
            advisory.kind.label(),
            advisory.summary,
            advisory.command.as_deref().unwrap_or("(manual)")
        );
    }

    let default = Engine::default_selection(&plan);
    println!("{} selected by default", default.len());

    // Export a report for the plan and confirm the files exist.
    let dir = pu_core::report::default_report_dir(&app.name).expect("report dir");
    let _ = std::fs::remove_dir_all(&dir);
    let report = pu_core::models::RemovalReport {
        removed: Vec::new(),
        bytes_freed: 0,
        failed: Vec::new(),
        refused: Vec::new(),
        audit_log: None,
        report_dir: None,
        elapsed_ms: 0,
    };
    let exported = pu_core::report::export(&dir, &plan, &report).expect("export");
    println!("exported report: {}", exported.html.display());
    assert!(exported.html.exists() && exported.json.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "depends on the host machine's installed software"]
fn advisories_across_host_apps() {
    let engine = Engine::new();
    let apps = engine.scan().expect("scan");
    let mut total = 0;
    for app in apps.iter().take(12) {
        let plan = engine.analyze(app).expect("analyze");
        if !plan.advisories.is_empty() {
            println!("{} — {} finding(s)", app.name, plan.advisories.len());
            for advisory in &plan.advisories {
                println!("   [{}] {}", advisory.kind.label(), advisory.summary);
            }
            total += plan.advisories.len();
        }
    }
    println!("total advisories across sample: {total}");
}

#[test]
#[ignore = "depends on the host machine's installed software"]
fn every_candidate_passes_the_safety_guards() {
    let engine = Engine::new();
    let apps = engine.scan().expect("scan").into_iter().take(5);
    for app in apps {
        let plan = engine.analyze(&app).expect("analyze");
        for candidate in &plan.candidates {
            pu_core::safety::ensure_safe(&candidate.path).unwrap_or_else(|e| {
                panic!("unsafe candidate for {}: {} ({e})", app.name, candidate.path.display())
            });
        }
    }
}
