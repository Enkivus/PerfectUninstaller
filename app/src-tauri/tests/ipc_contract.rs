use pu_core::{Engine, InstalledApp};

#[test]
fn scan_produces_serializable_apps() {
    let apps = Engine::new().scan().expect("scan should succeed");
    // The UI relies on each app round-tripping through the IPC boundary, which
    // means every field must be serializable; asserting that json encoding does
    // not panic on a real app guards the whole DTO contract.
    for app in apps.iter().take(5) {
        let json = serde_json::to_string(app).expect("InstalledApp serializes");
        let back: InstalledApp = serde_json::from_str(&json).expect("round-trips");
        assert_eq!(back.id, app.id);
        assert_eq!(back.name, app.name);
    }
}

#[test]
fn analyze_and_default_selection_round_trip() {
    let apps = match Engine::new().scan() {
        Ok(a) => a,
        Err(_) => return,
    };
    let Some(app) = apps.first() else { return };
    let Ok(plan) = Engine::new().analyze(app) else {
        return;
    };
    let json = serde_json::to_string(&plan).expect("RemovalPlan serializes");
    let back: pu_core::RemovalPlan = serde_json::from_str(&json).expect("round-trips");
    assert_eq!(back.app.name, plan.app.name);
    assert_eq!(back.candidates.len(), plan.candidates.len());

    let selected = Engine::default_selection(&plan);
    for path in &selected {
        assert!(
            plan.candidates.iter().any(|c| &c.path == path),
            "default selection must only contain plan candidates"
        );
    }
}