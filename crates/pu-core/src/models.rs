use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    Macos,
    Windows,
    Linux,
    Unknown,
}

impl Platform {
    pub fn name(self) -> &'static str {
        match self {
            Platform::Macos => "macOS",
            Platform::Windows => "Windows",
            Platform::Linux => "Linux",
            Platform::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstallMethod {
    AppBundle,
    PkgReceipt,
    Caskroom,
    Msi,
    Appx,
    Store,
    Deb,
    Rpm,
    Flatpak,
    Snap,
    Steam,
    Package,
    Manual,
    Unknown,
}

impl InstallMethod {
    pub fn label(self) -> &'static str {
        match self {
            InstallMethod::AppBundle => "Application bundle",
            InstallMethod::PkgReceipt => "Installer package receipt",
            InstallMethod::Caskroom => "Homebrew cask",
            InstallMethod::Msi => "Windows installer (MSI)",
            InstallMethod::Appx => "Appx package",
            InstallMethod::Store => "Store app",
            InstallMethod::Deb => "Debian package",
            InstallMethod::Rpm => "RPM package",
            InstallMethod::Flatpak => "Flatpak",
            InstallMethod::Snap => "Snap",
            InstallMethod::Steam => "Steam",
            InstallMethod::Package => "System package",
            InstallMethod::Manual => "Manual install",
            InstallMethod::Unknown => "Unknown",
        }
    }
}

/// A piece of software the engine discovered on this machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledApp {
    /// Stable key used by the GUI and reports (path or identifier based).
    pub id: String,
    pub name: String,
    /// Bundle id / package id / registry key name, when known.
    pub identifier: Option<String>,
    pub version: Option<String>,
    /// Primary locations the software is installed into. These are always
    /// eligible for removal in addition to the discovered traces.
    pub install_paths: Vec<PathBuf>,
    pub method: InstallMethod,
    pub platform: Platform,
    /// Size of install_paths in bytes, if measured during discovery.
    pub install_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Confidence {
    Low,
    Medium,
    High,
}

impl Confidence {
    pub fn label(self) -> &'static str {
        match self {
            Confidence::Low => "low",
            Confidence::Medium => "medium",
            Confidence::High => "high",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceCategory {
    Application,
    Preference,
    Cache,
    Support,
    Log,
    Container,
    SavedState,
    HttpStorage,
    WebKit,
    LaunchAgent,
    LaunchDaemon,
    PrivilegedHelper,
    Receipt,
    ShaderCache,
    Registry,
    Service,
    Other,
}

impl TraceCategory {
    pub fn label(self) -> &'static str {
        match self {
            TraceCategory::Application => "Application",
            TraceCategory::Preference => "Preferences",
            TraceCategory::Cache => "Caches",
            TraceCategory::Support => "Application support",
            TraceCategory::Log => "Logs",
            TraceCategory::Container => "Sandbox container",
            TraceCategory::SavedState => "Saved state",
            TraceCategory::HttpStorage => "HTTP storage",
            TraceCategory::WebKit => "WebKit data",
            TraceCategory::LaunchAgent => "Launch agent",
            TraceCategory::LaunchDaemon => "Launch daemon",
            TraceCategory::PrivilegedHelper => "Privileged helper",
            TraceCategory::Receipt => "Install receipt",
            TraceCategory::ShaderCache => "Shader cache",
            TraceCategory::Registry => "Registry",
            TraceCategory::Service => "Service",
            TraceCategory::Other => "Other",
        }
    }
}

/// A file or directory left behind by an app (or the app itself).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceCandidate {
    pub path: PathBuf,
    pub category: TraceCategory,
    pub confidence: Confidence,
    pub bytes: u64,
    /// Human readable explanation of why this path matched.
    pub reason: String,
    /// True when the path is the primary install location (always removable).
    pub primary: bool,
}

/// The full picture of everything that would be deleted for one app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovalPlan {
    pub app: InstalledApp,
    pub candidates: Vec<TraceCandidate>,
    pub total_bytes: u64,
    /// Non-fatal issues worth showing before deletion.
    pub warnings: Vec<String>,
}

impl RemovalPlan {
    pub fn selected_bytes(&self, selected: &[std::path::PathBuf]) -> u64 {
        self.candidates
            .iter()
            .filter(|c| selected.contains(&c.path))
            .map(|c| c.bytes)
            .sum()
    }
}

/// Result of executing a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovalReport {
    pub removed: Vec<PathBuf>,
    pub bytes_freed: u64,
    pub failed: Vec<RemovalFailure>,
    pub refused: Vec<PathBuf>,
    pub audit_log: Option<PathBuf>,
    pub elapsed_ms: u128,
}

impl RemovalReport {
    pub fn success(&self) -> bool {
        self.failed.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovalFailure {
    pub path: PathBuf,
    pub message: String,
}
