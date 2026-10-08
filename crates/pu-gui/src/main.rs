use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::OnceLock;
use std::thread;

use eframe::egui::{
    self, Align, Color32, CornerRadius, Id, Layout, Margin, RichText, Sense, Stroke, Vec2,
};
use pu_core::util::format_bytes;
use pu_core::{Confidence, Engine, InstalledApp, RemovalPlan, RemovalReport, TraceCategory};

const REPO_URL: &str = "https://github.com/Enkivus/PerfectUninstaller";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Os {
    Mac,
    Windows,
    Linux,
}

fn current_os() -> Os {
    #[cfg(target_os = "macos")]
    {
        Os::Mac
    }
    #[cfg(target_os = "windows")]
    {
        Os::Windows
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Os::Linux
    }
}

#[derive(Clone, Copy)]
struct Palette {
    bg: Color32,
    sidebar: Color32,
    header: Color32,
    card: Color32,
    card_stroke: Color32,
    control: Color32,
    control_hover: Color32,
    control_active: Color32,
    text: Color32,
    dim: Color32,
    accent: Color32,
    accent_fill: Color32,
    accent_soft: Color32,
    danger: Color32,
    danger_fill: Color32,
    ok: Color32,
    warn: Color32,
    border: Color32,
    window_fill: Color32,
    radius: u8,
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn palette_for(os: Os) -> Palette {
    match os {
        // macOS 26 "Liquid Glass": airy translucent layers, vibrant system blue,
        // bright 1px specular edges, generous corner radii.
        Os::Mac => Palette {
            bg: rgba(24, 26, 33, 132),
            sidebar: rgba(30, 33, 43, 96),
            header: rgba(34, 37, 47, 130),
            card: rgba(255, 255, 255, 22),
            card_stroke: rgba(255, 255, 255, 46),
            control: rgba(255, 255, 255, 26),
            control_hover: rgba(255, 255, 255, 46),
            control_active: rgba(10, 132, 255, 120),
            text: Color32::from_rgb(245, 246, 250),
            dim: Color32::from_rgb(170, 176, 192),
            accent: Color32::from_rgb(10, 132, 255),
            accent_fill: Color32::from_rgb(10, 132, 255),
            accent_soft: rgba(10, 132, 255, 72),
            danger: Color32::from_rgb(255, 96, 86),
            danger_fill: Color32::from_rgb(214, 61, 55),
            ok: Color32::from_rgb(48, 209, 88),
            warn: Color32::from_rgb(255, 214, 10),
            border: rgba(255, 255, 255, 40),
            window_fill: rgba(30, 33, 43, 232),
            radius: 12,
        },
        // Windows 11 Fluent: Mica base, subtle 5%–10% white layers, 8px radii,
        // system accent, restrained borders.
        Os::Windows => Palette {
            bg: rgba(32, 32, 32, 180),
            sidebar: rgba(40, 40, 40, 150),
            header: rgba(44, 44, 44, 170),
            card: rgba(255, 255, 255, 15),
            card_stroke: rgba(255, 255, 255, 28),
            control: rgba(255, 255, 255, 18),
            control_hover: rgba(255, 255, 255, 32),
            control_active: rgba(96, 205, 255, 110),
            text: Color32::from_rgb(255, 255, 255),
            dim: Color32::from_rgb(200, 200, 200),
            accent: Color32::from_rgb(96, 205, 255),
            accent_fill: Color32::from_rgb(0, 103, 192),
            accent_soft: rgba(96, 205, 255, 64),
            danger: Color32::from_rgb(255, 153, 164),
            danger_fill: Color32::from_rgb(196, 43, 28),
            ok: Color32::from_rgb(108, 203, 95),
            warn: Color32::from_rgb(252, 225, 0),
            border: rgba(255, 255, 255, 26),
            window_fill: rgba(44, 44, 44, 240),
            radius: 8,
        },
        Os::Linux => Palette {
            bg: Color32::from_rgb(20, 22, 26),
            sidebar: Color32::from_rgb(27, 30, 36),
            header: Color32::from_rgb(22, 24, 29),
            card: Color32::from_rgb(32, 36, 44),
            card_stroke: Color32::from_rgb(52, 58, 70),
            control: Color32::from_rgb(40, 45, 54),
            control_hover: Color32::from_rgb(52, 59, 72),
            control_active: Color32::from_rgb(58, 72, 122),
            text: Color32::from_rgb(226, 230, 238),
            dim: Color32::from_rgb(150, 156, 170),
            accent: Color32::from_rgb(122, 150, 255),
            accent_fill: Color32::from_rgb(96, 124, 230),
            accent_soft: Color32::from_rgb(58, 72, 122),
            danger: Color32::from_rgb(235, 110, 110),
            danger_fill: Color32::from_rgb(196, 62, 62),
            ok: Color32::from_rgb(110, 200, 140),
            warn: Color32::from_rgb(235, 180, 90),
            border: Color32::from_rgb(52, 58, 70),
            window_fill: Color32::from_rgb(32, 36, 44),
            radius: 10,
        },
    }
}

fn pal() -> &'static Palette {
    static PALETTE: OnceLock<Palette> = OnceLock::new();
    PALETTE.get_or_init(|| palette_for(current_os()))
}

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

