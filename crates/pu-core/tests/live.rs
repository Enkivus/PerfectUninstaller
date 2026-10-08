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

    let default = Engine::default_selection(&plan);
    println!("{} selected by default", default.len());
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
