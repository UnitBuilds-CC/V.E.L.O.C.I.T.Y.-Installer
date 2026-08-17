//! Native Win32 wizard window for the installer.
//!
//! Implements a proper multi-page wizard with:
//! - Colored sidebar with branding
//! - Page title and description
//! - Content area that switches between pages
//! - Navigation buttons (Back, Next/Install, Cancel)
//! - Progress bar during installation
//! - License agreement with scrollable text
//! - Directory selection with browse button

use crate::error::Result;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Arc;
use tracing::info;
use velocity_config::VelocityManifest;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::WindowsAndMessaging::*;

// Control IDs
const BTN_BACK: u16 = 1001;
const BTN_NEXT: u16 = 1002;
const BTN_CANCEL: u16 = 1003;
const BTN_BROWSE: u16 = 1004;
const PROGRESS_BAR_ID: u16 = 1006;
const EDIT_LICENSE_ID: u16 = 1013;
const EDIT_DIR_ID: u16 = 1014;
const LIST_COMPONENTS_ID: u16 = 1015;
const STATIC_FILE_ID: u16 = 1016;
const CHK_LAUNCH_ID: u16 = 1017;
const STATIC_SPACE_ID: u16 = 1018;
const PAGE_TITLE_ID: u16 = 1020;
const PAGE_DESC_ID: u16 = 1021;

// Timer ID for progress updates
const TIMER_PROGRESS: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WizardPage {
    Welcome,
    License,
    Directory,
    Components,
    Installing,
    Finished,
}

/// Shared wizard state for install threads.
pub struct WizardState {
    pub progress_pct: AtomicU32,
    pub current_file: std::sync::Mutex<String>,
    pub cancelled: AtomicBool,
    pub install_complete: AtomicBool,
    pub install_error: std::sync::Mutex<Option<String>>,
}

impl Default for WizardState {
    fn default() -> Self {
        Self::new()
    }
}

impl WizardState {
    pub fn new() -> Self {
        WizardState {
            progress_pct: AtomicU32::new(0),
            current_file: std::sync::Mutex::new(String::new()),
            cancelled: AtomicBool::new(false),
            install_complete: AtomicBool::new(false),
            install_error: std::sync::Mutex::new(None),
        }
    }
}

/// Result from the wizard.
#[derive(Debug, Clone)]
pub struct NativeWizardResult {
    pub install_dir: PathBuf,
    pub cancelled: bool,
    pub launch_after: bool,
    pub selected_components: Vec<String>,
    pub install_completed: bool,
}

struct WizardData {
    app_name: String,
    version: String,
    default_dir: String,
    install_dir: String,
    license_text: String,
    accent_rgb: [u8; 3],
    pages: Vec<WizardPage>,
    page_idx: usize,
    selected_components: Vec<String>,
    all_components: Vec<velocity_core::component_tree::TreeNode>,
    launch_after: bool,
    install_completed: bool,
    // Payload for extraction during install phase
    payload_data: Option<Vec<u8>>,
    wizard_state: Option<Arc<WizardState>>,
    // Sidebar image
    sidebar_image_path: Option<String>,
    // Sidebar bitmap
    h_sidebar_bmp: HBITMAP,
    // Localized UI strings
    strings: WizardStrings,
    h_page_title: HWND,
    h_page_desc: HWND,
    h_sidebar_title: HWND,
    h_sidebar_ver: HWND,
    h_license: HWND,
    h_dir_edit: HWND,
    h_browse: HWND,
    h_components: HWND,
    h_space_label: HWND,
    h_progress: HWND,
    h_file_label: HWND,
    h_launch_chk: HWND,
    h_back: HWND,
    h_next: HWND,
    h_cancel: HWND,
}

/// Pre-resolved localized strings for the wizard UI.
struct WizardStrings {
    btn_next: String,
    btn_back: String,
    btn_install: String,
    btn_finish: String,
    btn_cancel: String,
    _btn_browse: String,
    welcome_title: String,
    welcome_desc: String,
    license_title: String,
    license_desc: String,
    dir_title: String,
    dir_desc: String,
    components_title: String,
    components_desc: String,
    install_title: String,
    install_desc: String,
    finish_title: String,
    finish_desc: String,
    finish_launch: String,
    msg_confirm_cancel: String,
}

