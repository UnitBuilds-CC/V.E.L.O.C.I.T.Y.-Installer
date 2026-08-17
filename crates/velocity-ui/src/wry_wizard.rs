//! Cross-platform WebView wizard using wry + tao.
//!
//! Provides the same modern HTML/CSS wizard UI as the Windows WebView2
//! wizard, but rendered via wry (WebKit on Linux, WKWebView on macOS).
//! Uses tao for window management and event loop.
//!
//! ## IPC compatibility
//!
//! The wizard HTML was designed for WebView2's `chrome.webview` IPC API.
//! We inject a JavaScript compatibility shim that maps those calls to
//! wry's `window.ipc.postMessage()` so the same HTML works on both.

use crate::error::UiError;
use crate::wizard::InstallWizardResult;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::platform::run_return::EventLoopExtRunReturn;
use tao::window::WindowBuilder;
use tracing::{error, info, warn};
use velocity_config::VelocityManifest;
use wry::WebViewBuilder;

/// Result from the wry wizard.
#[derive(Debug, Clone)]
pub struct WryWizardResult {
    pub install_dir: PathBuf,
    pub cancelled: bool,
    pub launch_after: bool,
    pub selected_components: Vec<String>,
}

/// Wizard state shared between Rust and JavaScript.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WizardState {
    page: String,
    app_name: String,
    app_version: String,
    publisher: String,
    install_dir: String,
    default_dir: String,
    license_text: String,
    components: Vec<ComponentItem>,
    selected_components: Vec<String>,
    theme: String,
    progress_percent: u32,
    progress_file: String,
    cancelled: bool,
    launch_after: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ComponentItem {
    id: String,
    name: String,
    description: String,
    size_mb: f64,
    selected: bool,
    mandatory: bool,
}

/// Messages from JavaScript to Rust.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
enum JsMessage {
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "next")]
    Next,
    #[serde(rename = "back")]
    Back,
    #[serde(rename = "install")]
    Install,
    #[serde(rename = "cancel")]
    Cancel,
    #[serde(rename = "browse")]
    Browse,
    #[serde(rename = "set_dir")]
    SetDir(String),
    #[serde(rename = "toggle_component")]
    ToggleComponent(String),
    #[serde(rename = "finish")]
    Finish { launch: bool },
    #[serde(rename = "get_state")]
    GetState,
}

/// Messages from the installation background thread to the event loop.
enum InstallThreadMsg {
    Progress(u32, String),
    Completed,
    Error(String),
}

