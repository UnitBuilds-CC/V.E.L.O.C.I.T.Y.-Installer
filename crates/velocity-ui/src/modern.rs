//! Modern WebView2-based installer wizard.
//!
//! Provides a contemporary wizard UI rendered via Microsoft Edge WebView2.
//! Features:
//! - Dark and light theme support (auto-detected from Windows settings)
//! - Smooth CSS transitions between pages
//! - JavaScript ↔ Rust bidirectional communication
//! - Embedded HTML/CSS (no external files needed)
//! - Responsive layout that adapts to window size

use crate::error::UiError;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tracing::{error, info, warn};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{
    CoTaskMemPWSTR, CreateCoreWebView2ControllerCompletedHandler,
    CreateCoreWebView2EnvironmentCompletedHandler, WebMessageReceivedEventHandler,
};
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::{self, HBRUSH};
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Result from the modern wizard dialog.
#[derive(Debug, Clone)]
pub struct ModernWizardResult {
    pub install_dir: std::path::PathBuf,
    pub cancelled: bool,
    pub launch_after: bool,
    pub selected_components: Vec<String>,
    pub install_completed: bool,
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

/// Messages sent from JavaScript to Rust.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[allow(dead_code)] // Used by WebView2 RPC handler
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

/// Run the modern WebView2 wizard.
///
/// Creates a Win32 window hosting a WebView2 control that renders the
/// installer wizard UI. Communicates with the JavaScript frontend via
/// JSON messages over the WebView2 RPC channel.
///
/// Returns `Err(UiError::WebView2NotAvailable)` if the WebView2 runtime
/// is not installed, so the caller can fall back to the classic wizard.
pub fn run_modern_wizard(
    app_name: &str,
    app_version: &str,
    publisher: &str,
    default_dir: &str,
    license_text: &str,
    components: &[(String, String, String, f64, bool, bool)], // (id, name, desc, size_mb, selected, mandatory)
) -> std::result::Result<ModernWizardResult, UiError> {
    // Check for WebView2 runtime before attempting to create any windows
    if !is_webview2_runtime_installed() {
        warn!("WebView2 runtime not detected on this system");
        return Err(UiError::WebView2NotAvailable);
    }

    info!(
        "Starting modern WebView2 wizard for {} v{}",
        app_name, app_version
    );

    // Detect system theme
    let theme = detect_system_theme();
    info!("System theme: {}", theme);

    // Build initial wizard state
    let state = Arc::new(Mutex::new(WizardState {
        page: "welcome".to_string(),
        app_name: app_name.to_string(),
        app_version: app_version.to_string(),
        publisher: publisher.to_string(),
        install_dir: default_dir.to_string(),
        default_dir: default_dir.to_string(),
        license_text: license_text.to_string(),
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
        theme: theme.clone(),
        progress_percent: 0,
        progress_file: String::new(),
        cancelled: false,
        launch_after: false,
    }));

    // This is a blocking call that runs the Win32 message loop
    let result = run_wizard_window(state)?;

    Ok(result)
}

/// Detect whether the system is using dark or light theme.
fn detect_system_theme() -> String {
    // Read from Windows registry: HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize
    // AppsUseLightTheme: 0 = dark, 1 = light
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    match RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")
    {
        Ok(key) => match key.get_value::<u32, _>("AppsUseLightTheme") {
            Ok(0) => "dark".to_string(),
            Ok(_) => "light".to_string(),
            Err(_) => "light".to_string(),
        },
        Err(_) => "light".to_string(),
    }
}

/// Check if the WebView2 Evergreen runtime is installed on this system.
///
/// Checks the registry for the WebView2 Evergreen Bootstrapper client key,
/// which is present when the runtime has been installed (either standalone
/// or via Evergreen distribution).
///
/// See: <https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution>
pub fn is_webview2_runtime_installed() -> bool {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    // WebView2 Evergreen runtime client key (same GUID for both HKLM and HKCU)
    let wv2_key = r"Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

    // Check HKLM first (machine-wide install), then HKCU (per-user install)
    for root in &[HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        if RegKey::predef(*root).open_subkey(wv2_key).is_ok() {
            info!("WebView2 runtime detected via registry ({:?})", root);
            return true;
        }
    }

    // Also check if the WebView2 loader DLL is available (fixed version runtime)
    // This covers the case where someone bundled the runtime alongside the app
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    if let Some(dir) = exe_dir {
        // Check for Microsoft.Web.WebView2.Core.dll in the app directory
        if dir.join("Microsoft.Web.WebView2.Core.dll").exists() {
            info!("WebView2 runtime detected via bundled DLL");
            return true;
        }
    }

    false
}

/// Shared state between the Win32 window procedure and WebView2 callbacks.
struct WebView2Shared {
    state: Arc<Mutex<WizardState>>,
    webview: Mutex<Option<ICoreWebView2>>,
    controller: Mutex<Option<ICoreWebView2Controller>>,
    #[allow(dead_code)]
    done: std::sync::atomic::AtomicBool,
    hwnd: Mutex<HWND>,
}

/// Custom window message sent when the wizard is complete.
const WM_WIZARD_COMPLETE: u32 = WM_APP + 1;

/// Run the Win32 window + WebView2 message loop.
///
/// Creates a real Win32 window hosting a WebView2 control. The WebView2
/// environment and controller are created asynchronously; once ready, the
/// wizard HTML is loaded and the JS↔Rust message channel is active.
#[allow(clippy::arc_with_non_send_sync)]
fn run_wizard_window(
    state: Arc<Mutex<WizardState>>,
) -> std::result::Result<ModernWizardResult, UiError> {
    // Safety: COM must be initialized for WebView2
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let shared = Arc::new(WebView2Shared {
        state,
        webview: Mutex::new(None),
        controller: Mutex::new(None),
        done: std::sync::atomic::AtomicBool::new(false),
        hwnd: Mutex::new(HWND::default()),
    });

    // Generate HTML content
    let theme = {
        let st = shared
            .state
            .lock()
            .map_err(|_| UiError::Wizard("Lock poisoned".into()))?;
        st.theme.clone()
    };
    let html = generate_wizard_html(&theme);

    // Create Win32 window
    unsafe {
        let hi = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("VelocityModernWizard");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(modern_wnd_proc),
            hInstance: hi.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH::default(),
            lpszClassName: class_name,
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);

        let shared_ptr = Arc::into_raw(shared.clone());
        let win_w = 800i32;
        let win_h = 600i32;

        let title_h = HSTRING::from(format!(
            "{} - Setup",
            shared
                .state
                .lock()
                .map_err(|_| UiError::Wizard("Lock".into()))?
                .app_name
        ));

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            &title_h,
            WS_OVERLAPPEDWINDOW & !(WS_THICKFRAME | WS_MAXIMIZEBOX),
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            win_w,
            win_h,
            None,
            None,
            Some(hi.into()),
            Some(shared_ptr as *mut _),
        );