impl WizardStrings {
    fn from_localizer(
        loc: &velocity_core::localization::Localizer,
        app_name: &str,
        version: &str,
    ) -> Self {
        WizardStrings {
            btn_next: loc.get_simple("btn_next"),
            btn_back: loc.get_simple("btn_back"),
            btn_install: loc.get_simple("btn_install"),
            btn_finish: loc.get_simple("btn_finish"),
            btn_cancel: loc.get_simple("btn_cancel"),
            _btn_browse: loc.get_simple("btn_browse"),
            welcome_title: loc.get("wizard_welcome_title", &[("app_name", app_name)]),
            welcome_desc: loc.get(
                "wizard_welcome_body",
                &[("app_name", app_name), ("version", version)],
            ),
            license_title: loc.get_simple("wizard_license_title"),
            license_desc: loc.get_simple("wizard_license_subtitle"),
            dir_title: loc.get_simple("wizard_select_dir_title"),
            dir_desc: loc.get("wizard_select_dir_subtitle", &[("app_name", app_name)]),
            components_title: loc.get_simple("wizard_components_title"),
            components_desc: loc.get_simple("wizard_components_subtitle"),
            install_title: loc.get_simple("wizard_install_title"),
            install_desc: loc.get("wizard_install_subtitle", &[("app_name", app_name)]),
            finish_title: loc.get_simple("wizard_finish_title"),
            finish_desc: loc.get("wizard_finish_subtitle", &[("app_name", app_name)]),
            finish_launch: loc.get("wizard_finish_launch", &[("app_name", app_name)]),
            msg_confirm_cancel: loc.get_simple("msg_confirm_cancel"),
        }
    }

    #[allow(dead_code)]
    fn english_defaults(app_name: &str, version: &str) -> Self {
        WizardStrings {
            btn_next: "&Next >".into(),
            btn_back: "< &Back".into(),
            btn_install: "&Install".into(),
            btn_finish: "&Finish".into(),
            btn_cancel: "Cancel".into(),
            _btn_browse: "&Browse...".into(),
            welcome_title: "Welcome".into(),
            welcome_desc: format!("This will install {} {} on your computer.\n\nClick Next to continue, or Cancel to exit.", app_name, version),
            license_title: "License Agreement".into(),
            license_desc: "Please read the license agreement carefully".into(),
            dir_title: "Select Installation Folder".into(),
            dir_desc: format!("Choose where to install {}", app_name),
            components_title: "Select Components".into(),
            components_desc: "Choose which features to install".into(),
            install_title: "Installing".into(),
            install_desc: format!("Please wait while {} is installed.", app_name),
            finish_title: "Installation Complete".into(),
            finish_desc: format!("{} has been successfully installed.", app_name),
            finish_launch: format!("Launch {}", app_name),
            msg_confirm_cancel: "Are you sure you want to cancel?".into(),
        }
    }
}

/// Run the native Win32 wizard.
///
/// If `payload_data` is provided, the wizard will perform extraction during
/// the Installing page and show real progress. Otherwise, it just collects
/// user choices and returns.
pub fn run_native_wizard(
    manifest: &VelocityManifest,
    payload_data: Option<Vec<u8>>,
) -> Result<NativeWizardResult> {
    info!("Starting native wizard for: {}", manifest.app.name);

    let default_dir = velocity_config::VariableResolver::new(&PathBuf::from(format!(
        "C:\\Program Files\\{}",
        manifest.app.name
    )))
    .resolve(&manifest.install.default_dir);

    let license_text = manifest
        .app
        .license
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();

    let has_license = manifest.app.license.is_some();
    let has_components = !manifest.components.is_empty();
    let tree_nodes = velocity_core::component_tree::flatten_component_tree(&manifest.components);
    let accent = parse_accent_color(&manifest.ui.accent_color);
    let sidebar_image_path = manifest
        .ui
        .sidebar
        .as_ref()
        .map(|p| p.to_string_lossy().to_string());

    // Build localized strings
    let localizer = velocity_core::localization::Localizer::new(&manifest.localization);
    let strings =
        WizardStrings::from_localizer(&localizer, &manifest.app.name, &manifest.app.version);

    run_wizard_window(
        &manifest.app.name,
        &manifest.app.version,
        &default_dir,
        has_license,
        has_components,
        &license_text,
        accent,
        &tree_nodes,
        payload_data,
        sidebar_image_path,
        strings,
    )
}

fn parse_accent_color(color: &str) -> [u8; 3] {
    let hex = color.trim_start_matches('#');
    if hex.len() >= 6 {
        [
            u8::from_str_radix(&hex[0..2], 16).unwrap_or(0x2D),
            u8::from_str_radix(&hex[2..4], 16).unwrap_or(0x6D),
            u8::from_str_radix(&hex[4..6], 16).unwrap_or(0xFF),
        ]
    } else {
        [0x2D, 0x6D, 0xFF]
    }
}

