//! High-level wizard orchestrator — selects and runs the appropriate UI theme.

use crate::error::{Result, UiError};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use velocity_config::VelocityManifest;

/// Result from running the installer wizard.
#[derive(Debug, Clone)]
pub struct InstallWizardResult {
    pub install_dir: PathBuf,
    pub cancelled: bool,
    pub launch_after: bool,
    pub selected_components: Vec<String>,
    /// Whether the wizard already completed file extraction (native wizard mode)
    pub install_completed: bool,
}

/// Run the installer wizard based on the manifest's UI configuration.
pub fn run_install_wizard(manifest: &VelocityManifest) -> Result<InstallWizardResult> {
    run_install_wizard_with_payload(manifest, None)
}

/// Run the installer wizard with payload data for in-wizard extraction.
///
/// When `payload_data` is provided and the theme is "native"/"modern",
/// the wizard will perform extraction internally and show real progress.
pub fn run_install_wizard_with_payload(
    manifest: &VelocityManifest,
    _payload_data: Option<Vec<u8>>,
) -> Result<InstallWizardResult> {
    // On non-Windows, use the wry+tao GUI wizard
    #[cfg(not(target_os = "windows"))]
    {
        tracing::info!("Using wry+tao GUI wizard for non-Windows platform");
        return crate::wry_wizard::run_wry_wizard(manifest);
    }

    // On Windows, select the appropriate GUI wizard
    #[cfg(target_os = "windows")]
    {
        match manifest.ui.theme.as_str() {
            "classic" => run_classic(manifest),
            "modern" | "native" => {
                tracing::info!("Using native Win32 wizard for theme: {}", manifest.ui.theme);
                run_native_with_payload(manifest, _payload_data)
            }
            "webview" | "webview2" => {
                tracing::info!(
                    "Using WebView2 modern wizard for theme: {}",
                    manifest.ui.theme
                );
                run_webview(manifest)
            }
            _ => Err(UiError::Other(format!(
                "Unknown theme: {}",
                manifest.ui.theme
            ))),
        }
    }
}

// ===========================================================================
// Windows-specific wizard implementations
// ===========================================================================
#[cfg(target_os = "windows")]
fn run_classic(manifest: &VelocityManifest) -> Result<InstallWizardResult> {
    let result = crate::classic::run_classic_wizard(manifest)?;

    if result.cancelled {
        return Err(UiError::Cancelled);
    }

    Ok(InstallWizardResult {
        install_dir: result.install_dir,
        cancelled: false,
        launch_after: result.launch_after,
        selected_components: Vec::new(),
        install_completed: false,
    })
}

#[cfg(target_os = "windows")]
fn run_native_with_payload(
    manifest: &VelocityManifest,
    payload_data: Option<Vec<u8>>,
) -> Result<InstallWizardResult> {
    let result = crate::native_wizard::run_native_wizard(manifest, payload_data)?;

    if result.cancelled {
        return Err(UiError::Cancelled);
    }

    Ok(InstallWizardResult {
        install_dir: result.install_dir,
        cancelled: false,
        launch_after: result.launch_after,
        selected_components: result.selected_components,
        install_completed: result.install_completed,
    })
}

#[cfg(target_os = "windows")]
fn run_webview(manifest: &VelocityManifest) -> Result<InstallWizardResult> {
    let components: Vec<(String, String, String, f64, bool, bool)> = manifest
        .components
        .iter()
        .map(|c| {
            let size_mb = c.size as f64 / (1024.0 * 1024.0);
            (
                c.id.clone(),
                c.name.clone(),
                c.description.clone().unwrap_or_default(),
                size_mb,
                c.selected_by_default,
                c.mandatory,
            )
        })
        .collect();

    let license_text = manifest
        .app
        .license
        .as_ref()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default();

    match crate::modern::run_modern_wizard(
        &manifest.app.name,
        &manifest.app.version,
        &manifest.app.publisher,
        &manifest.install.default_dir,
        &license_text,
        &components,
    ) {
        Ok(result) => {
            if result.cancelled {
                return Err(UiError::Cancelled);
            }
            Ok(InstallWizardResult {
                install_dir: result.install_dir,
                cancelled: false,
                launch_after: result.launch_after,
                selected_components: result.selected_components,
                install_completed: result.install_completed,
            })
        }
        Err(UiError::WebView2NotAvailable) => {
            tracing::warn!(
                "WebView2 runtime not found — falling back to classic wizard for {}",
                manifest.app.name
            );
            crate::classic::show_message(
                &format!("{} Setup", manifest.app.name),
                "The modern wizard requires the Microsoft Edge WebView2 runtime.\n\n\
                 Falling back to the classic wizard. To install WebView2, visit:\n\
                 https://developer.microsoft.com/en-us/microsoft-edge/webview2/",
            );
            run_classic(manifest)
        }
        Err(e) => Err(e),
    }
}

// ===========================================================================
// Cross-platform progress and display utilities
// ===========================================================================

/// Progress tracker with ETA calculation.
///
/// Tracks installation progress and provides estimated time remaining
/// based on a rolling average of processing speed.
pub struct ProgressTracker {
    start_time: Instant,
    total_items: u64,
    completed_items: AtomicU64,
    last_update_ms: AtomicU64,
    /// Rolling average: files per millisecond (scaled by 1000 for precision)
    speed_scaled: AtomicU64,
}