/// Run the cross-platform wry+tao wizard on Linux/macOS.
///
/// Creates a native window with a WebView that renders the same modern
/// wizard HTML as the Windows WebView2 wizard. An IPC shim layer ensures
/// the JavaScript works identically across both backends.
///
/// When `install_fn` and `payload_data` are provided, the wizard runs
/// the installation in a background thread and shows real progress.
pub fn run_wry_wizard<F, E>(
    manifest: &VelocityManifest,
    payload_data: Option<Vec<u8>>,
    install_fn: Option<F>,
) -> std::result::Result<InstallWizardResult, UiError>
where
    F: Fn(
            &VelocityManifest,
            &std::path::Path,
            &[u8],
            fn(u32, String),
        ) -> std::result::Result<(), E>
        + Send
        + 'static,
    E: std::fmt::Display + Send + 'static,
{
    info!(
        "Starting wry+tao wizard for {} v{}",
        manifest.app.name, manifest.app.version
    );

    // On Linux, GTK must be initialized before creating any windows
    #[cfg(target_os = "linux")]
    {
        if !gtk::init_check() {
            error!("Failed to initialize GTK — cannot create wizard window");
            return Err(UiError::WindowCreation(
                "GTK initialization failed. Is a display server running?".into(),
            ));
        }
    }

    // Detect theme from environment
    let theme = detect_system_theme();
    info!("System theme: {}", theme);

    // Build component list from manifest
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

    // Read license text
    let license_text = manifest
        .app
        .license
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();

    // Compute default install dir
    let default_dir = velocity_core::platform::default_install_dir(&manifest.app.name)
        .to_string_lossy()
        .to_string();

    // Build initial state
    let state = Arc::new(Mutex::new(WizardState {
        page: "welcome".to_string(),
        app_name: manifest.app.name.clone(),
        app_version: manifest.app.version.clone(),
        publisher: manifest.app.publisher.clone(),
        install_dir: default_dir.clone(),
        default_dir,
        license_text,
        components: components
            .iter()
            .map(|(id, name, desc, size, sel, mand)| ComponentItem {
                id: id.clone(),
                name: name.clone(),
                description: desc.clone(),
                size_mb: *size,
                selected: *sel,
                mandatory: *mand,
            })
            .collect(),
        selected_components: components
            .iter()
            .filter(|(_, _, _, _, sel, _)| *sel)
            .map(|(id, _, _, _, _, _)| id.clone())
            .collect(),
        theme,
        progress_percent: 0,
        progress_file: String::new(),
        cancelled: false,
        launch_after: false,
    }));

    // Generate HTML with cross-platform IPC shim
    let base_html = crate::wizard_html::generate_wizard_html(
        &state
            .lock()
            .map_err(|_| UiError::Wizard("Lock poisoned".into()))?
            .theme,
    );
    let html = inject_ipc_shim(&base_html);

    // Create the tao event loop and window
    let mut event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(format!("{} - Setup", manifest.app.name))
        .with_inner_size(tao::dpi::LogicalSize::new(800.0, 600.0))
        .with_resizable(false)
        .build(&event_loop)
        .map_err(|e| UiError::WindowCreation(format!("{}", e)))?;

    // Build the webview with IPC handler
    // Use a channel to forward browse requests to the event loop
    let (browse_tx, browse_rx) = mpsc::channel::<()>();
    let webview = WebViewBuilder::new()
        .with_html(html)
        .with_ipc_handler({
            let state = state.clone();
            let browse_tx = browse_tx.clone();
            let install_req_tx = install_req_tx.clone();
            move |request| {
                let json = request.body().clone();
                handle_ipc_message(&state, &json, &browse_tx, &install_req_tx);
            }
        })
        .build(&window)
        .map_err(|e| UiError::WebView(format!("Failed to create webview: {}", e)))?;

    // Keep webview alive for the event loop
    let webview = Arc::new(Mutex::new(webview));

    // Channel for progress updates from the installation background thread
    let (progress_tx, progress_rx) = mpsc::channel::<InstallThreadMsg>();
    // Channel for install requests from IPC handler to event loop
    let (install_req_tx, install_req_rx) = mpsc::channel::<()>();

    // Send initial state to JS once the page loads
    {
        let st = state.lock().unwrap();
        let state_json = serde_json::to_string(&*st).unwrap_or_default();
        let script = format!(
            "if(window.__velocityState)window.__velocityState({})",
            state_json
        );
        if let Ok(wv) = webview.lock() {
            let _ = wv.evaluate_script(&script);
        }
    }

    // Run the event loop (returns when window is closed)
    let state_close = state.clone();
    let webview_close = webview.clone();
    let mut installing = false;
    let mut install_thread: Option<std::thread::JoinHandle<()>> = None;
    let state_install = state.clone();
    event_loop.run_return(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;

        // On Linux, pump GTK main loop iterations alongside tao
        #[cfg(target_os = "linux")]
        {
            while gtk::events_pending() {
                gtk::main_iteration();
            }
        }

        // Process progress updates from the installation background thread
        while let Ok(msg) = progress_rx.try_recv() {
            match msg {
                InstallThreadMsg::Progress(pct, file) => {
                    if let Ok(mut st) = state_close.lock() {
                        st.progress_percent = pct;
                        st.progress_file = file;
                    }
                    push_state_to_webview(&state_close, &webview_close);
                }
                InstallThreadMsg::Completed => {
                    installing = false;
                    if let Ok(mut st) = state_close.lock() {
                        st.progress_percent = 100;
                        st.progress_file = "Installation complete".to_string();
                        st.page = "finish".to_string();
                    }
                    push_state_to_webview(&state_close, &webview_close);
                }
                InstallThreadMsg::Error(e) => {
                    installing = false;
                    error!("Installation failed: {}", e);
                    if let Ok(mut st) = state_close.lock() {
                        st.progress_file = format!("Error: {}", e);
                        st.page = "finish".to_string();
                    }
                    push_state_to_webview(&state_close, &webview_close);
                }
            }
        }

        // Process browse folder dialog requests from the IPC handler
        if let Ok(()) = browse_rx.try_recv() {
            if let Some(dir) = show_folder_dialog() {
                // Update state
                if let Ok(mut st) = state_close.lock() {
                    st.install_dir = dir.clone();
                }
                // Update the directory input in the webview
                let escaped = dir.replace('\\', "\\\\").replace('\'', "\\'");
                if let Ok(wv) = webview_close.lock() {
                    let script =
                        format!("document.getElementById('dir-input').value='{}'", escaped);
                    let _ = wv.evaluate_script(&script);
                }
            }
        }

        // Process install requests from the IPC handler
        if let Ok(()) = install_req_rx.try_recv() {
            if let (Some(payload), Some(install_fn)) = (payload_data.take(), install_fn.take()) {
                installing = true;
                let tx = progress_tx.clone();
                let install_dir = state_install
                    .lock()
                    .map(|s| s.install_dir.clone())
                    .unwrap_or_default();
                let manifest_clone = manifest.clone();
                let handle = std::thread::spawn(move || {
                    let progress_cb: fn(u32, String) = {
                        let tx = tx.clone();
                        move |pct, msg| {
                            let _ = tx.send(InstallThreadMsg::Progress(pct, msg));
                        }
                    };
                    match install_fn(
                        &manifest_clone,
                        &std::path::PathBuf::from(&install_dir),
                        &payload,
                        progress_cb,
                    ) {
                        Ok(()) => {
                            let _ = tx.send(InstallThreadMsg::Completed);
                        }
                        Err(e) => {
                            let _ = tx.send(InstallThreadMsg::Error(format!("{}", e)));
                        }
                    }
                });
                install_thread = Some(handle);
            } else {
                // No install function — skip to finish
                if let Ok(mut st) = state_close.lock() {
                    st.progress_percent = 100;
                    st.progress_file = "Installation complete".to_string();
                    st.page = "finish".to_string();
                }
                push_state_to_webview(&state_close, &webview_close);
            }
        }

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if installing {
                    // Don't allow closing during installation
                    return;
                }
                let mut st = state_close.lock().unwrap();
                st.cancelled = true;
                *control_flow = ControlFlow::ExitWithCode(0);
            }
            _ => {}
        }
    });

    // Wait for install thread to finish (should already be done at this point)
    if let Some(handle) = install_thread {
        let _ = handle.join();
    }

    // Extract result from shared state
    let final_state = state
        .lock()
        .map_err(|_| UiError::Wizard("Lock poisoned".into()))?;

    // Determine if installation completed successfully
    let install_completed = !final_state.cancelled
        && final_state.progress_percent == 100
        && !final_state.progress_file.starts_with("Error");

    Ok(InstallWizardResult {
        install_dir: PathBuf::from(&final_state.install_dir),
        cancelled: final_state.cancelled,
        launch_after: final_state.launch_after,
        selected_components: final_state.selected_components.clone(),
        install_completed,
    })
}