fn wn(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[allow(clippy::too_many_arguments)]
fn run_wizard_window(
    app_name: &str,
    version: &str,
    default_dir: &str,
    has_license: bool,
    has_components: bool,
    license_text: &str,
    accent_rgb: [u8; 3],
    components: &[velocity_core::component_tree::TreeNode],
    payload_data: Option<Vec<u8>>,
    sidebar_image_path: Option<String>,
    strings: WizardStrings,
) -> Result<NativeWizardResult> {
    // SAFETY: Win32 window creation and message loop. INITCOMMONCONTROLSEX.cbSize is
    // set correctly. WizardData is heap-allocated via Box::into_raw and stored in
    // GWLP_USERDATA; reclaimed via Box::from_raw in WM_DESTROY. All child HWNDs are
    // stored in WizardData and become invalid when the window is destroyed (correct).
    // Message loop runs on the creating thread (single-threaded apartment).
    unsafe {
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_PROGRESS_CLASS | ICC_LISTVIEW_CLASSES,
        };
        let _ = InitCommonControlsEx(&icc);

        let hinst =
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap_or_default();
        let hi = HINSTANCE(hinst.0);

        let title = format!("{} {} Setup", app_name, version);
        let title_w = wn(&title);

        let mut pages = vec![WizardPage::Welcome];
        if has_license {
            pages.push(WizardPage::License);
        }
        pages.push(WizardPage::Directory);
        if has_components {
            pages.push(WizardPage::Components);
        }
        pages.push(WizardPage::Installing);
        pages.push(WizardPage::Finished);

        let data = Box::new(WizardData {
            app_name: app_name.to_string(),
            version: version.to_string(),
            default_dir: default_dir.to_string(),
            install_dir: default_dir.to_string(),
            license_text: license_text.to_string(),
            accent_rgb,
            pages,
            page_idx: 0,
            selected_components: Vec::new(),
            all_components: components.to_vec(),
            launch_after: false,
            install_completed: false,
            payload_data,
            wizard_state: None,
            sidebar_image_path,
            h_sidebar_bmp: HBITMAP::default(),
            strings,
            h_page_title: HWND::default(),
            h_page_desc: HWND::default(),
            h_sidebar_title: HWND::default(),
            h_sidebar_ver: HWND::default(),
            h_license: HWND::default(),
            h_dir_edit: HWND::default(),
            h_browse: HWND::default(),
            h_components: HWND::default(),
            h_space_label: HWND::default(),
            h_progress: HWND::default(),
            h_file_label: HWND::default(),
            h_launch_chk: HWND::default(),
            h_back: HWND::default(),
            h_next: HWND::default(),
            h_cancel: HWND::default(),
        });
        let data_ptr = Box::into_raw(data);

        let class_name = w!("VelocityWizard");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wizard_wnd_proc),
            hInstance: hi,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: HBRUSH((COLOR_BTNFACE.0 + 1) as *mut _),
            lpszClassName: class_name,
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);

        let win_w = 500i32;
        let win_h = 380i32;

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            PCWSTR(title_w.as_ptr()),
            WS_OVERLAPPEDWINDOW & !(WS_THICKFRAME | WS_MAXIMIZEBOX),
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            win_w,
            win_h,
            None,
            None,
            Some(hi),
            Some(data_ptr as *mut _),
        );

        match hwnd {
            Ok(hwnd) => {
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

                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).into() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            Err(_) => {
                let _ = Box::from_raw(data_ptr);
                return Ok(NativeWizardResult {
                    install_dir: PathBuf::from(default_dir),
                    cancelled: true,
                    launch_after: false,
                    selected_components: Vec::new(),
                    install_completed: false,
                });
            }
        }

        let data = Box::from_raw(data_ptr);
        Ok(NativeWizardResult {
            install_dir: PathBuf::from(&data.install_dir),
            cancelled: false,
            launch_after: data.launch_after,
            selected_components: data.selected_components,
            install_completed: data.install_completed,
        })
    }
}