#[derive(PartialEq, Clone, Copy)]
enum ConfirmStage {
    None,
    Review,
    Final,
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

    confirm: ConfirmStage,
    confirm_ack: bool,
    confirm_name: String,
    show_about: bool,
    glass_applied: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut app = Self {
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
            confirm: ConfirmStage::None,
            confirm_ack: false,
            confirm_name: String::new(),
            show_about: false,
            glass_applied: false,
        };
        install_theme(&app.ctx);
        app.spawn_scan();
        app
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
                            self.checked = Engine::default_selection(&plan).into_iter().collect();
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

    fn apply_native_material(&mut self, frame: &eframe::Frame) {
        if self.glass_applied {
            return;
        }
        self.glass_applied = true;
        let Some(window) = frame.winit_window() else {
            return;
        };

        #[cfg(target_os = "macos")]
        {
            use window_vibrancy::{
                apply_liquid_glass, apply_vibrancy, LiquidGlassOptions, NSGlassEffectViewStyle,
                NSVisualEffectMaterial, NSVisualEffectState,
            };
            let glass = LiquidGlassOptions::new(NSGlassEffectViewStyle::Regular);
            if apply_liquid_glass(window.as_ref(), glass).is_err() {
                let _ = apply_vibrancy(
                    window.as_ref(),
                    NSVisualEffectMaterial::HudWindow,
                    Some(NSVisualEffectState::Active),
                    Some(0.0),
                );
            }
        }

        #[cfg(target_os = "windows")]
        {
            use window_vibrancy::{apply_acrylic, apply_mica};
            if apply_mica(window.as_ref(), Some(true)).is_err() {
                let _ = apply_acrylic(window.as_ref(), Some((26, 27, 32, 200)));
            }
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = window;
        }
    }

    fn spawn_scan(&mut self) {
        if self.busy != Busy::None {
            return;
        }
        self.busy = Busy::Scan;
        self.report = None;
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
            let result = Engine::new().analyze(&app).map_err(|e| e.to_string());
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
        self.confirm = ConfirmStage::None;
        self.confirm_ack = false;
        self.confirm_name.clear();
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
        self.report = None;
        self.error = None;
        self.spawn_analyze(app.clone());
    }

    fn filtered(&self) -> Vec<InstalledApp> {
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
            .cloned()
            .collect()
    }