/// Push the current wizard state to the JavaScript frontend.
fn push_state_to_webview(state: &Arc<Mutex<WizardState>>, webview: &Arc<Mutex<wry::WebView>>) {
    if let (Ok(st), Ok(wv)) = (state.lock(), webview.lock()) {
        if let Ok(state_json) = serde_json::to_string(&*st) {
            let script = format!(
                "if(window.__velocityState)window.__velocityState({})",
                state_json
            );
            let _ = wv.evaluate_script(&script);
        }
    }
}

/// Inject the cross-platform IPC compatibility shim into the HTML.
///
/// Replaces `chrome.webview.*` calls with abstraction functions that
/// work on both WebView2 (Windows) and wry (Linux/macOS).
fn inject_ipc_shim(html: &str) -> String {
    let shim = r#"
// Cross-platform IPC shim (wry + WebView2)
window.__velocityState = function(state) {
    var evt = { data: JSON.stringify({ type: 'state', data: state }) };
    if (window.__velocityHandler) window.__velocityHandler(evt);
};
function sendMsg(obj) {
    var json = JSON.stringify(obj);
    if (typeof chrome !== 'undefined' && chrome.webview) {
        chrome.webview.postMessage(json);
    } else if (window.ipc) {
        window.ipc.postMessage(json);
    }
}
function onMsg(fn) { window.__velocityHandler = fn; }
"#;

    let mut result = html.replace("<script>\n", &format!("<script>\n{}\n", shim));

    // Replace chrome.webview.postMessage(...) with sendMsg(...)
    result = result.replace("chrome.webview.postMessage(", "sendMsg(");

    // Replace the event listener registration
    result = result.replace(
        "chrome.webview.addEventListener('message', function(event)",
        "onMsg(function(event)",
    );

    result
}