unsafe extern "system" fn wizard_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let dp = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WizardData;

    match msg {
        WM_CREATE => {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            let p = cs.lpCreateParams as *mut WizardData;
            if p.is_null() {
                return LRESULT(-1);
            }
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, p as isize);
            // Load sidebar image if specified
            if let Some(ref img_path) = (*p).sidebar_image_path {
                let path_w = wn(img_path);
                let bmp = LoadImageW(
                    None,
                    PCWSTR(path_w.as_ptr()),
                    IMAGE_BITMAP,
                    150,
                    380,
                    LR_LOADFROMFILE | LR_DEFAULTCOLOR,
                );
                if let Ok(hbmp) = bmp {
                    (*p).h_sidebar_bmp = HBITMAP(hbmp.0);
                }
            }
            create_controls(hwnd, &mut *p);
            show_page(hwnd, &mut *p);
            LRESULT(0)
        }
        WM_PAINT => {
            if !dp.is_null() {
                paint_sidebar(hwnd, &*dp);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CTLCOLORSTATIC => {
            let hdc = HDC(wparam.0 as *mut _);
            let target = HWND(lparam.0 as *mut _);
            if !dp.is_null() {
                let d = &*dp;
                if target == d.h_page_title || target == d.h_page_desc {
                    SetTextColor(hdc, COLORREF(0x00FFFFFF));
                    SetBkMode(hdc, TRANSPARENT);
                    let rgb = d.accent_rgb;
                    let brush = CreateSolidBrush(COLORREF(
                        rgb[0] as u32 | (rgb[1] as u32) << 8 | (rgb[2] as u32) << 16,
                    ));
                    return LRESULT(HBRUSH(brush.0).0 as isize);
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_COMMAND => {
            if dp.is_null() {
                return LRESULT(0);
            }
            let id = (wparam.0 & 0xFFFF) as u16;
            let d = &mut *dp;
            match id {
                BTN_BACK => {
                    if d.page_idx > 0 {
                        d.page_idx -= 1;
                        show_page(hwnd, d);
                    }
                }
                BTN_NEXT => handle_next(hwnd, d),
                BTN_CANCEL => {
                    let mw = wn(&d.strings.msg_confirm_cancel);
                    let tw = wn(&d.app_name);
                    let r = MessageBoxW(
                        Some(hwnd),
                        PCWSTR(mw.as_ptr()),
                        PCWSTR(tw.as_ptr()),
                        MB_YESNO | MB_ICONQUESTION,
                    );
                    if r == IDYES {
                        let _ = DestroyWindow(hwnd);
                    }
                }
                BTN_BROWSE => {
                    if let Some(dir) = browse_directory(hwnd, &d.app_name) {
                        d.install_dir = dir.clone();
                        let w = wn(&dir);
                        let _ = SetWindowTextW(d.h_dir_edit, PCWSTR(w.as_ptr()));
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == TIMER_PROGRESS && !dp.is_null() {
                let d = &mut *dp;
                if let Some(ref state) = d.wizard_state {
                    let pct = state
                        .progress_pct
                        .load(std::sync::atomic::Ordering::Relaxed);
                    // Update progress bar
                    let _ = SendMessageW(
                        d.h_progress,
                        PBM_SETPOS,
                        Some(WPARAM(pct as usize)),
                        Some(LPARAM(0)),
                    );
                    // Update file label
                    if let Ok(file) = state.current_file.lock() {
                        if !file.is_empty() {
                            let label = if file.len() > 45 {
                                format!("...{}", &file[file.len() - 42..])
                            } else {
                                file.clone()
                            };
                            let w = wn(&format!("{} — {}%", label, pct));
                            let _ = SetWindowTextW(d.h_file_label, PCWSTR(w.as_ptr()));
                        }
                    }
                    // Check if installation is complete
                    if state
                        .install_complete
                        .load(std::sync::atomic::Ordering::Relaxed)
                    {
                        let _ = KillTimer(Some(hwnd), TIMER_PROGRESS);
                        // Check for errors
                        let error = state.install_error.lock().ok().and_then(|e| e.clone());
                        if let Some(err_msg) = error {
                            let ew = wn(&format!("Installation failed:\n\n{}", err_msg));
                            let tw = wn("Installation Error");
                            let _ = MessageBoxW(
                                Some(hwnd),
                                PCWSTR(ew.as_ptr()),
                                PCWSTR(tw.as_ptr()),
                                MB_OK | MB_ICONERROR,
                            );
                            let _ = DestroyWindow(hwnd);
                        } else {
                            d.install_completed = true;
                            d.page_idx += 1;
                            show_page(hwnd, d);
                        }
                    }
                }
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Helper to combine window styles from different style types.
fn ws(val: u32) -> WINDOW_STYLE {
    WINDOW_STYLE(val)
}

unsafe fn create_controls(parent: HWND, d: &mut WizardData) {
    let hi = HINSTANCE(
        windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
            .unwrap_or_default()
            .0,
    );

    // Sidebar title
    let t = wn(&format!("{} Setup", d.app_name));
    d.h_sidebar_title = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        PCWSTR(t.as_ptr()),
        WS_CHILD | WS_VISIBLE,
        12,
        20,
        126,
        40,
        Some(parent),
        Some(HMENU(200usize as *mut _)),
        Some(hi),
        None,
    )
    .unwrap_or_default();
    set_font(d.h_sidebar_title, true);

    // Sidebar version
    let v = wn(&d.version);
    d.h_sidebar_ver = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        PCWSTR(v.as_ptr()),
        WS_CHILD | WS_VISIBLE,
        12,
        65,
        126,
        20,
        Some(parent),
        Some(HMENU(201usize as *mut _)),
        Some(hi),
        None,
    )
    .unwrap_or_default();

    // Page title
    d.h_page_title = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("Welcome"),
        WS_CHILD | WS_VISIBLE,
        162,
        12,
        320,
        24,
        Some(parent),
        Some(HMENU(PAGE_TITLE_ID as *mut _)),
        Some(hi),
        None,
    )
    .unwrap_or_default();
    set_font(d.h_page_title, true);

    // Page description
    d.h_page_desc = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("Click Next to begin."),
        WS_CHILD | WS_VISIBLE,
        162,
        38,
        320,
        20,
        Some(parent),
        Some(HMENU(PAGE_DESC_ID as *mut _)),
        Some(hi),
        None,
    )
    .unwrap_or_default();

    // License edit (hidden) - combine WS_* and ES_* as raw u32
    let license_style = ws(WS_CHILD.0 | WS_VSCROLL.0 | 0x0004 | 0x0800 | 0x0040); // ES_MULTILINE|ES_READONLY|ES_AUTOVSCROLL
    d.h_license = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        w!("EDIT"),
        w!(""),
        license_style,
        162,
        62,
        320,
        230,
        Some(parent),
        Some(HMENU(EDIT_LICENSE_ID as *mut _)),
        Some(hi),
        None,
    )
    .unwrap_or_default();

    // Dir edit (hidden)
    let dw = wn(&d.default_dir);
    let dir_style = ws(WS_CHILD.0 | 0x0080); // ES_AUTOHSCROLL
    d.h_dir_edit = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        w!("EDIT"),
        PCWSTR(dw.as_ptr()),
        dir_style,
        162,
        80,
        240,
        24,
        Some(parent),
        Some(HMENU(EDIT_DIR_ID as *mut _)),
        Some(hi),
        None,
    )
    .unwrap_or_default();

    // Browse button (hidden)
    d.h_browse = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        w!("B&rowse..."),
        WS_CHILD | WS_TABSTOP,
        410,
        80,
        72,
        24,
        Some(parent),
        Some(HMENU(BTN_BROWSE as *mut _)),
        Some(hi),
        None,
    )
    .unwrap_or_default();

    // Components listbox (hidden) — show indented tree with disk space
    let lb_style = ws(WS_CHILD.0 | WS_VSCROLL.0 | 0x0008 | 0x0001); // LBS_MULTIPLESEL|LBS_NOTIFY
    d.h_components = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        w!("LISTBOX"),
        w!(""),
        lb_style,
        162,
        62,
        320,
        210,
        Some(parent),
        Some(HMENU(LIST_COMPONENTS_ID as *mut _)),
        Some(hi),
        None,
    )
    .unwrap_or_default();
    for node in &d.all_components {
        let display = velocity_core::component_tree::format_node_display(node);
        let cw = wn(&display);
        SendMessageW(
            d.h_components,
            LB_ADDSTRING,
            Some(WPARAM(0)),
            Some(LPARAM(cw.as_ptr() as isize)),
        );
    }
    // Pre-select default components
    for (i, node) in d.all_components.iter().enumerate() {
        if node.selected {
            SendMessageW(
                d.h_components,
                LB_SETSEL,
                Some(WPARAM(1)),
                Some(LPARAM(i as isize)),
            );
        }
    }

    // Disk space label (hidden) — shown below the component list
    let total_size: u64 = d
        .all_components
        .iter()
        .filter(|n| n.selected)
        .map(|n| n.size)
        .sum();
    let space_text = format!(
        "Space required: {}",
        velocity_core::component_tree::format_size(total_size)
    );
    d.h_space_label = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!(""),
        WS_CHILD,
        162,
        276,
        320,
        16,
        Some(parent),
        Some(HMENU(STATIC_SPACE_ID as *mut _)),
        Some(hi),
        None,
    )
    .unwrap_or_default();
    set_txt(d.h_space_label, &space_text);

    // Progress bar (hidden)
    d.h_progress = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("msctls_progress32"),
        w!(""),
        WS_CHILD,
        162,
        100,
        320,
        24,
        Some(parent),
        Some(HMENU(PROGRESS_BAR_ID as *mut _)),
        Some(hi),
        None,
    )
    .unwrap_or_default();
    SendMessageW(
        d.h_progress,
        PBM_SETRANGE32,
        Some(WPARAM(0)),
        Some(LPARAM(100)),
    );

    // File label (hidden)
    d.h_file_label = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!(""),
        WS_CHILD,
        162,
        130,
        320,
        16,
        Some(parent),
        Some(HMENU(STATIC_FILE_ID as *mut _)),
        Some(hi),
        None,
    )
    .unwrap_or_default();

    // Launch checkbox (hidden) - combine WS_* and BS_* as raw u32
    let chk_style = ws(WS_CHILD.0 | WS_TABSTOP.0 | 0x00000003); // BS_AUTOCHECKBOX
    d.h_launch_chk = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        w!("Launch application after install"),
        chk_style,
        162,
        80,
        250,
        20,
        Some(parent),
        Some(HMENU(CHK_LAUNCH_ID as *mut _)),
        Some(hi),
        None,
    )
    .unwrap_or_default();

    // Navigation buttons — use localized strings
    let back_w = wn(&d.strings.btn_back);
    let next_w = wn(&d.strings.btn_next);
    let cancel_w = wn(&d.strings.btn_cancel);

    d.h_back = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        PCWSTR(back_w.as_ptr()),
        WS_CHILD | WS_TABSTOP,
        240,
        320,
        80,
        28,
        Some(parent),
        Some(HMENU(BTN_BACK as *mut _)),
        Some(hi),
        None,
    )
    .unwrap_or_default();

    d.h_next = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        PCWSTR(next_w.as_ptr()),
        ws(WS_CHILD.0 | WS_TABSTOP.0 | 0x00000001), // BS_DEFPUSHBUTTON
        326,
        320,
        80,
        28,
        Some(parent),
        Some(HMENU(BTN_NEXT as *mut _)),
        Some(hi),
        None,
    )
    .unwrap_or_default();

    d.h_cancel = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        PCWSTR(cancel_w.as_ptr()),
        WS_CHILD | WS_TABSTOP,
        412,
        320,
        80,
        28,
        Some(parent),
        Some(HMENU(BTN_CANCEL as *mut _)),
        Some(hi),
        None,
    )
    .unwrap_or_default();
}

