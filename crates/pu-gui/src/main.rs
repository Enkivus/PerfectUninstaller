use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use eframe::egui::{self, Align2, Color32, RichText};
use pu_core::util::format_bytes;
use pu_core::{Confidence, Engine, InstalledApp, RemovalPlan, RemovalReport};

enum Job {
    Progress { path: String, done: usize, total: usize },
    Scan(Result<Vec<InstalledApp>, String>),
    Analyze(Result<RemovalPlan, String>),
    Uninstall(Result<RemovalReport, String>),
}

#[derive(PartialEq, Clone, Copy)]
enum Busy {
    None,
    Scan,
    Analyze,
    Uninstall,
}

struct App {
    ctx: egui::Context,
    platform: &'static str,
    tx: Sender<Job>,
    rx: Receiver<Job>,

    apps: Vec<InstalledApp>,
    filter: String,
    selected: Option<String>,

    plan: Option<RemovalPlan>,
    checked: HashSet<PathBuf>,
    report: Option<RemovalReport>,

    busy: Busy,
    progress: Option<(String, usize, usize)>,
    error: Option<String>,
    confirm: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            ctx: cc.egui_ctx.clone(),
            platform: Engine::new().platform().name(),
            tx,
            rx,
            apps: Vec::new(),
            filter: String::new(),
            selected: None,
            plan: None,
            checked: HashSet::new(),
            report: None,
            busy: Busy::None,
            progress: None,
            error: None,
            confirm: false,
        }
    }

    fn pump(&mut self) {
        while let Ok(job) = self.rx.try_recv() {
            match job {
                Job::Progress { path, done, total } => {
                    self.progress = Some((path, done, total));
                }
                Job::Scan(result) => {
                    self.busy = Busy::None;
                    match result {
                        Ok(apps) => self.apps = apps,
                        Err(err) => self.error = Some(format!("Scan failed: {err}")),
                    }
                }
                Job::Analyze(result) => {
                    self.busy = Busy::None;
                    match result {
                        Ok(plan) => {
                            self.checked =
                                Engine::default_selection(&plan).into_iter().collect();
                            self.plan = Some(plan);
                        }
                        Err(err) => {
                            self.error = Some(format!("Analysis failed: {err}"));
                            self.selected = None;
                        }
                    }
                }
                Job::Uninstall(result) => {
                    self.busy = Busy::None;
                    self.progress = None;
                    match result {
                        Ok(report) => {
                            self.plan = None;
                            self.checked.clear();
                            self.report = Some(report);
                            self.spawn_scan();
                        }
                        Err(err) => self.error = Some(format!("Uninstall failed: {err}")),
                    }
                }
            }
        }
    }

    fn spawn_scan(&mut self) {
        if self.busy != Busy::None {
            return;
        }
        self.busy = Busy::Scan;
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        thread::spawn(move || {
            let result = Engine::new().scan().map_err(|e| e.to_string());
            let _ = tx.send(Job::Scan(result));
            ctx.request_repaint();
        });
    }

    fn spawn_analyze(&mut self, app: InstalledApp) {
        if self.busy != Busy::None {
            return;
        }
        self.busy = Busy::Analyze;
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        thread::spawn(move || {
            let result = Engine::new()
                .analyze(&app)
                .map_err(|e| e.to_string());
            let _ = tx.send(Job::Analyze(result));
            ctx.request_repaint();
        });
    }

    fn spawn_uninstall(&mut self) {
        let Some(plan) = self.plan.clone() else {
            return;
        };
        let selection: Vec<PathBuf> = self.checked.iter().cloned().collect();
        if selection.is_empty() || self.busy != Busy::None {
            return;
        }
        self.confirm = false;
        self.busy = Busy::Uninstall;
        self.progress = None;
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        thread::spawn(move || {
            let engine = Engine::new();
            let result = engine
                .uninstall(&plan, &selection, &mut |path, done, total| {
                    let _ = tx.send(Job::Progress {
                        path: path.display().to_string(),
                        done,
                        total,
                    });
                    ctx.request_repaint();
                })
                .map_err(|e| e.to_string());
            let _ = tx.send(Job::Uninstall(result));
            ctx.request_repaint();
        });
    }

    fn select(&mut self, app: &InstalledApp) {
        if self.selected.as_deref() == Some(app.id.as_str()) && self.plan.is_some() {
            return;
        }
        self.selected = Some(app.id.clone());
        self.plan = None;
        self.checked.clear();
        self.error = None;
        self.spawn_analyze(app.clone());
    }

    fn filtered(&self) -> Vec<&InstalledApp> {
        let q = self.filter.to_lowercase();
        self.apps
            .iter()
            .filter(|a| {
                q.is_empty()
                    || a.name.to_lowercase().contains(&q)
                    || a.identifier
                        .as_deref()
                        .is_some_and(|i| i.to_lowercase().contains(&q))
            })
            .collect()
    }

    fn set_all(&mut self, checked: bool) {
        let Some(plan) = self.plan.as_ref() else {
            return;
        };
        self.checked.clear();
        if checked {
            self.checked.extend(plan.candidates.iter().map(|c| c.path.clone()));
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.pump();

        egui::Panel::top("header").show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading("PerfectUninstaller");
                ui.separator();
                ui.label(RichText::new(self.platform).weak());
                if self.busy == Busy::Scan {
                    ui.spinner();
                    ui.label("Scanning…");
                }
                if ui
                    .add_enabled(
                        self.busy == Busy::None,
                        egui::Button::new("Scan for installed software"),
                    )
                    .clicked()
                {
                    self.spawn_scan();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{} apps found", self.apps.len())).weak(),
                    );
                });
            });
            ui.add_space(4.0);
        });

        if let Some(error) = self.error.clone() {
            egui::Panel::top("error").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(220, 80, 80), &error);
                    if ui.button("Dismiss").clicked() {
                        self.error = None;
                    }
                });
            });
        }

        if let Some((path, done, total)) = self.progress.clone() {
            egui::Panel::bottom("progress").show(ui, |ui| {
                ui.add_space(6.0);
                ui.add(
                    egui::ProgressBar::new(done as f32 / total.max(1) as f32)
                        .show_percentage()
                        .desired_width(f32::INFINITY),
                );
                ui.label(RichText::new(shorten(&path, 110)).monospace().weak());
                ui.add_space(4.0);
            });
        }

        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(report) = self.report.clone() {
                self.report_view(ui, &report);
            } else if self.plan.is_some() {
                self.plan_view(ui);
            } else {
                self.list_view(ui);
            }
        });

        self.confirm_window(&ctx);
    }
}