/// Handle an IPC message from JavaScript.
fn handle_ipc_message(
    state: &Arc<Mutex<WizardState>>,
    json: &str,
    browse_tx: &mpsc::Sender<()>,
    install_req_tx: &mpsc::Sender<()>,
) {
    let msg: std::result::Result<JsMessage, _> = serde_json::from_str(json);
    match msg {
        Ok(JsMessage::Ready) => {
            info!("Wry wizard: JS ready");
        }
        Ok(JsMessage::Next) => {
            let mut st = match state.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            let pages = [
                "welcome",
                "license",
                "directory",
                "components",
                "progress",
                "finish",
            ];
            if let Some(idx) = pages.iter().position(|p| *p == st.page) {
                if idx + 1 < pages.len() {
                    st.page = pages[idx + 1].to_string();
                }
            }
        }
        Ok(JsMessage::Back) => {
            let mut st = match state.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            let pages = [
                "welcome",
                "license",
                "directory",
                "components",
                "progress",
                "finish",
            ];
            if let Some(idx) = pages.iter().position(|p| *p == st.page) {
                if idx > 0 {
                    st.page = pages[idx - 1].to_string();
                }
            }
        }
        Ok(JsMessage::Install) => {
            let mut st = match state.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            st.page = "progress".to_string();
            st.progress_percent = 0;
            st.progress_file = "Preparing installation...".to_string();
            // Forward install request to event loop for thread spawning
            let _ = install_req_tx.send(());
        }
        Ok(JsMessage::Cancel) => {
            let mut st = match state.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            st.cancelled = true;
        }
        Ok(JsMessage::Browse) => {
            // Forward to event loop for native folder dialog
            let _ = browse_tx.send(());
        }
        Ok(JsMessage::SetDir(dir)) => {
            let mut st = match state.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            st.install_dir = dir;
        }
        Ok(JsMessage::ToggleComponent(id)) => {
            let mut st = match state.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            if let Some(comp) = st.components.iter_mut().find(|c| c.id == id) {
                if !comp.mandatory {
                    comp.selected = !comp.selected;
                }
            }
            st.selected_components = st
                .components
                .iter()
                .filter(|c| c.selected)
                .map(|c| c.id.clone())
                .collect();
        }
        Ok(JsMessage::Finish { launch }) => {
            let mut st = match state.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            st.launch_after = launch;
            st.cancelled = false;
        }
        Ok(JsMessage::GetState) => {
            // State is sent periodically
        }
        Err(e) => {
            warn!("Failed to parse wry IPC message: {} (json: {})", e, json);
        }
    }
}