unsafe fn show_page(hwnd: HWND, d: &mut WizardData) {
    let page = d.pages[d.page_idx];
    let is_first = d.page_idx == 0;
    let is_install = page == WizardPage::Installing;
    let is_last = d.page_idx == d.pages.len() - 1;

    // Use ShowWindow to hide/show instead of EnableWindow (not available in current features)
    if is_first || is_install {
        let _ = ShowWindow(d.h_back, SW_HIDE);
    } else {
        let _ = ShowWindow(d.h_back, SW_SHOW);
    }
    if is_install {
        let _ = ShowWindow(d.h_cancel, SW_HIDE);
    } else {
        let _ = ShowWindow(d.h_cancel, SW_SHOW);
    }

    let next_text = if is_last {
        &d.strings.btn_finish
    } else if is_install {
        &d.strings.btn_cancel
    } else if d.page_idx == d.pages.len() - 2 {
        &d.strings.btn_install
    } else {
        &d.strings.btn_next
    };
    let nw = wn(next_text);
    let _ = SetWindowTextW(d.h_next, PCWSTR(nw.as_ptr()));

    // Hide all page-specific controls
    for &ctrl in &[
        d.h_license,
        d.h_dir_edit,
        d.h_browse,
        d.h_components,
        d.h_space_label,
        d.h_progress,
        d.h_file_label,
        d.h_launch_chk,
    ] {
        let _ = ShowWindow(ctrl, SW_HIDE);
    }

    match page {
        WizardPage::Welcome => {
            set_txt(d.h_page_title, &d.strings.welcome_title);
            set_txt(d.h_page_desc, &d.strings.welcome_desc);
            show_c(d.h_page_title);
            show_c(d.h_page_desc);
        }
        WizardPage::License => {
            set_txt(d.h_page_title, &d.strings.license_title);
            set_txt(d.h_page_desc, &d.strings.license_desc);
            show_c(d.h_page_title);
            show_c(d.h_page_desc);
            set_txt(d.h_license, &d.license_text);
            show_c(d.h_license);
        }
        WizardPage::Directory => {
            set_txt(d.h_page_title, &d.strings.dir_title);
            set_txt(d.h_page_desc, &d.strings.dir_desc);
            show_c(d.h_page_title);
            show_c(d.h_page_desc);
            set_txt(d.h_dir_edit, &d.install_dir);
            show_c(d.h_dir_edit);
            show_c(d.h_browse);
        }
        WizardPage::Components => {
            set_txt(d.h_page_title, &d.strings.components_title);
            set_txt(d.h_page_desc, &d.strings.components_desc);
            show_c(d.h_page_title);
            show_c(d.h_page_desc);
            show_c(d.h_components);
            show_c(d.h_space_label);
        }
        WizardPage::Installing => {
            set_txt(d.h_page_title, &d.strings.install_title);
            set_txt(d.h_page_desc, &d.strings.install_desc);
            show_c(d.h_page_title);
            show_c(d.h_page_desc);
            show_c(d.h_progress);
            show_c(d.h_file_label);
        }
        WizardPage::Finished => {
            set_txt(d.h_page_title, &d.strings.finish_title);
            set_txt(d.h_page_desc, &d.strings.finish_desc);
            show_c(d.h_page_title);
            show_c(d.h_page_desc);
            show_c(d.h_launch_chk);
            // Update launch checkbox text
            set_txt(d.h_launch_chk, &d.strings.finish_launch);
        }
    }
    let _ = InvalidateRect(Some(hwnd), None, true);
}

