use std::path::PathBuf;

use crate::error::Result;
use crate::models::{InstalledApp, Platform, RemovalPlan};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

/// Everything the engine needs from an operating system: what is installed,
/// where its leftovers live, and which directories we are allowed to touch.
pub trait PlatformBackend: Send + Sync {
    fn platform(&self) -> Platform;

    /// Software installed on this machine.
    fn discover(&self) -> Result<Vec<InstalledApp>>;

    /// Full set of removable traces for one app.
    fn analyze(&self, app: &InstalledApp) -> Result<RemovalPlan>;

    /// Directories the engine may search and delete inside. Removal refuses
    /// any path outside these roots plus the app's own install paths.
    fn trace_roots(&self) -> Vec<PathBuf>;
}

pub fn backend() -> Box<dyn PlatformBackend> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacosBackend)
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsBackend)
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxBackend)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        struct Unsupported;
        impl PlatformBackend for Unsupported {
            fn platform(&self) -> Platform {
                Platform::Unknown
            }
            fn discover(&self) -> Result<Vec<InstalledApp>> {
                Err(crate::error::Error::Unsupported("no backend for this OS"))
            }
            fn analyze(&self, _: &InstalledApp) -> Result<RemovalPlan> {
                Err(crate::error::Error::Unsupported("no backend for this OS"))
            }
            fn trace_roots(&self) -> Vec<PathBuf> {
                Vec::new()
            }
        }
        Box::new(Unsupported)
    }
}