/// Show a native folder selection dialog.
///
/// Uses `zenity` on Linux (standard on most desktop environments) and
/// `osascript` on macOS (built-in). Returns `None` if the user cancels
/// or if no dialog tool is available.
fn show_folder_dialog() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        // Try zenity first (GNOME/GTK standard)
        if let Ok(output) = std::process::Command::new("zenity")
            .args([
                "--file-selection",
                "--directory",
                "--title=Select Installation Directory",
            ])
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(path);
                }
            }
            // User cancelled or zenity not available
            if output.status.code() == Some(1) {
                return None; // User cancelled
            }
        }

        // Try kdialog (KDE)
        if let Ok(output) = std::process::Command::new("kdialog")
            .args([
                "--getexistingdirectory",
                ".",
                "--title",
                "Select Installation Directory",
            ])
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(path);
                }
            }
        }

        warn!("No folder dialog available (install zenity or kdialog)");
        None
    }

    #[cfg(target_os = "macos")]
    {
        let script = r#"tell application "Finder" to activate
try
    set theFolder to choose folder with prompt "Select Installation Directory"
    return POSIX path of theFolder
on error
    return ""
end try"#;
        if let Ok(output) = std::process::Command::new("osascript")
            .args(["-e", script])
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(path);
                }
            }
        }
        None
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// Detect system theme (dark/light) on Linux/macOS.
fn detect_system_theme() -> String {
    // Check GTK theme on Linux
    #[cfg(target_os = "linux")]
    {
        if let Ok(theme) = std::env::var("GTK_THEME") {
            if theme.to_lowercase().contains("dark") {
                return "dark".to_string();
            }
        }
        // Check XDG settings
        if let Ok(output) = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
            .output()
        {
            let theme_name = String::from_utf8_lossy(&output.stdout).to_lowercase();
            if theme_name.contains("dark") {
                return "dark".to_string();
            }
        }
    }

    // Check macOS appearance
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = std::process::Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
        {
            let style = String::from_utf8_lossy(&output.stdout).to_lowercase();
            if style.contains("dark") {
                return "dark".to_string();
            }
        }
    }

    "light".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_shim_injection() {
        let html = "<script>\nchrome.webview.postMessage({type:'ready'});\nchrome.webview.addEventListener('message', fn);\n</script>";
        let result = inject_ipc_shim(html);
        assert!(result.contains("sendMsg("));
        assert!(result.contains("onMsg("));
        assert!(result.contains("window.ipc"));
        assert!(!result.contains("chrome.webview.postMessage"));
    }

    #[test]
    fn test_wizard_state_serialization() {
        let state = WizardState {
            page: "welcome".to_string(),
            app_name: "Test".to_string(),
            app_version: "1.0".to_string(),
            publisher: "Test".to_string(),
            install_dir: "/opt/test".to_string(),
            default_dir: "/opt/test".to_string(),
            license_text: "MIT".to_string(),
            components: vec![],
            selected_components: vec![],
            theme: "light".to_string(),
            progress_percent: 0,
            progress_file: String::new(),
            cancelled: false,
            launch_after: false,
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: WizardState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.app_name, "Test");
    }

    #[test]
    fn test_detect_theme_returns_valid() {
        let theme = detect_system_theme();
        assert!(theme == "light" || theme == "dark");
    }

    #[test]
    fn test_js_message_ready() {
        let json = r#"{"type":"ready"}"#;
        let msg: JsMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, JsMessage::Ready));
    }

    #[test]
    fn test_js_message_next_back() {
        let msg: JsMessage = serde_json::from_str(r#"{"type":"next"}"#).unwrap();
        assert!(matches!(msg, JsMessage::Next));
        let msg: JsMessage = serde_json::from_str(r#"{"type":"back"}"#).unwrap();
        assert!(matches!(msg, JsMessage::Back));
    }

    #[test]
    fn test_js_message_set_dir() {
        let json = r#"{"type":"set_dir","data":"/opt/myapp"}"#;
        let msg: JsMessage = serde_json::from_str(json).unwrap();
        match msg {
            JsMessage::SetDir(dir) => assert_eq!(dir, "/opt/myapp"),
            _ => panic!("Expected SetDir"),
        }
    }

    #[test]
    fn test_js_message_toggle_component() {
        let json = r#"{"type":"toggle_component","data":"core"}"#;
        let msg: JsMessage = serde_json::from_str(json).unwrap();
        match msg {
            JsMessage::ToggleComponent(id) => assert_eq!(id, "core"),
            _ => panic!("Expected ToggleComponent"),
        }
    }

    #[test]
    fn test_js_message_finish_with_launch() {
        let json = r#"{"type":"finish","data":{"launch":true}}"#;
        let msg: JsMessage = serde_json::from_str(json).unwrap();
        match msg {
            JsMessage::Finish { launch } => assert!(launch),
            _ => panic!("Expected Finish"),
        }
    }

    #[test]
    fn test_js_message_finish_no_launch() {
        let json = r#"{"type":"finish","data":{"launch":false}}"#;
        let msg: JsMessage = serde_json::from_str(json).unwrap();
        match msg {
            JsMessage::Finish { launch } => assert!(!launch),
            _ => panic!("Expected Finish"),
        }
    }

    #[test]
    fn test_js_message_invalid_json() {
        let result: std::result::Result<JsMessage, _> = serde_json::from_str("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_js_message_unknown_type() {
        let result: std::result::Result<JsMessage, _> =
            serde_json::from_str(r#"{"type":"unknown_action"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_ipc_next_navigation() {
        let state = Arc::new(Mutex::new(WizardState {
            page: "welcome".to_string(),
            app_name: "Test".into(),
            app_version: "1.0".into(),
            publisher: "Test".into(),
            install_dir: "/opt/test".into(),
            default_dir: "/opt/test".into(),
            license_text: "".into(),
            components: vec![],
            selected_components: vec![],
            theme: "light".into(),
            progress_percent: 0,
            progress_file: "".into(),
            cancelled: false,
            launch_after: false,
        }));
        let (browse_tx, _) = mpsc::channel();
        let (install_req_tx, _) = mpsc::channel();

        // Navigate forward
        handle_ipc_message(&state, r#"{"type":"next"}"#, &browse_tx, &install_req_tx);
        assert_eq!(state.lock().unwrap().page, "license");

        handle_ipc_message(&state, r#"{"type":"next"}"#, &browse_tx, &install_req_tx);
        assert_eq!(state.lock().unwrap().page, "directory");

        // Navigate back
        handle_ipc_message(&state, r#"{"type":"back"}"#, &browse_tx, &install_req_tx);
        assert_eq!(state.lock().unwrap().page, "license");
    }

    #[test]
    fn test_handle_ipc_toggle_component() {
        let state = Arc::new(Mutex::new(WizardState {
            page: "components".to_string(),
            app_name: "Test".into(),
            app_version: "1.0".into(),
            publisher: "Test".into(),
            install_dir: "/opt/test".into(),
            default_dir: "/opt/test".into(),
            license_text: "".into(),
            components: vec![
                ComponentItem {
                    id: "core".into(),
                    name: "Core".into(),
                    description: "Core files".into(),
                    size_mb: 10.0,
                    selected: true,
                    mandatory: true,
                },
                ComponentItem {
                    id: "docs".into(),
                    name: "Docs".into(),
                    description: "Documentation".into(),
                    size_mb: 5.0,
                    selected: false,
                    mandatory: false,
                },
            ],
            selected_components: vec!["core".into()],
            theme: "light".into(),
            progress_percent: 0,
            progress_file: "".into(),
            cancelled: false,
            launch_after: false,
        }));
        let (browse_tx, _) = mpsc::channel();
        let (install_req_tx, _) = mpsc::channel();

        // Toggle non-mandatory component
        handle_ipc_message(
            &state,
            r#"{"type":"toggle_component","data":"docs"}"#,
            &browse_tx,
            &install_req_tx,
        );
        let st = state.lock().unwrap();
        assert!(
            st.components
                .iter()
                .find(|c| c.id == "docs")
                .unwrap()
                .selected
        );
        assert!(st.selected_components.contains(&"docs".to_string()));

        // Cannot toggle mandatory component
        handle_ipc_message(
            &state,
            r#"{"type":"toggle_component","data":"core"}"#,
            &browse_tx,
            &install_req_tx,
        );
        let st = state.lock().unwrap();
        assert!(
            st.components
                .iter()
                .find(|c| c.id == "core")
                .unwrap()
                .selected
        );
    }

    #[test]
    fn test_handle_ipc_cancel() {
        let state = Arc::new(Mutex::new(WizardState {
            page: "welcome".to_string(),
            app_name: "Test".into(),
            app_version: "1.0".into(),
            publisher: "Test".into(),
            install_dir: "/opt/test".into(),
            default_dir: "/opt/test".into(),
            license_text: "".into(),
            components: vec![],
            selected_components: vec![],
            theme: "light".into(),
            progress_percent: 0,
            progress_file: "".into(),
            cancelled: false,
            launch_after: false,
        }));
        let (browse_tx, _) = mpsc::channel();
        let (install_req_tx, _) = mpsc::channel();

        handle_ipc_message(&state, r#"{"type":"cancel"}"#, &browse_tx, &install_req_tx);
        assert!(state.lock().unwrap().cancelled);
    }

    #[test]
    fn test_handle_ipc_set_dir() {
        let state = Arc::new(Mutex::new(WizardState {
            page: "directory".to_string(),
            app_name: "Test".into(),
            app_version: "1.0".into(),
            publisher: "Test".into(),
            install_dir: "/opt/test".into(),
            default_dir: "/opt/test".into(),
            license_text: "".into(),
            components: vec![],
            selected_components: vec![],
            theme: "light".into(),
            progress_percent: 0,
            progress_file: "".into(),
            cancelled: false,
            launch_after: false,
        }));
        let (browse_tx, _) = mpsc::channel();
        let (install_req_tx, _) = mpsc::channel();

        handle_ipc_message(
            &state,
            r#"{"type":"set_dir","data":"/home/user/myapp"}"#,
            &browse_tx,
            &install_req_tx,
        );
        assert_eq!(state.lock().unwrap().install_dir, "/home/user/myapp");
    }

    #[test]
    fn test_ipc_shim_preserves_non_script_content() {
        let html = "<div>Hello</div><script>\nvar x = 1;\n</script><p>World</p>";
        let result = inject_ipc_shim(html);
        assert!(result.contains("<div>Hello</div>"));
        assert!(result.contains("<p>World</p>"));
        assert!(result.contains("window.ipc"));
    }
}