impl App {
    fn list_view(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Installed software").strong());
            ui.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text("Search by name or identifier…")
                    .desired_width(320.0),
            );
            if self.busy == Busy::Analyze {
                ui.spinner();
                ui.label("Analyzing…");
            }
        });
        ui.add_space(6.0);

        if self.apps.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Press “Scan for installed software” to begin.").weak());
            });
            return;
        }

        let filtered: Vec<InstalledApp> = self.filtered().into_iter().cloned().collect();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for app in &filtered {
                    let selected = self.selected.as_deref() == Some(app.id.as_str());
                    let label = format!(
                        "{}   {}{}",
                        app.name,
                        app.version.as_deref().unwrap_or(""),
                        if app.version.is_some() { "  ·  " } else { "" },
                    );
                    let resp = ui.selectable_label(
                        selected,
                        format!("{}  [{}]", label, app.method.label()),
                    );
                    if resp.clicked() {
                        self.select(app);
                    }
                    if let Some(id) = &app.identifier {
                        if id != &app.name {
                            ui.label(RichText::new(format!("    {id}")).weak().small());
                        }
                    }
                }
            });
    }

    fn plan_view(&mut self, ui: &mut egui::Ui) {
        let Some(plan) = self.plan.clone() else {
            return;
        };

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("← Back").clicked() {
                self.plan = None;
                self.checked.clear();
                self.selected = None;
            }
            ui.separator();
            ui.heading(&plan.app.name);
            ui.label(RichText::new(plan.app.method.label()).weak());
            if let Some(id) = &plan.app.identifier {
                ui.label(RichText::new(id).monospace().weak());
            }
            if self.busy == Busy::Analyze {
                ui.spinner();
            }
        });

        if !plan.warnings.is_empty() {
            ui.add_space(4.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                for warning in &plan.warnings {
                    ui.colored_label(Color32::from_rgb(230, 160, 40), format!("• {warning}"));
                }
            });
        }

        if !plan.advisories.is_empty() {
            ui.add_space(4.0);
            let title = format!(
                "System configuration findings ({} — detected, not changed automatically)",
                plan.advisories.len()
            );
            egui::CollapsingHeader::new(title)
                .default_open(false)
                .show(ui, |ui| {
                    for advisory in &plan.advisories {
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(advisory.kind.label()).strong());
                            ui.colored_label(Color32::from_rgb(200, 140, 60), &advisory.summary);
                        });
                        if let Some(path) = &advisory.path {
                            ui.label(
                                RichText::new(path.display().to_string()).monospace().small().weak(),
                            );
                        }
                        ui.label(RichText::new(&advisory.detail).small());
                        if let Some(command) = &advisory.command {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(command).monospace().small());
                                if ui.small_button("Copy command").clicked() {
                                    ui.ctx().copy_text(command.clone());
                                }
                            });
                        }
                        ui.separator();
                    }
                });
        }

        let selected: Vec<PathBuf> = self.checked.iter().cloned().collect();
        let selected_bytes = plan.selected_bytes(&selected);

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "{} item(s) selected · {} of {}",
                    selected.len(),
                    format_bytes(selected_bytes),
                    format_bytes(plan.total_bytes),
                ))
                .strong(),
            );
            if ui.button("All").clicked() {
                self.set_all(true);
            }
            if ui.button("Default").clicked() {
                self.checked = Engine::default_selection(&plan).into_iter().collect();
            }
            if ui.button("None").clicked() {
                self.set_all(false);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let enabled = !selected.is_empty() && self.busy == Busy::None;
                if ui
                    .add_enabled(
                        enabled,
                        egui::Button::new(
                            RichText::new(format!(
                                "Uninstall permanently ({})",
                                format_bytes(selected_bytes)
                            ))
                            .color(Color32::WHITE)
                            .strong(),
                        )
                        .fill(Color32::from_rgb(180, 45, 45)),
                    )
                    .clicked()
                {
                    self.confirm = true;
                }
            });
        });
        ui.add_space(6.0);

        egui::ScrollArea::vertical()
            .id_salt("plan_rows")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("plan_grid")
                    .striped(true)
                    .num_columns(5)
                    .spacing([14.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Keep").strong());
                        ui.label(RichText::new("Size").strong());
                        ui.label(RichText::new("Confidence").strong());
                        ui.label(RichText::new("Type").strong());
                        ui.label(RichText::new("Path").strong());
                        ui.end_row();

                        for candidate in &plan.candidates {
                            let mut is_checked = self.checked.contains(&candidate.path);
                            if ui.checkbox(&mut is_checked, "").changed() {
                                if is_checked {
                                    self.checked.insert(candidate.path.clone());
                                } else {
                                    self.checked.remove(&candidate.path);
                                }
                            }
                            ui.label(format_bytes(candidate.bytes));
                            let (text, color) = match candidate.confidence {
                                Confidence::High => ("high", Color32::from_rgb(90, 170, 90)),
                                Confidence::Medium => ("medium", Color32::from_rgb(220, 160, 60)),
                                Confidence::Low => ("low", Color32::from_gray(140)),
                            };
                            ui.label(RichText::new(text).color(color));
                            ui.label(candidate.category.label());
                            let path = candidate.path.display().to_string();
                            ui.label(RichText::new(shorten(&path, 90)).monospace().small())
                                .on_hover_text(format!(
                                    "{}\n\n{}",
                                    path, candidate.reason
                                ));
                            ui.end_row();
                        }
                    });
            });
    }

    fn report_view(&mut self, ui: &mut egui::Ui, report: &RemovalReport) {
        ui.add_space(12.0);
        ui.heading("Uninstall complete");
        ui.add_space(6.0);

        ui.label(
            RichText::new(format!(
                "{} item(s) removed · {} freed · {} ms",
                report.removed.len(),
                format_bytes(report.bytes_freed),
                report.elapsed_ms
            ))
            .strong(),
        );

        if !report.refused.is_empty() {
            ui.add_space(6.0);
            ui.colored_label(
                Color32::from_rgb(230, 160, 40),
                format!(
                    "{} path(s) refused by safety guards (left untouched):",
                    report.refused.len()
                ),
            );
            for path in &report.refused {
                ui.label(RichText::new(path.display().to_string()).monospace().small());
            }
        }

        if !report.failed.is_empty() {
            ui.add_space(6.0);
            ui.colored_label(
                Color32::from_rgb(220, 80, 80),
                format!("{} path(s) could not be removed:", report.failed.len()),
            );
            for failure in &report.failed {
                ui.label(
                    RichText::new(format!("{} — {}", failure.path.display(), failure.message))
                        .monospace()
                        .small(),
                );
            }
        }

        if let Some(audit) = &report.audit_log {
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("Audit log: {}", audit.display()))
                    .weak()
                    .small(),
            );
        }

        if let Some(dir) = &report.report_dir {
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("Exported report: {}", dir.display()))
                    .weak()
                    .small(),
            );
            ui.horizontal(|ui| {
                if ui.button("Open report folder").clicked() {
                    let _ = open_path(dir);
                }
                if ui.button("Open HTML report").clicked() {
                    let _ = open_path(&dir.join("report.html"));
                }
                if ui.small_button("Copy folder path").clicked() {
                    ui.ctx().copy_text(dir.display().to_string());
                }
            });
        }

        ui.add_space(12.0);
        if ui.button("← Back to app list").clicked() {
            self.report = None;
        }
    }

    fn confirm_window(&mut self, ctx: &egui::Context) {
        if !self.confirm {
            return;
        }
        let Some(plan) = self.plan.clone() else {
            self.confirm = false;
            return;
        };
        let selected: Vec<PathBuf> = self.checked.iter().cloned().collect();
        let bytes = plan.selected_bytes(&selected);

        let mut open = true;
        egui::Window::new("Confirm permanent deletion")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label(format!(
                    "Permanently delete {} item(s) ({}) belonging to “{}”?",
                    selected.len(),
                    format_bytes(bytes),
                    plan.app.name
                ));
                ui.add_space(4.0);
                ui.colored_label(
                    Color32::from_rgb(220, 80, 80),
                    "There is no undo. A JSONL audit log of every path will be kept.",
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.confirm = false;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Delete forever").color(Color32::WHITE).strong(),
                                )
                                .fill(Color32::from_rgb(180, 45, 45)),
                            )
                            .clicked()
                        {
                            self.spawn_uninstall();
                        }
                    });
                });
            });
        if !open {
            self.confirm = false;
        }
    }
}

fn open_path(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(target_os = "windows")]
    let program = "explorer";
    #[cfg(all(unix, not(target_os = "macos")))]
    let program = "xdg-open";

    std::process::Command::new(program).arg(path).spawn().map(|_| ())
}

fn shorten(path: &str, max: usize) -> String {
    if path.chars().count() <= max {
        return path.to_string();
    }
    let head: String = path.chars().take(max.saturating_sub(3)).collect();
    format!("{head}…")
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 740.0])
            .with_min_inner_size([880.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "PerfectUninstaller",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