    fn selected_app_name(&self) -> Option<String> {
        let id = self.selected.as_deref()?;
        self.apps.iter().find(|a| a.id == id).map(|a| a.name.clone())
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
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            Color32::TRANSPARENT.to_normalized_gamma_f32()
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            pal().bg.to_normalized_gamma_f32()
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.apply_native_material(frame);
        let ctx = ui.ctx().clone();
        self.pump();

        egui::Panel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(pal().header)
                    .inner_margin(Margin::symmetric(18, 12)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("◆").color(pal().accent).size(22.0));
                    ui.vertical(|ui| {
                        ui.label(RichText::new("PerfectUninstaller").size(19.0).strong());
                        ui.label(
                            RichText::new("Complete software removal")
                                .size(11.0)
                                .color(pal().dim),
                        );
                    });
                    ui.add_space(10.0);
                    badge(ui, self.platform, pal().accent, pal().accent_soft);

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let scan = ui.add_enabled(
                            self.busy == Busy::None,
                            egui::Button::new(RichText::new("↻  Scan").color(primary_text()).strong())
                                .fill(pal().accent_fill),
                        );
                        if scan.clicked() {
                            self.spawn_scan();
                        }
                        if ui.button("About").clicked() {
                            self.show_about = true;
                        }
                        if self.busy == Busy::Scan {
                            ui.spinner();
                        }
                        ui.label(
                            RichText::new(format!("{} apps", self.apps.len()))
                                .color(pal().dim)
                                .small(),
                        );
                    });
                });
            });

        if let Some(error) = self.error.clone() {
            egui::Panel::top("error")
                .frame(
                    egui::Frame::new()
                        .fill(rgba(120, 30, 34, 150))
                        .inner_margin(Margin::symmetric(18, 8)),
                )
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("⚠").color(pal().danger).strong());
                        ui.colored_label(pal().danger, &error);
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui.small_button("Dismiss").clicked() {
                                self.error = None;
                            }
                        });
                    });
                });
        }

        if let Some((path, done, total)) = self.progress.clone() {
            egui::Panel::bottom("progress")
                .frame(
                    egui::Frame::new()
                        .fill(pal().header)
                        .inner_margin(Margin::symmetric(18, 12)),
                )
                .show(ui, |ui| {
                    ui.add(
                        egui::ProgressBar::new(done as f32 / total.max(1) as f32)
                            .fill(pal().accent)
                            .corner_radius(pal().radius)
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("Deleting {done}/{total}…"))
                            .small()
                            .color(pal().dim),
                    );
                    ui.label(RichText::new(shorten(&path, 120)).monospace().small().weak());
                });
        }

        egui::Panel::left("apps")
            .resizable(true)
            .default_size(322.0)
            .min_size(266.0)
            .max_size(460.0)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(pal().sidebar)
                    .inner_margin(Margin::symmetric(14, 14)),
            )
            .show(ui, |ui| self.sidebar(ui));

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(pal().bg)
                    .inner_margin(Margin::symmetric(24, 18)),
            )
            .show(ui, |ui| {
                if let Some(report) = self.report.clone() {
                    self.report_view(ui, &report);
                } else if self.busy == Busy::Analyze && self.plan.is_none() {
                    self.analyzing_view(ui);
                } else if self.plan.is_some() {
                    self.plan_view(ui);
                } else {
                    self.welcome_view(ui);
                }
            });

        self.confirm_modal(&ctx);
        self.about_modal(&ctx);
    }
}

impl App {
    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let count = {
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
                .count()
        };