impl ProgressTracker {
    /// Create a new progress tracker for the given number of total items.
    pub fn new(total_items: u64) -> Self {
        ProgressTracker {
            start_time: Instant::now(),
            total_items,
            completed_items: AtomicU64::new(0),
            last_update_ms: AtomicU64::new(0),
            speed_scaled: AtomicU64::new(0),
        }
    }

    /// Report that one more item has been completed.
    /// Returns (current, total, percentage, eta_string).
    pub fn tick(&self, file_name: &str) -> (u64, u64, u32, String) {
        let current = self.completed_items.fetch_add(1, Ordering::Relaxed) + 1;
        let elapsed_ms = self.start_time.elapsed().as_millis() as u64;

        if elapsed_ms > 0 && current > 1 {
            let speed = (current * 1000) / elapsed_ms;
            self.speed_scaled.store(speed, Ordering::Relaxed);
        }

        let pct = if self.total_items > 0 {
            ((current * 100) / self.total_items) as u32
        } else {
            0
        };

        let eta_str = self.calculate_eta(current, elapsed_ms);
        self.last_update_ms.store(elapsed_ms, Ordering::Relaxed);

        let display_name = if file_name.len() > 40 {
            format!("...{}", &file_name[file_name.len() - 37..])
        } else {
            file_name.to_string()
        };

        if current == self.total_items {
            let total_secs = elapsed_ms / 1000;
            tracing::info!(
                "Progress: [{}/{}] 100% - {} (completed in {}s)",
                current,
                self.total_items,
                display_name,
                total_secs
            );
        } else if current.is_multiple_of(10) || current <= 5 {
            tracing::info!(
                "Progress: [{}/{}] {}% - {} ETA: {}",
                current,
                self.total_items,
                pct,
                display_name,
                eta_str
            );
        }

        (current, self.total_items, pct, eta_str)
    }

    fn calculate_eta(&self, current: u64, elapsed_ms: u64) -> String {
        if current == 0 || elapsed_ms == 0 {
            return "calculating...".to_string();
        }
        let remaining = self.total_items.saturating_sub(current);
        let speed = self.speed_scaled.load(Ordering::Relaxed);
        if speed == 0 {
            return "calculating...".to_string();
        }
        let eta_ms = (remaining * 1000) / speed;
        format_duration(eta_ms)
    }

    /// Get the total elapsed time as a formatted string.
    pub fn elapsed(&self) -> String {
        format_duration(self.start_time.elapsed().as_millis() as u64)
    }

    /// Get current progress as a fraction.
    pub fn progress(&self) -> (u64, u64) {
        (
            self.completed_items.load(Ordering::Relaxed),
            self.total_items,
        )
    }
}

/// Format a duration in milliseconds to a human-readable string.
fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60_000 {
        let secs = ms / 1000;
        format!("{}s", secs)
    } else {
        let mins = ms / 60_000;
        let secs = (ms % 60_000) / 1000;
        format!("{}m {}s", mins, secs)
    }
}

/// Show installation progress (terminal-based).
pub fn show_progress(current: usize, total: usize, file_name: &str) {
    let pct = if total > 0 {
        (current * 100) / total
    } else {
        0
    };
    print!("\r[{}/{}] {}% - {}", current, total, pct, file_name);
    if current == total {
        println!();
    }
}

/// Show installation complete message.
#[cfg(target_os = "windows")]
pub fn show_complete(app_name: &str, install_dir: &std::path::Path) {
    crate::classic::show_message(
        &format!("{} Installed", app_name),
        &format!(
            "{} has been successfully installed to:\n\n{}\n\nClick OK to finish.",
            app_name,
            install_dir.display()
        ),
    );
}

#[cfg(not(target_os = "windows"))]
pub fn show_complete(app_name: &str, install_dir: &std::path::Path) {
    crate::cross_platform::show_complete(app_name, install_dir);
}

/// Show an error message.
#[cfg(target_os = "windows")]
pub fn show_error(title: &str, message: &str) {
    crate::classic::show_error(title, message);
}

#[cfg(not(target_os = "windows"))]
pub fn show_error(title: &str, message: &str) {
    crate::cross_platform::show_error(title, message);
}

/// Show the finish dialog with option to launch the app.
#[cfg(target_os = "windows")]
pub fn show_finish(app_name: &str, install_dir: &std::path::Path, run_after: Option<&str>) -> bool {
    crate::classic::show_finish_dialog(app_name, install_dir, run_after)
}

#[cfg(not(target_os = "windows"))]
pub fn show_finish(app_name: &str, install_dir: &std::path::Path, run_after: Option<&str>) -> bool {
    crate::cross_platform::show_finish(app_name, install_dir, run_after)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_ms() {
        assert_eq!(format_duration(500), "500ms");
    }

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(5000), "5s");
        assert_eq!(format_duration(30000), "30s");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(90000), "1m 30s");
        assert_eq!(format_duration(125000), "2m 5s");
    }

    #[test]
    fn test_progress_tracker() {
        let tracker = ProgressTracker::new(100);
        let (current, total, pct, _eta) = tracker.tick("file1.txt");
        assert_eq!(current, 1);
        assert_eq!(total, 100);
        assert_eq!(pct, 1);
    }

    #[test]
    fn test_progress_tracker_complete() {
        let tracker = ProgressTracker::new(3);
        tracker.tick("a");
        tracker.tick("b");
        let (current, total, pct, _) = tracker.tick("c");
        assert_eq!(current, 3);
        assert_eq!(total, 3);
        assert_eq!(pct, 100);
    }
}
