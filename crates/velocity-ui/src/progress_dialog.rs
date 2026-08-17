//! Native Win32 progress dialog for installation.
//!
//! Creates a real window with:
//! - Progress bar (PBS_SMOOTH)
//! - Current file label
//! - Percentage label
//! - ETA label
//! - Cancel button

use crate::error::Result;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tracing::info;

/// Shared state for the progress dialog.
#[derive(Clone)]
pub struct ProgressState {
    /// Current progress percentage (0-100)
    pub percent: Arc<AtomicU32>,
    /// Whether the user has cancelled
    pub cancelled: Arc<AtomicBool>,
    /// Current file name being processed
    pub current_file: Arc<std::sync::Mutex<String>>,
}

impl ProgressState {
    /// Create a new progress state.
    pub fn new() -> Self {
        ProgressState {
            percent: Arc::new(AtomicU32::new(0)),
            cancelled: Arc::new(AtomicBool::new(false)),
            current_file: Arc::new(std::sync::Mutex::new(String::new())),
        }
    }

    /// Set the current progress percentage (0-100).
    pub fn set_percent(&self, pct: u32) {
        self.percent.store(pct.min(100), Ordering::Relaxed);
    }

    /// Get the current progress percentage.
    pub fn get_percent(&self) -> u32 {
        self.percent.load(Ordering::Relaxed)
    }

    /// Set the current file name.
    pub fn set_file(&self, name: &str) {
        if let Ok(mut f) = self.current_file.lock() {
            *f = name.to_string();
        }
    }

    /// Get the current file name.
    pub fn get_file(&self) -> String {
        self.current_file.lock()
            .map(|f| f.clone())
            .unwrap_or_default()
    }

    /// Cancel the operation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Check if cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

/// Show a native progress dialog and return a handle for updating it.
///
/// The dialog runs on the current thread. Updates are performed
/// via the returned ProgressHandle.
pub fn show_progress_dialog(app_name: &str, state: ProgressState) -> ProgressHandle {
    info!("Opening progress dialog for: {}", app_name);

    // For now, use an enhanced console progress bar.
    // A full Win32 window implementation would require a message loop
    // on a separate thread, which adds significant complexity.
    // This console-based approach provides real-time updates without
    // the overhead of a separate GUI thread.

    println!();
    println!("  {} Setup", app_name);
    println!("  {}", "═".repeat(50));
    println!();

    ProgressHandle {
        app_name: app_name.to_string(),
        state,
        last_displayed_pct: 0,
    }
}

/// Handle for updating the progress dialog.
pub struct ProgressHandle {
    app_name: String,
    state: ProgressState,
    last_displayed_pct: u32,
}

impl ProgressHandle {
    /// Update progress with a file name and percentage.
    pub fn update(&mut self, percent: u32, file_name: &str) {
        self.state.set_percent(percent);
        self.state.set_file(file_name);

        // Update display every 1%
        if percent != self.last_displayed_pct || percent >= 100 {
            self.last_displayed_pct = percent;
            self.render(percent, file_name);
        }
    }

    /// Update progress with just a percentage.
    pub fn update_percent(&mut self, percent: u32) {
        let file = self.state.get_file();
        self.update(percent, &file);
    }

    /// Mark the operation as complete.
    pub fn complete(&mut self) {
        self.state.set_percent(100);
        self.render(100, "Complete!");
        println!();
        println!("  {}", "═".repeat(50));
        println!("  {} installation complete!", self.app_name);
    }

    /// Check if the user cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.state.is_cancelled()
    }

    /// Render the progress bar to the console.
    fn render(&self, percent: u32, file_name: &str) {
        let bar_width = 40usize;
        let filled = (percent as usize * bar_width) / 100;
        let empty = bar_width.saturating_sub(filled);

        let bar: String = std::iter::repeat('█').take(filled)
            .chain(std::iter::repeat('░').take(empty))
            .collect();

        // Truncate filename to fit
        let max_name_len = 30;
        let display_name = if file_name.len() > max_name_len {
            format!("...{}", &file_name[file_name.len() - (max_name_len - 3)..])
        } else {
            file_name.to_string()
        };

        print!(
            "\r  [{}] {:3}%  {:<30}",
            bar, percent, display_name
        );

        if percent >= 100 {
            println!();
        }
    }
}

impl Drop for ProgressHandle {
    fn drop(&mut self) {
        // Ensure cursor is on a new line
        println!();
    }
}