        ui.horizontal(|ui| {
            ui.label(RichText::new("Applications").strong().size(14.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(format!("{count}")).small().color(pal().dim));
            });
        });
        ui.add_space(8.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.filter)
                .hint_text("Search by name or identifier…")
                .desired_width(f32::INFINITY),
        );
        ui.add_space(10.0);

        if self.busy == Busy::Scan && self.apps.is_empty() {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.spinner();
                ui.add_space(6.0);
                ui.label(RichText::new("Scanning…").color(pal().dim));
            });
            return;
        }

        if self.apps.is_empty() {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No software found.").color(pal().dim));
            });
            return;
        }

        let filtered = self.filtered();
        egui::ScrollArea::vertical()
            .id_salt("apps_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if filtered.is_empty() {
                    ui.add_space(16.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("No matches.").color(pal().dim));
                    });
                }
                for app in &filtered {
                    let selected = self.selected.as_deref() == Some(app.id.as_str());
                    let analyzing = self.busy == Busy::Analyze && selected;
                    if app_row(ui, app, selected, analyzing).clicked() {
                        self.select(app);
                    }
                }
            });
    }

    fn welcome_view(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(rect.height() * 0.22);
                ui.label(RichText::new("◆").color(pal().accent).size(54.0));
                ui.add_space(10.0);
                ui.label(RichText::new("Nothing selected").size(22.0).strong());
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "Pick an application on the left to see everything it left behind,\n\
                         then decide what to remove.",
                    )
                    .color(pal().dim),
                );
                ui.add_space(14.0);
                if self.apps.is_empty()
                    && ui
                        .add(
                            egui::Button::new(
                                RichText::new("Scan for installed software")
                                    .color(primary_text())
                                    .strong(),
                            )
                            .fill(pal().accent_fill),
                        )
                        .clicked()
                {
                    self.spawn_scan();
                }
            });
        });
    }

    fn analyzing_view(&mut self, ui: &mut egui::Ui) {
        let name = self.selected_app_name().unwrap_or_default();
        let rect = ui.available_rect_before_wrap();
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(rect.height() * 0.24);
                ui.add(egui::Spinner::new().size(42.0).color(pal().accent));
                ui.add_space(14.0);
                ui.label(RichText::new(format!("Analyzing {name}")).size(20.0).strong());
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "Searching caches, preferences, containers, launch agents, receipts,\n\
                         crash reports and system configuration…",
                    )
                    .color(pal().dim),
                );
                ui.add_space(18.0);
                ui.add(
                    egui::ProgressBar::new(0.0)
                        .animate(true)
                        .fill(pal().accent)
                        .corner_radius(pal().radius)
                        .desired_width(320.0),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new("This can take a few seconds on a large app.")
                        .small()
                        .color(pal().dim),
                );
            });
        });
    }

    fn plan_view(&mut self, ui: &mut egui::Ui) {
        let Some(plan) = self.plan.clone() else {
            return;
        };

        ui.horizontal(|ui| {
            if ui.button("← Back").clicked() {
                self.plan = None;
                self.checked.clear();
                self.selected = None;
            }
            ui.add_space(4.0);
            ui.label(RichText::new(&plan.app.name).size(22.0).strong());
            badge(ui, plan.app.method.label(), pal().dim, pal().card);
            if let Some(id) = &plan.app.identifier {
                if id != &plan.app.name {
                    ui.label(RichText::new(id).monospace().small().color(pal().dim));
                }
            }
        });

        let selected: Vec<PathBuf> = self.checked.iter().cloned().collect();
        let selected_bytes = plan.selected_bytes(&selected);

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            stat_card(ui, "Selected items", &format!("{}", selected.len()), pal().accent);
            stat_card(ui, "Selected size", &format_bytes(selected_bytes), pal().accent);
            stat_card(ui, "Total found", &format_bytes(plan.total_bytes), pal().text);
            stat_card(ui, "Categories", &format!("{}", group_categories(&plan).len()), pal().text);
        });

        if !plan.warnings.is_empty() {
            ui.add_space(10.0);
            for warning in &plan.warnings {
                warning_row(ui, warning);
            }
        }

        if !plan.advisories.is_empty() {
            ui.add_space(10.0);
            let title = format!(
                "System configuration findings ({} detected · never changed automatically)",
                plan.advisories.len()
            );
            egui::CollapsingHeader::new(RichText::new(title).strong())
                .default_open(false)
                .show(ui, |ui| {
                    for advisory in &plan.advisories {
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            badge(ui, advisory.kind.label(), pal().warn, pal().card);
                            ui.label(RichText::new(&advisory.summary).color(pal().text));
                        });
                        if let Some(path) = &advisory.path {
                            ui.label(
                                RichText::new(path.display().to_string())
                                    .monospace()
                                    .small()
                                    .weak(),
                            );
                        }
                        ui.label(RichText::new(&advisory.detail).small().color(pal().dim));
                        if let Some(command) = &advisory.command {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(command).monospace().small());
                                if ui.small_button("Copy").clicked() {
                                    ui.ctx().copy_text(command.clone());
                                }
                            });
                        }
                        ui.separator();
                    }
                });
        }

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "{} of {} items selected",
                    selected.len(),
                    plan.candidates.len()
                ))
                .strong(),
            );
            ui.add_space(6.0);
            if ui.button("All").clicked() {
                self.set_all(true);
            }
            if ui.button("Default").clicked() {
                self.checked = Engine::default_selection(&plan).into_iter().collect();
            }
            if ui.button("None").clicked() {
                self.set_all(false);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let enabled = !selected.is_empty() && self.busy == Busy::None;
                if ui
                    .add_enabled(
                        enabled,
                        egui::Button::new(
                            RichText::new(format!("Uninstall  ·  {}", format_bytes(selected_bytes)))
                                .color(primary_text())
                                .strong(),
                        )
                        .fill(pal().danger_fill)
                        .min_size(Vec2::new(0.0, 32.0)),
                    )
                    .clicked()
                {
                    self.confirm = ConfirmStage::Review;
                    self.confirm_ack = false;
                    self.confirm_name.clear();
                }
            });
        });
        ui.add_space(8.0);

        let groups = group_categories(&plan);
        egui::ScrollArea::vertical()
            .id_salt("plan_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (category, indices) in &groups {
                    let cat_bytes: u64 =
                        indices.iter().map(|&i| plan.candidates[i].bytes).sum();
                    let all_selected = indices
                        .iter()
                        .all(|&i| self.checked.contains(&plan.candidates[i].path));

                    ui.add_space(6.0);
                    glass_card(ui, |ui| {
                        ui.horizontal(|ui| {
                            let mut all = all_selected;
                            if ui.checkbox(&mut all, "").changed() {
                                for &i in indices {
                                    let path = &plan.candidates[i].path;
                                    if all {
                                        self.checked.insert(path.clone());
                                    } else {
                                        self.checked.remove(path);
                                    }
                                }
                            }
                            ui.label(RichText::new(category.label()).strong());
                            ui.label(
                                RichText::new(format!("{} item(s)", indices.len()))
                                    .small()
                                    .color(pal().dim),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(format_bytes(cat_bytes))
                                        .small()
                                        .color(pal().dim),
                                );
                            });
                        });
                    });

                    ui.add_space(2.0);
                    ui.indent(("cat", category.label()), |ui| {
                        for &i in indices {
                            let candidate = &plan.candidates[i];
                            ui.horizontal(|ui| {
                                let mut is_checked = self.checked.contains(&candidate.path);
                                if ui.checkbox(&mut is_checked, "").changed() {
                                    if is_checked {
                                        self.checked.insert(candidate.path.clone());
                                    } else {
                                        self.checked.remove(&candidate.path);
                                    }
                                }
                                let (text, color) = confidence_style(candidate.confidence);
                                ui.label(RichText::new(text).color(color).small());
                                let path = candidate.path.display().to_string();
                                ui.label(RichText::new(shorten(&path, 76)).monospace().small())
                                    .on_hover_text(format!("{path}\n\n{}", candidate.reason));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(format_bytes(candidate.bytes))
                                            .small()
                                            .color(pal().dim),
                                    );
                                });
                            });
                        }
                    });
                }
            });
    }

    fn report_view(&mut self, ui: &mut egui::Ui, report: &RemovalReport) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("✓").color(pal().ok).size(28.0).strong());
            ui.vertical(|ui| {
                ui.label(RichText::new("Uninstall complete").size(22.0).strong());
                ui.label(
                    RichText::new("The selected items were permanently deleted.")
                        .color(pal().dim)
                        .small(),
                );
            });
        });

        ui.add_space(14.0);
        ui.horizontal(|ui| {
            stat_card(ui, "Items removed", &format!("{}", report.removed.len()), pal().ok);
            stat_card(ui, "Space freed", &format_bytes(report.bytes_freed), pal().ok);
            stat_card(ui, "Duration", &format!("{} ms", report.elapsed_ms), pal().text);
            stat_card(
                ui,
                "Problems",
                &format!("{}", report.failed.len()),
                if report.failed.is_empty() { pal().ok } else { pal().danger },
            );
        });

        egui::ScrollArea::vertical()
            .id_salt("report_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if !report.refused.is_empty() {
                    ui.add_space(12.0);
                    ui.colored_label(
                        pal().warn,
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
                    ui.add_space(12.0);
                    ui.colored_label(
                        pal().danger,
                        format!("{} path(s) could not be removed:", report.failed.len()),
                    );
                    for failure in &report.failed {
                        ui.label(
                            RichText::new(format!(
                                "{} — {}",
                                failure.path.display(),
                                failure.message
                            ))
                            .monospace()
                            .small(),
                        );
                    }
                }

                ui.add_space(12.0);
                if let Some(audit) = &report.audit_log {
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
                        if ui.small_button("Copy path").clicked() {
                            ui.ctx().copy_text(dir.display().to_string());
                        }
                    });
                }
            });

        ui.add_space(14.0);
        if ui.button("← Back to app list").clicked() {
            self.report = None;
        }
    }

    fn confirm_modal(&mut self, ctx: &egui::Context) {
        match self.confirm {
            ConfirmStage::None => {}
            ConfirmStage::Review => self.confirm_review(ctx),
            ConfirmStage::Final => self.confirm_final(ctx),
        }
    }

    fn confirm_review(&mut self, ctx: &egui::Context) {
        let Some(plan) = self.plan.clone() else {
            self.confirm = ConfirmStage::None;
            return;
        };
        let selected: Vec<PathBuf> = self.checked.iter().cloned().collect();
        let bytes = plan.selected_bytes(&selected);
        let groups = group_categories(&plan);
        let mut advance = false;
        let mut cancel = false;

        let resp = egui::Modal::new(Id::new("confirm_review")).show(ctx, |ui| {
            ui.set_min_width(560.0);
            ui.label(RichText::new("Step 1 of 2 · Review").color(pal().accent).small().strong());
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!("Remove everything for “{}”?", plan.app.name))
                    .size(19.0)
                    .strong(),
            );
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                badge(ui, &format!("{} items", selected.len()), pal().accent, pal().accent_soft);
                badge(ui, &format_bytes(bytes), pal().accent, pal().accent_soft);
            });
            ui.add_space(6.0);
            ui.label(
                RichText::new("These are the exact items that will be deleted:")
                    .color(pal().dim),
            );
            ui.add_space(8.0);

            glass_card(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("review_scroll")
                    .max_height(300.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (category, indices) in &groups {
                            let sel: Vec<usize> = indices
                                .iter()
                                .copied()
                                .filter(|&i| self.checked.contains(&plan.candidates[i].path))
                                .collect();
                            if sel.is_empty() {
                                continue;
                            }
                            let cat_bytes: u64 =
                                sel.iter().map(|&i| plan.candidates[i].bytes).sum();
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(category.label()).strong().small());
                                ui.label(
                                    RichText::new(format!("({})", sel.len()))
                                        .small()
                                        .color(pal().dim),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(format_bytes(cat_bytes))
                                            .small()
                                            .color(pal().dim),
                                    );
                                });
                            });
                            for &i in &sel {
                                let candidate = &plan.candidates[i];
                                ui.horizontal(|ui| {
                                    ui.add_space(10.0);
                                    ui.label(
                                        RichText::new(shorten(
                                            &candidate.path.display().to_string(),
                                            58,
                                        ))
                                        .monospace()
                                        .small(),
                                    )
                                    .on_hover_text(candidate.path.display().to_string());
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.label(
                                            RichText::new(format_bytes(candidate.bytes))
                                                .small()
                                                .color(pal().dim),
                                        );
                                    });
                                });
                            }
                        }
                    });
            });

            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Continue →").color(primary_text()).strong(),
                            )
                            .fill(pal().accent_fill),
                        )
                        .clicked()
                    {
                        advance = true;
                    }
                });
            });
        });

        if cancel || resp.should_close() {
            self.confirm = ConfirmStage::None;
        } else if advance {
            self.confirm = ConfirmStage::Final;
            self.confirm_ack = false;
            self.confirm_name.clear();
        }
    }

    fn confirm_final(&mut self, ctx: &egui::Context) {
        let Some(plan) = self.plan.clone() else {
            self.confirm = ConfirmStage::None;
            return;
        };
        let selected: Vec<PathBuf> = self.checked.iter().cloned().collect();
        let bytes = plan.selected_bytes(&selected);
        let typed_ok = self
            .confirm_name
            .trim()
            .eq_ignore_ascii_case(plan.app.name.trim());
        let can_delete = self.confirm_ack && typed_ok && !selected.is_empty();
        let mut delete = false;
        let mut cancel = false;
        let mut back = false;

        let resp = egui::Modal::new(Id::new("confirm_final")).show(ctx, |ui| {
            ui.set_min_width(520.0);
            ui.label(
                RichText::new("Step 2 of 2 · Final confirmation")
                    .color(pal().danger)
                    .small()
                    .strong(),
            );
            ui.add_space(4.0);
            ui.label(RichText::new("This cannot be undone").size(19.0).strong());
            ui.add_space(8.0);
            ui.colored_label(
                pal().danger,
                format!(
                    "You are about to permanently delete {} item(s) ({}) for “{}”. \
                     Files are removed immediately — there is no trash and no undo. \
                     A JSONL audit log and an HTML/JSON report will be kept.",
                    selected.len(),
                    format_bytes(bytes),
                    plan.app.name
                ),
            );
            ui.add_space(12.0);

            ui.checkbox(
                &mut self.confirm_ack,
                "I understand this permanently deletes the listed items and cannot be undone.",
            );
            ui.add_space(10.0);
            ui.label(
                RichText::new(format!("Type “{}” below to confirm:", plan.app.name))
                    .color(pal().dim),
            );
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.confirm_name)
                    .hint_text(plan.app.name.clone())
                    .desired_width(f32::INFINITY),
            );

            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
                if ui.button("← Back").clicked() {
                    back = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            can_delete,
                            egui::Button::new(
                                RichText::new("Delete forever").color(primary_text()).strong(),
                            )
                            .fill(pal().danger_fill)
                            .min_size(Vec2::new(0.0, 30.0)),
                        )
                        .clicked()
                    {
                        delete = true;
                    }
                });
            });
        });

        if cancel || resp.should_close() {
            self.confirm = ConfirmStage::None;
            self.confirm_ack = false;
            self.confirm_name.clear();
        } else if back {
            self.confirm = ConfirmStage::Review;
        } else if delete {
            self.spawn_uninstall();
        }
    }

    fn about_modal(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let mut close = false;
        let version = env!("CARGO_PKG_VERSION");
        let resp = egui::Modal::new(Id::new("about_modal")).show(ctx, |ui| {
            ui.set_min_width(430.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("◆").color(pal().accent).size(30.0));
                ui.vertical(|ui| {
                    ui.label(RichText::new("PerfectUninstaller").size(21.0).strong());
                    ui.label(
                        RichText::new("Complete software removal")
                            .small()
                            .color(pal().dim),
                    );
                });
            });
            ui.add_space(12.0);
            ui.label(
                "Permanently removes an app together with its caches, preferences, containers, \
                 launch agents, receipts, crash reports and other residual traces — with a full \
                 audit log and an exportable report.",
            );
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Created by").color(pal().dim));
                ui.label(RichText::new("Enkivus").strong());
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("GitHub").color(pal().dim));
                ui.hyperlink_to("github.com/Enkivus/PerfectUninstaller", REPO_URL);
            });
            ui.add_space(10.0);
            ui.label(
                RichText::new(format!("Version {version} · {}", self.platform))
                    .small()
                    .color(pal().dim),
            );
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });

        if close || resp.should_close() {
            self.show_about = false;
        }
    }
}