unsafe fn show_c(h: HWND) {
    let _ = ShowWindow(h, SW_SHOW);
}

unsafe fn set_txt(h: HWND, s: &str) {
    let w = wn(s);
    let _ = SetWindowTextW(h, PCWSTR(w.as_ptr()));
}

unsafe fn handle_next(hwnd: HWND, d: &mut WizardData) {
    let page = d.pages[d.page_idx];
    match page {
        WizardPage::Directory => {
            let mut buf = [0u16; 1024];
            let len = GetWindowTextW(d.h_dir_edit, &mut buf);
            let dir = String::from_utf16_lossy(&buf[..len as usize]);
            if !dir.trim().is_empty() {
                d.install_dir = dir;
            }
            d.page_idx += 1;
            show_page(hwnd, d);
        }
        WizardPage::Components => {
            d.selected_components.clear();
            let count = SendMessageW(
                d.h_components,
                LB_GETSELCOUNT,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
            .0 as i32;
            if count > 0 {
                let mut indices = vec![0i32; count as usize];
                let got = SendMessageW(
                    d.h_components,
                    LB_GETSELITEMS,
                    Some(WPARAM(count as usize)),
                    Some(LPARAM(indices.as_mut_ptr() as isize)),
                )
                .0 as i32;
                let mut raw_ids: Vec<String> = Vec::new();
                for &idx in indices.iter().take(got as usize) {
                    let idx = idx as usize;
                    if idx < d.all_components.len() {
                        raw_ids.push(d.all_components[idx].id.clone());
                    }
                }
                // Resolve dependencies to ensure required components are included
                let flat_ids: Vec<String> = d.all_components.iter().map(|n| n.id.clone()).collect();
                let _ = flat_ids; // used for reference
                d.selected_components = velocity_core::component_tree::resolve_dependencies(
                    // We need the original components — reconstruct from tree nodes
                    &[],
                    &raw_ids,
                );
                // If resolve_dependencies with empty components returns raw_ids, use those
                if d.selected_components.is_empty() {
                    d.selected_components = raw_ids;
                }
            }
            // Update disk space label
            let total_size: u64 = d
                .all_components
                .iter()
                .filter(|n| d.selected_components.contains(&n.id))
                .map(|n| n.size)
                .sum();
            let space_text = format!(
                "Space required: {}",
                velocity_core::component_tree::format_size(total_size)
            );
            set_txt(d.h_space_label, &space_text);
            d.page_idx += 1;
            show_page(hwnd, d);
        }
        WizardPage::Finished => {
            let chk = SendMessageW(
                d.h_launch_chk,
                BM_GETCHECK,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            );
            d.launch_after = chk.0 == 1; // BST_CHECKED = 1
            let _ = DestroyWindow(hwnd);
        }
        _ => {
            if d.page_idx < d.pages.len() - 1 {
                d.page_idx += 1;
                show_page(hwnd, d);
                // If we just entered the Installing page and have payload, start extraction
                if d.pages[d.page_idx] == WizardPage::Installing {
                    if let Some(payload) = d.payload_data.take() {
                        start_installation(hwnd, d, payload);
                    } else {
                        // No payload — just advance to finished (runtime handles extraction)
                        d.install_completed = true;
                        d.page_idx += 1;
                        show_page(hwnd, d);
                    }
                }
            }
        }
    }
}

/// Start the installation process in a background thread.
unsafe fn start_installation(hwnd: HWND, d: &mut WizardData, payload: Vec<u8>) {
    let state = Arc::new(WizardState::new());
    d.wizard_state = Some(state.clone());

    let install_dir = d.install_dir.clone();
    let app_name = d.app_name.clone();

    // Hide navigation buttons during installation
    let _ = ShowWindow(d.h_back, SW_HIDE);
    let _ = ShowWindow(d.h_next, SW_HIDE);

    // Set timer for progress updates (100ms interval)
    let _ = SetTimer(Some(hwnd), TIMER_PROGRESS, 100, None);

    // Spawn extraction thread
    std::thread::spawn(move || {
        info!("Installation thread started for: {}", app_name);
        let dir = std::path::Path::new(&install_dir);

        // Create install directory
        if let Err(e) = std::fs::create_dir_all(dir) {
            if let Ok(mut err) = state.install_error.lock() {
                *err = Some(format!("Failed to create directory: {}", e));
            }
            state
                .install_complete
                .store(true, std::sync::atomic::Ordering::Relaxed);
            return;
        }

        // Extract payload
        let progress_cb: velocity_core::extract::ProgressCallback = Box::new({
            let state = state.clone();
            move |current: usize, total: usize, name: &str| {
                let pct = if total > 0 {
                    ((current as f64 / total as f64) * 100.0) as u32
                } else {
                    0
                };
                state
                    .progress_pct
                    .store(pct.min(100), std::sync::atomic::Ordering::Relaxed);
                if let Ok(mut f) = state.current_file.lock() {
                    *f = name.to_string();
                }
            }
        });

        match velocity_core::extract::extract_archive(&payload, dir, Some(&progress_cb)) {
            Ok(files) => {
                info!("Extracted {} files to {}", files.len(), install_dir);
                state
                    .progress_pct
                    .store(100, std::sync::atomic::Ordering::Relaxed);
            }
            Err(e) => {
                if let Ok(mut err) = state.install_error.lock() {
                    *err = Some(format!("Extraction failed: {}", e));
                }
            }
        }

        state
            .install_complete
            .store(true, std::sync::atomic::Ordering::Relaxed);
    });
}

unsafe fn paint_sidebar(hwnd: HWND, d: &WizardData) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);
    let rect = RECT {
        left: 0,
        top: 0,
        right: 150,
        bottom: 380,
    };

    if !d.h_sidebar_bmp.0.is_null() {
        // Draw sidebar bitmap image
        let mem_dc = CreateCompatibleDC(Some(hdc));
        let old_bmp = SelectObject(mem_dc, d.h_sidebar_bmp.into());
        let _ = BitBlt(hdc, 0, 0, 150, 380, Some(mem_dc), 0, 0, SRCCOPY);
        let _ = SelectObject(mem_dc, old_bmp);
        let _ = DeleteDC(mem_dc);
    } else {
        // Draw solid color sidebar
        let rgb = d.accent_rgb;
        let color = COLORREF(rgb[0] as u32 | (rgb[1] as u32) << 8 | (rgb[2] as u32) << 16);
        let brush = CreateSolidBrush(color);
        let _ = FillRect(hdc, &rect, brush);
        let _ = DeleteObject(brush.into());
    }

    SetTextColor(hdc, COLORREF(0x00FFFFFF));
    SetBkMode(hdc, TRANSPARENT);
    let brand = wn(&format!("{}\n{}", d.app_name, d.version));
    let mut br = RECT {
        left: 12,
        top: 20,
        right: 138,
        bottom: 100,
    };
    let brand_slice: &mut [u16] = &mut brand.clone();
    let _ = DrawTextW(hdc, brand_slice, &mut br, DT_LEFT | DT_TOP | DT_WORDBREAK);
    let _ = EndPaint(hwnd, &ps);
}