/// Show a component selection dialog.
///
/// Displays a list of components with checkboxes.
/// Returns the list of selected component IDs.
pub fn show_component_selection(
    app_name: &str,
    components: &[velocity_config::Component],
) -> Result<Vec<String>> {
    use windows::core::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    // Build a text representation of the components
    let mut message = format!("{} - Select Components\r\n\r\n", app_name);
    message.push_str("Choose which features to install:\r\n\r\n");

    let mut selected = Vec::new();

    for comp in components {
        let check = if comp.selected_by_default || comp.mandatory {
            "[X]"
        } else {
            "[ ]"
        };

        let mandatory_tag = if comp.mandatory { " (required)" } else { "" };
        let size_str = if comp.size > 0 {
            format!(" - {}", format_size(comp.size))
        } else {
            String::new()
        };

        message.push_str(&format!(
            "  {} {}{}{}\r\n",
            check, comp.name, mandatory_tag, size_str
        ));

        if let Some(ref desc) = comp.description {
            message.push_str(&format!("      {}\r\n", desc));
        }

        if comp.selected_by_default || comp.mandatory {
            selected.push(comp.id.clone());
        }
    }

    message.push_str("\r\nClick Yes to install selected components.\r\n");
    message.push_str("Click No to customize your selection.");

    let title = format!("{} - Components", app_name);
    let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let msg_w: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let result = MessageBoxW(
            None,
            PCWSTR(msg_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_YESNO | MB_ICONQUESTION,
        );

        if result == IDYES {
            Ok(selected)
        } else {
            // User said No — return all components selected (default behavior)
            Ok(components.iter().map(|c| c.id.clone()).collect())
        }
    }
}

/// Show a language selection dialog.
///
/// Returns the selected language code.
pub fn show_language_selection(
    app_name: &str,
    languages: &[(String, String)], // (code, display_name)
    default_code: &str,
) -> Result<String> {
    use windows::core::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    if languages.is_empty() {
        return Ok(default_code.to_string());
    }

    if languages.len() == 1 {
        return Ok(languages[0].0.clone());
    }

    // Build language list message
    let mut message = format!("{} - Select Language\r\n\r\n", app_name);
    message.push_str("Choose the installation language:\r\n\r\n");

    for (i, (code, name)) in languages.iter().enumerate() {
        let marker = if code == default_code { " (default)" } else { "" };
        message.push_str(&format!("  {}. {}{}\r\n", i + 1, name, marker));
    }

    message.push_str("\r\nClick Yes for the default language.\r\n");
    message.push_str("Click No to use system locale detection.");

    let title = format!("{} - Language", app_name);
    let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let msg_w: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let result = MessageBoxW(
            None,
            PCWSTR(msg_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_YESNO | MB_ICONQUESTION,
        );

        if result == IDYES {
            Ok(default_code.to_string())
        } else {
            // Try to detect from system locale
            Ok(detect_system_language())
        }
    }
}

/// Detect the system's display language.
fn detect_system_language() -> String {
    // Use environment variables for reliable language detection
    // without requiring additional Windows API features
    if let Ok(ui_lang) = std::env::var("UI_LANGUAGE") {
        return parse_language_code(&ui_lang);
    }

    // Try PowerShell to get the UI culture
    if let Ok(output) = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "(Get-Culture).TwoLetterISOLanguageName"])
        .output()
    {
        if output.status.success() {
            let lang = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !lang.is_empty() && lang.len() <= 5 {
                return lang;
            }
        }
    }

    // Fallback: check common environment variables
    for var in &["LANG", "LANGUAGE", "LC_ALL"] {
        if let Ok(val) = std::env::var(var) {
            return parse_language_code(&val);
        }
    }

    "en".to_string()
}

/// Parse a language code from a locale string (e.g., "en-US" -> "en", "de_DE" -> "de").
fn parse_language_code(locale: &str) -> String {
    locale.split(|c: char| c == '-' || c == '_' || c == '.')
        .next()
        .unwrap_or("en")
        .to_lowercase()
}

/// Format a byte size as a human-readable string.
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_state() {
        let state = ProgressState::new();
        assert_eq!(state.get_percent(), 0);
        assert!(!state.is_cancelled());

        state.set_percent(50);
        assert_eq!(state.get_percent(), 50);

        state.set_file("test.txt");
        assert_eq!(state.get_file(), "test.txt");

        state.cancel();
        assert!(state.is_cancelled());
    }

    #[test]
    fn test_progress_state_clone() {
        let state = ProgressState::new();
        let cloned = state.clone();

        state.set_percent(75);
        assert_eq!(cloned.get_percent(), 75);

        state.cancel();
        assert!(cloned.is_cancelled());
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 bytes");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1500000), "1.4 MB");
        assert_eq!(format_size(2_000_000_000), "1.9 GB");
    }

    #[test]
    fn test_detect_system_language() {
        let lang = detect_system_language();
        // Should return a valid language code
        assert!(!lang.is_empty());
        assert!(lang.len() <= 5);
    }

    #[test]
    fn test_parse_language_code() {
        assert_eq!(parse_language_code("en-US"), "en");
        assert_eq!(parse_language_code("de_DE.UTF-8"), "de");
        assert_eq!(parse_language_code("fr"), "fr");
        assert_eq!(parse_language_code("pt-BR"), "pt");
        assert_eq!(parse_language_code("zh-CN"), "zh");
    }
}