fn install_theme(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    let p = pal();

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = p.bg;
    visuals.window_fill = p.window_fill;
    visuals.extreme_bg_color = rgba(12, 14, 18, 170);
    visuals.faint_bg_color = p.card;
    visuals.code_bg_color = p.sidebar;
    visuals.hyperlink_color = p.accent;
    visuals.warn_fg_color = p.warn;
    visuals.error_fg_color = p.danger;
    visuals.window_corner_radius = CornerRadius::same(p.radius + 4);
    visuals.window_stroke = Stroke::new(1.0, p.border);
    visuals.window_shadow = egui::Shadow {
        offset: [0, 12],
        blur: 34,
        spread: 0,
        color: Color32::from_black_alpha(120),
    };

    let r = CornerRadius::same(p.radius);
    visuals.widgets.noninteractive.bg_fill = p.card;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.card_stroke);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.noninteractive.corner_radius = r;

    visuals.widgets.inactive.bg_fill = p.control;
    visuals.widgets.inactive.weak_bg_fill = p.control;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, p.card_stroke);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.inactive.corner_radius = r;

    visuals.widgets.hovered.bg_fill = p.control_hover;
    visuals.widgets.hovered.weak_bg_fill = p.control_hover;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, p.border);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.hovered.corner_radius = r;

    visuals.widgets.active.bg_fill = p.control_active;
    visuals.widgets.active.weak_bg_fill = p.control_active;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, p.border);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, p.text);
    visuals.widgets.active.corner_radius = r;

    visuals.widgets.open = visuals.widgets.hovered;
    visuals.selection.bg_fill = p.accent_soft;
    visuals.selection.stroke = Stroke::new(1.0, p.accent);

    ctx.all_styles_mut(|style| {
        style.visuals = visuals.clone();
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(12.0, 7.0);
        style.spacing.window_margin = Margin::same(18);
        style.spacing.indent = 18.0;
        style.spacing.interact_size.y = 26.0;
    });
}