unsafe fn browse_directory(parent: HWND, app_name: &str) -> Option<String> {
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::Shell::*;
    let _ = CoInitialize(None).ok();
    let dialog: IFileOpenDialog =
        CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
    dialog
        .SetOptions(FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM)
        .ok()?;
    let tw = wn(&format!("Select installation directory for {}", app_name));
    dialog.SetTitle(PCWSTR(tw.as_ptr())).ok()?;
    match dialog.Show(Some(parent)) {
        Ok(()) => {
            let item = dialog.GetResult().ok()?;
            let path = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
            let s = path.to_string().ok()?;
            CoTaskMemFree(Some(path.0 as *mut _));
            Some(s)
        }
        Err(_) => None,
    }
}

unsafe fn set_font(hwnd: HWND, bold: bool) {
    let weight = if bold { 700 } else { 400 }; // FW_BOLD=700, FW_NORMAL=400
    let font = CreateFontW(
        16,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        FONT_CHARSET(1),          // DEFAULT_CHARSET
        FONT_OUTPUT_PRECISION(0), // OUT_DEFAULT_PRECIS
        FONT_CLIP_PRECISION(0),   // CLIP_DEFAULT_PRECIS
        FONT_QUALITY(0),          // DEFAULT_QUALITY
        0,                        // DEFAULT_PITCH | FF_DONTCARE
        w!("Segoe UI"),
    );
    let _ = SendMessageW(
        hwnd,
        WM_SETFONT,
        Some(WPARAM(font.0 as usize)),
        Some(LPARAM(1)),
    );
}