        let hwnd = match hwnd {
            Ok(h) => h,
            Err(e) => {
                let _ = Arc::from_raw(shared_ptr);
                return Err(UiError::WindowCreation(format!("{}", e)));
            }
        };

        {
            let mut h = shared
                .hwnd
                .lock()
                .map_err(|_| UiError::Wizard("hwnd lock poisoned".into()))?;
            *h = hwnd;
        }

        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        let _ = SetWindowPos(
            hwnd,
            None,
            (sw - win_w) / 2,
            (sh - win_h) / 2,
            0,
            0,
            SWP_NOZORDER | SWP_NOSIZE,
        );

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = Gdi::UpdateWindow(hwnd);

        // Create WebView2 environment using wait_with_pump pattern
        let environment = {
            let (tx, rx) = std::sync::mpsc::channel();

            CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
                Box::new(|environmentcreatedhandler| {
                    CreateCoreWebView2Environment(&environmentcreatedhandler)
                        .map_err(webview2_com::Error::WindowsError)
                }),
                Box::new(move |error_code, environment| {
                    error_code?;
                    let _ =
                        tx.send(environment.ok_or_else(|| windows::core::Error::from(E_POINTER)));
                    Ok(())
                }),
            )
            .ok();

            rx.recv().ok()
        };

        let environment = match environment {
            Some(Ok(env)) => env,
            Some(Err(e)) => {
                error!("WebView2 environment creation failed: {}", e);
                // Fall through to message loop — window will show but without WebView2
                // The wizard will return default results
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).into() {
                    if msg.message == WM_WIZARD_COMPLETE {
                        break;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                let st = shared
                    .state
                    .lock()
                    .map_err(|_| UiError::Wizard("Lock".into()))?;
                return Ok(ModernWizardResult {
                    install_dir: std::path::PathBuf::from(&st.install_dir),
                    cancelled: st.cancelled,
                    launch_after: st.launch_after,
                    selected_components: st.selected_components.clone(),
                    install_completed: false,
                });
            }
            None => {
                error!("WebView2 environment channel closed");
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).into() {
                    if msg.message == WM_WIZARD_COMPLETE {
                        break;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                let st = shared
                    .state
                    .lock()
                    .map_err(|_| UiError::Wizard("Lock".into()))?;
                return Ok(ModernWizardResult {
                    install_dir: std::path::PathBuf::from(&st.install_dir),
                    cancelled: st.cancelled,
                    launch_after: st.launch_after,
                    selected_components: st.selected_components.clone(),
                    install_completed: false,
                });
            }
        };

        // Create WebView2 controller
        let controller = {
            let (tx, rx) = std::sync::mpsc::channel();

            CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
                Box::new(move |handler| {
                    environment
                        .CreateCoreWebView2Controller(hwnd, &handler)
                        .map_err(webview2_com::Error::WindowsError)
                }),
                Box::new(move |error_code, controller| {
                    error_code?;
                    let _ =
                        tx.send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)));
                    Ok(())
                }),
            )
            .ok();

            rx.recv().ok()
        };

        let controller = match controller {
            Some(Ok(c)) => c,
            _ => {
                error!("WebView2 controller creation failed");
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).into() {
                    if msg.message == WM_WIZARD_COMPLETE {
                        break;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                let st = shared
                    .state
                    .lock()
                    .map_err(|_| UiError::Wizard("Lock".into()))?;
                return Ok(ModernWizardResult {
                    install_dir: std::path::PathBuf::from(&st.install_dir),
                    cancelled: st.cancelled,
                    launch_after: st.launch_after,
                    selected_components: st.selected_components.clone(),
                    install_completed: false,
                });
            }
        };

        // Store controller
        {
            let mut ctrl = shared
                .controller
                .lock()
                .map_err(|_| UiError::Wizard("controller lock poisoned".into()))?;
            *ctrl = Some(controller.clone());
        }

        // Get the CoreWebView2
        let webview = controller.CoreWebView2();
        if let Ok(ref wv) = webview {
            // Store webview reference
            {
                let mut wv_store = shared
                    .webview
                    .lock()
                    .map_err(|_| UiError::Wizard("webview lock poisoned".into()))?;
                *wv_store = Some(wv.clone());
            }

            // Set up WebMessageReceived handler
            let shared_for_msg = shared.clone();
            let mut _token = 0;
            let _ = wv.add_WebMessageReceived(
                &WebMessageReceivedEventHandler::create(Box::new(move |_webview, args| {
                    if let Some(args) = args {
                        let mut message = PWSTR(std::ptr::null_mut());
                        if args.WebMessageAsJson(&mut message).is_ok() {
                            let msg_str = message.to_string().unwrap_or_default();
                            handle_js_message(&shared_for_msg, &msg_str);
                        }
                    }
                    Ok(())
                })),
                &mut _token,
            );

            // Navigate to the wizard HTML
            let html_pcwstr = CoTaskMemPWSTR::from(html.as_str());
            let _ = wv.Navigate(*html_pcwstr.as_ref().as_pcwstr());
        }

        // Make WebView2 fill the entire window
        let _ = controller.SetBounds(RECT {
            left: 0,
            top: 0,
            right: win_w,
            bottom: win_h,
        });
        let _ = controller.SetIsVisible(true);

        info!("WebView2 initialized and ready");

        // Run the Win32 message loop
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            if msg.message == WM_WIZARD_COMPLETE {
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // The shared Arc is still valid here — the window's Arc was recovered in WM_DESTROY.
    // Our local clone (shared) is still alive.

    let st = shared
        .state
        .lock()
        .map_err(|_| UiError::Wizard("Lock poisoned".into()))?;

    Ok(ModernWizardResult {
        install_dir: std::path::PathBuf::from(&st.install_dir),
        cancelled: st.cancelled,
        launch_after: st.launch_after,
        selected_components: st.selected_components.clone(),
        install_completed: false,
    })
}

/// Handle a JSON message received from JavaScript.
fn handle_js_message(shared: &Arc<WebView2Shared>, json: &str) {
    let msg: std::result::Result<JsMessage, _> = serde_json::from_str(json);
    match msg {
        Ok(JsMessage::Ready) => {
            // Send initial state to JS
            send_state_to_js(shared);
        }
        Ok(JsMessage::Next) => {
            let mut st = match shared.state.lock() {
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
            send_state_to_js(shared);
        }
        Ok(JsMessage::Back) => {
            let mut st = match shared.state.lock() {
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
            send_state_to_js(shared);
        }
        Ok(JsMessage::Install) => {
            let mut st = match shared.state.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            st.page = "progress".to_string();
            // Simulate installation progress (in real use, the runtime drives this)
            st.progress_percent = 100;
            st.progress_file = "Installation complete".to_string();
            st.install_dir = st.install_dir.clone();
            send_state_to_js(shared);
            // Navigate to finish page after a brief delay
            st.page = "finish".to_string();
            send_state_to_js(shared);
        }
        Ok(JsMessage::Cancel) => {
            let mut st = match shared.state.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            st.cancelled = true;
            close_wizard_window(shared);
        }
        Ok(JsMessage::Browse) => {
            // In a full implementation, this would open a folder browser dialog
            info!("Browse button clicked (folder browser not yet implemented in WebView2 wizard)");
        }
        Ok(JsMessage::SetDir(dir)) => {
            let mut st = match shared.state.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            st.install_dir = dir.clone();
            // Send updated dir back to JS
            if let Ok(wv) = shared.webview.lock() {
                if let Some(ref webview) = *wv {
                    let script = format!(
                        "chrome.webview.postMessage({{type:'set_dir',data:'{}'}})",
                        dir.replace('\\', "\\\\").replace('\'', "\\'")
                    );
                    unsafe {
                        let _ = webview.ExecuteScript(
                            *CoTaskMemPWSTR::from(script.as_str()).as_ref().as_pcwstr(),
                            None,
                        );
                    }
                }
            }
        }
        Ok(JsMessage::ToggleComponent(id)) => {
            let mut st = match shared.state.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            // Find the component and toggle its selection
            if let Some(comp) = st.components.iter_mut().find(|c| c.id == id) {
                if !comp.mandatory {
                    comp.selected = !comp.selected;
                }
            }
            // Update selected_components list
            st.selected_components = st
                .components
                .iter()
                .filter(|c| c.selected)
                .map(|c| c.id.clone())
                .collect();
            send_state_to_js(shared);
        }
        Ok(JsMessage::Finish { launch }) => {
            let mut st = match shared.state.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            st.launch_after = launch;
            close_wizard_window(shared);
        }
        Ok(JsMessage::GetState) => {
            send_state_to_js(shared);
        }
        Err(e) => {
            warn!("Failed to parse JS message: {} (json: {})", e, json);
        }
    }
}

/// Send the current wizard state to JavaScript via ExecuteScript.
fn send_state_to_js(shared: &Arc<WebView2Shared>) {
    let st = match shared.state.lock() {
        Ok(s) => s,
        Err(_) => return,
    };
    let state_json = match serde_json::to_string(&*st) {
        Ok(j) => j,
        Err(e) => {
            warn!("Failed to serialize wizard state: {}", e);
            return;
        }
    };
    let script = format!(
        "chrome.webview.postMessage({{type:'state',data:{}}})",
        state_json
    );
    if let Ok(wv) = shared.webview.lock() {
        if let Some(ref webview) = *wv {
            let script_pwstr = CoTaskMemPWSTR::from(script.as_str());
            unsafe {
                let _ = webview.ExecuteScript(*script_pwstr.as_ref().as_pcwstr(), None);
            }
        }
    }
}

/// Close the wizard window, ending the message loop.
fn close_wizard_window(shared: &Arc<WebView2Shared>) {
    if let Ok(hwnd) = shared.hwnd.lock() {
        if !hwnd.is_invalid() {
            unsafe {
                let _ = PostMessageW(Some(*hwnd), WM_WIZARD_COMPLETE, WPARAM(0), LPARAM(0));
            }
        }
    }
}

/// Win32 window procedure for the modern wizard window.
unsafe extern "system" fn modern_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            let shared_ptr = cs.lpCreateParams as *const WebView2Shared;
            if !shared_ptr.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, shared_ptr as isize);
            }
            LRESULT(0)
        }
        WM_SIZE => {
            // Resize WebView2 to fill the window
            let shared_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const WebView2Shared;
            if !shared_ptr.is_null() {
                let shared = &*shared_ptr;
                let cx = (lparam.0 & 0xFFFF) as i32;
                let cy = ((lparam.0 >> 16) & 0xFFFF) as i32;
                if let Ok(ctrl) = shared.controller.lock() {
                    if let Some(ref controller) = *ctrl {
                        let _ = controller.SetBounds(RECT {
                            left: 0,
                            top: 0,
                            right: cx,
                            bottom: cy,
                        });
                    }
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            // Recover the Arc we stored via into_raw
            let shared_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const WebView2Shared;
            if !shared_ptr.is_null() {
                let _ = Arc::from_raw(shared_ptr);
            }
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Generate the complete HTML content for the wizard.
///
/// Delegates to the cross-platform `wizard_html` module so the same
/// template is shared with the wry-based wizard on Linux/macOS.
pub fn generate_wizard_html(theme: &str) -> String {
    crate::wizard_html::generate_wizard_html(theme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_system_theme() {
        // Should not panic — returns "light" or "dark"
        let theme = detect_system_theme();
        assert!(theme == "light" || theme == "dark");
    }

    #[test]
    fn test_generate_wizard_html_light() {
        let html = generate_wizard_html("light");
        assert!(html.contains("data-theme=\"light\""));
        assert!(html.contains("Welcome"));
        assert!(html.contains("License Agreement"));
        assert!(html.contains("Installation Directory"));
        assert!(html.contains("Select Components"));
        assert!(html.contains("Installing"));
        assert!(html.contains("Installation Complete"));
    }

    #[test]
    fn test_generate_wizard_html_dark() {
        let html = generate_wizard_html("dark");
        assert!(html.contains("data-theme=\"dark\""));
        assert!(html.contains("--bg: #1e1e1e"));
    }

    #[test]
    fn test_wizard_state_serialization() {
        let state = WizardState {
            page: "welcome".to_string(),
            app_name: "Test App".to_string(),
            app_version: "1.0.0".to_string(),
            publisher: "Test".to_string(),
            install_dir: "C:\\Test".to_string(),
            default_dir: "C:\\Test".to_string(),
            license_text: "MIT License".to_string(),
            components: vec![ComponentItem {
                id: "core".to_string(),
                name: "Core".to_string(),
                description: "Core files".to_string(),
                size_mb: 10.5,
                selected: true,
                mandatory: true,
            }],
            selected_components: vec!["core".to_string()],
            theme: "light".to_string(),
            progress_percent: 0,
            progress_file: String::new(),
            cancelled: false,
            launch_after: false,
        };
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("Test App"));
        let deserialized: WizardState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.app_name, "Test App");
        assert_eq!(deserialized.components.len(), 1);
    }

    #[test]
    fn test_js_message_deserialization() {
        let msg: JsMessage = serde_json::from_str(r#"{"type":"ready"}"#).unwrap();
        assert!(matches!(msg, JsMessage::Ready));

        let msg: JsMessage = serde_json::from_str(r#"{"type":"cancel"}"#).unwrap();
        assert!(matches!(msg, JsMessage::Cancel));

        let msg: JsMessage =
            serde_json::from_str(r#"{"type":"set_dir","data":"C:\\MyApp"}"#).unwrap();
        match msg {
            JsMessage::SetDir(dir) => assert_eq!(dir, "C:\\MyApp"),
            _ => panic!("Expected SetDir"),
        }

        let msg: JsMessage =
            serde_json::from_str(r#"{"type":"finish","data":{"launch":true}}"#).unwrap();
        match msg {
            JsMessage::Finish { launch } => assert!(launch),
            _ => panic!("Expected Finish"),
        }
    }

    #[test]
    fn test_html_contains_theme_variables() {
        let html = generate_wizard_html("light");
        // Light theme variables
        assert!(html.contains("--bg: #ffffff"));
        assert!(html.contains("--accent: #0078d4"));
        // Dark theme variables
        assert!(html.contains("--bg: #1e1e1e"));
        assert!(html.contains("--accent: #4cc2ff"));
    }

    #[test]
    fn test_html_contains_javascript_rpc() {
        let html = generate_wizard_html("light");
        assert!(html.contains("chrome.webview.postMessage"));
        assert!(html.contains("chrome.webview.addEventListener"));
    }
}