fn primary_text() -> Color32 {
    Color32::WHITE
}

fn glass_card(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(pal().card)
        .stroke(Stroke::new(1.0, pal().card_stroke))
        .corner_radius(CornerRadius::same(pal().radius))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, add_contents);
}

fn app_row(ui: &mut egui::Ui, app: &InstalledApp, selected: bool, analyzing: bool) -> egui::Response {
    let width = ui.available_width();
    let height = 52.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());

    if ui.is_rect_visible(rect) {
        let bg = if selected {
            pal().accent_soft
        } else if response.hovered() {
            pal().control_hover
        } else {
            Color32::TRANSPARENT
        };
        ui.painter()
            .rect_filled(rect, CornerRadius::same(pal().radius), bg);
        if selected {
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(pal().radius),
                Stroke::new(1.0, pal().border),
                egui::StrokeKind::Inside,
            );
            let bar = egui::Rect::from_min_size(
                rect.min + Vec2::new(0.0, 10.0),
                Vec2::new(3.0, height - 20.0),
            );
            ui.painter()
                .rect_filled(bar, CornerRadius::same(2), pal().accent);
        }

        let inner = egui::Rect::from_min_max(
            rect.min + Vec2::new(14.0, 8.0),
            rect.max - Vec2::new(14.0, 8.0),
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(inner)
                .layout(Layout::top_down(Align::Min)),
            |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&app.name).size(13.5).strong().color(pal().text));
                    if analyzing {
                        ui.add(egui::Spinner::new().size(12.0).color(pal().accent));
                    }
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if let Some(version) = &app.version {
                        ui.label(RichText::new(version).small().color(pal().dim));
                    }
                    ui.label(RichText::new(app.method.label()).small().color(pal().dim));
                });
            },
        );
    }

    response
}