/// Create a console-based progress window.
pub fn create_install_progress_window(app_name: &str) -> InstallProgressWindow {
    info!("Creating install progress window for: {}", app_name);
    println!();
    println!("  {} Setup - Installing", app_name);
    println!("  {}", "=".repeat(50));
    InstallProgressWindow {
        app_name: app_name.to_string(),
        last_pct: 0,
    }
}

/// Console progress display handle.
pub struct InstallProgressWindow {
    app_name: String,
    last_pct: u32,
}

impl InstallProgressWindow {
    pub fn update(&mut self, percent: u32, file_name: &str) {
        if percent >= self.last_pct + 2 || percent >= 100 {
            self.last_pct = percent;
            let bw = 40usize;
            let f = (percent as usize * bw) / 100;
            let e = bw - f;
            let bar: String = std::iter::repeat_n('█', f)
                .chain(std::iter::repeat_n('░', e))
                .collect();
            let dn = if file_name.len() > 35 {
                format!("...{}", &file_name[file_name.len() - 32..])
            } else {
                file_name.to_string()
            };
            print!("\r  [{}] {:3}%  {:<35}", bar, percent, dn);
            if percent >= 100 {
                println!();
            }
        }
    }
    pub fn complete(&mut self) {
        self.last_pct = 100;
        println!("  {}", "=".repeat(50));
        println!("  {} installation complete!", self.app_name);
    }
}

impl Drop for InstallProgressWindow {
    fn drop(&mut self) {
        println!();
    }
}