fn badge(ui: &mut egui::Ui, text: &str, fg: Color32, bg: Color32) {
    egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0, pal().card_stroke))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).color(fg).small().strong());
        });
}

fn stat_card(ui: &mut egui::Ui, title: &str, value: &str, accent: Color32) {
    egui::Frame::new()
        .fill(pal().card)
        .stroke(Stroke::new(1.0, pal().card_stroke))
        .corner_radius(CornerRadius::same(pal().radius))
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_min_width(120.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(title).small().color(pal().dim));
                ui.label(RichText::new(value).size(18.0).strong().color(accent));
            });
        });
}

fn warning_row(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .fill(rgba(120, 90, 20, 90))
        .stroke(Stroke::new(1.0, rgba(255, 214, 10, 70)))
        .corner_radius(CornerRadius::same(pal().radius))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("⚠").color(pal().warn));
                ui.label(RichText::new(text).color(pal().warn).small());
            });
        });
}

fn confidence_style(confidence: Confidence) -> (&'static str, Color32) {
    match confidence {
        Confidence::High => ("● high", pal().ok),
        Confidence::Medium => ("● medium", pal().warn),
        Confidence::Low => ("● low", pal().dim),
    }
}

fn group_categories(plan: &RemovalPlan) -> Vec<(TraceCategory, Vec<usize>)> {
    let mut groups: Vec<(TraceCategory, Vec<usize>)> = Vec::new();
    for (i, candidate) in plan.candidates.iter().enumerate() {
        if let Some(group) = groups.iter_mut().find(|(cat, _)| *cat == candidate.category) {
            group.1.push(i);
        } else {
            groups.push((candidate.category, vec![i]));
        }
    }
    groups
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
            .with_inner_size([1240.0, 800.0])
            .with_min_inner_size([960.0, 600.0])
            .with_transparent(true),
        ..Default::default()
    };
    eframe::run_native(
        "PerfectUninstaller",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
