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
const BTN_FINISH: u16 = 1005;
const PROGRESS_BAR_ID: u16 = 1006;
const EDIT_LICENSE_ID: u16 = 1013;
const EDIT_DIR_ID: u16 = 1014;
const _LIST_COMPONENTS_ID: u16 = 1015;
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
    Ready,      // Summary page before installation
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
    user_cancelled: bool,
    // Payload for extraction during install phase
    payload_data: Option<Vec<u8>>,
    wizard_state: Option<Arc<WizardState>>,
    // Sidebar image
    sidebar_image_path: Option<String>,
    // Sidebar bitmap
    h_sidebar_bmp: HBITMAP,
    // Localized UI strings
    strings: WizardStrings,
    // Whether to use classic (simpler) styling
    classic_style: bool,
    // DPI scale factor (1.0 = 100%)
    dpi_scale: f32,
    h_page_title: HWND,
    h_page_desc: HWND,
    _h_sidebar_title: HWND,
    _h_sidebar_ver: HWND,
    h_license: HWND,
    h_dir_edit: HWND,
    h_browse: HWND,
    h_components: HWND,
    h_component_checks: Vec<HWND>, // Checkboxes for each component
    h_space_label: HWND,
    h_progress: HWND,
    h_file_label: HWND,
    h_launch_chk: HWND,
    h_back: HWND,
    h_next: HWND,
    h_cancel: HWND,
    h_finish: HWND,
}

// ===========================================================================
// Layout system — all positions computed from window size and DPI
// ===========================================================================

/// Logical layout constants (in unscaled pixels at 96 DPI).
/// All values are scaled by DPI at use time.
mod layout {
    // --- Window defaults ---
    pub const MODERN_WIN_W: i32 = 680;
    pub const CLASSIC_WIN_W: i32 = 680;
    pub const MIN_WIN_H: i32 = 480;

    // --- Sidebar (classic only) ---
    pub const _SIDEBAR_W: i32 = 150;
    pub const _SIDEBAR_PAD: i32 = 12;

    // --- Content area margins ---
    pub const MODERN_MARGIN_L: i32 = 24;
    pub const MODERN_MARGIN_R: i32 = 24;
    pub const _CLASSIC_CONTENT_X: i32 = 162;
    pub const _CLASSIC_CONTENT_PAD_R: i32 = 38;

    // --- Header ---
    pub const TITLE_Y: i32 = 16;
    pub const TITLE_H: i32 = 28;
    pub const DESC_Y_OFFSET: i32 = 30; // Below title
    pub const DESC_H: i32 = 40; // Multi-line description
    pub const HEADER_PAD_BOTTOM: i32 = 12;

    // --- Controls ---
    pub const EDIT_H: i32 = 24;
    pub const BTN_BROWSE_W: i32 = 76;
    pub const BROWSE_GAP: i32 = 8;
    pub const CHK_H: i32 = 22;
    pub const CHK_SPACING: i32 = 4;
    pub const PROGRESS_H: i32 = 22;
    pub const LABEL_H: i32 = 16;

    // --- Spacing between controls ---
    pub const SECTION_GAP: i32 = 16;
    pub const CONTROL_GAP: i32 = 8;

    // --- Button bar ---
    pub const BTN_H: i32 = 30;
    pub const BTN_W: i32 = 90;
    pub const BTN_GAP: i32 = 10;
    pub const BTN_PAD_BOTTOM: i32 = 14; // Padding below last button
    pub const SEPARATOR_PAD: i32 = 8; // Extra space above separator line

    // --- Font sizes (logical pixels) ---
    pub const FONT_TITLE: i32 = 18;
    pub const FONT_BODY: i32 = 13;
    pub const _FONT_SMALL: i32 = 11;

    // --- Branding ---
    pub const BRAND_H: i32 = 16;
    pub const STEP_SIZE: i32 = 8;
    pub const STEP_SPACING: i32 = 24;
    pub const STEP_Y: i32 = 14;
    pub const ACCENT_BAR_H: i32 = 3;
}

/// Computed layout for the current window state.
/// All values are in physical (DPI-scaled) pixels.
struct WizardLayout {
    _dpi_scale: f32,
    _classic: bool,
    _client_w: i32,
    _client_h: i32,
    // Sidebar
    _sidebar_w: i32,
    // Content area
    content_x: i32,
    _content_y: i32,
    content_w: i32,
    // Header
    title_x: i32,
    title_y: i32,
    title_w: i32,
    desc_x: i32,
    desc_y: i32,
    desc_w: i32,
    // Content start (below header)
    body_y: i32,
    // Button bar
    btn_y: i32,
    btn_h: i32,
    btn_w: i32,
    back_x: i32,
    next_x: i32,
    cancel_x: i32,
    // Separator line
    sep_y: i32,
    // Branding
    _brand_y: i32,
}

impl WizardLayout {
    #[allow(dead_code)]
    fn s(&self, v: i32) -> i32 {
        (v as f32 * self._dpi_scale) as i32
    }

    /// Compute layout from client area dimensions.
    fn compute(client_w: i32, client_h: i32, dpi_scale: f32, classic: bool) -> Self {
        let s = |v: i32| -> i32 { (v as f32 * dpi_scale) as i32 };

        // Both classic and modern use the same layout (no sidebar)
        let sidebar_w = 0;
        let content_x = s(layout::MODERN_MARGIN_L);
        let content_right = client_w - s(layout::MODERN_MARGIN_R);
        let content_w = content_right - content_x;

        let title_x = content_x;
        let title_y = s(layout::TITLE_Y);
        let title_w = content_w;
        let desc_x = content_x;
        let desc_y = title_y + s(layout::TITLE_H) + s(layout::DESC_Y_OFFSET - layout::TITLE_H);
        let desc_w = content_w;

        let body_y = desc_y + s(layout::DESC_H) + s(layout::HEADER_PAD_BOTTOM);

        // Buttons anchored to bottom
        let btn_h = s(layout::BTN_H);
        let btn_w = s(layout::BTN_W);
        let btn_gap = s(layout::BTN_GAP);
        let btn_y = client_h - s(layout::BTN_PAD_BOTTOM) - btn_h;
        let cancel_x = content_right - btn_w;
        let next_x = cancel_x - btn_gap - btn_w;
        let back_x = next_x - btn_gap - btn_w;

        // Separator line above buttons
        let sep_y = btn_y - s(layout::SEPARATOR_PAD);

        // Branding at very bottom
        let brand_y = client_h - s(layout::BRAND_H) - s(4);

        WizardLayout {
            _dpi_scale: dpi_scale,
            _classic: classic,
            _client_w: client_w,
            _client_h: client_h,
            _sidebar_w: sidebar_w,
            content_x,
            _content_y: body_y,
            content_w,
            title_x,
            title_y,
            title_w,
            desc_x,
            desc_y,
            desc_w,
            body_y,
            btn_y,
            btn_h,
            btn_w,
            back_x,
            next_x,
            cancel_x,
            sep_y,
            _brand_y: brand_y,
        }
    }

    /// Calculate the required window height for a given number of components.
    fn required_height(num_components: usize, _classic: bool, dpi_scale: f32) -> i32 {
        let s = |v: i32| -> i32 { (v as f32 * dpi_scale) as i32 };

        // Header: title + desc + padding
        let header_h = s(layout::TITLE_Y) + s(layout::TITLE_H) + s(layout::DESC_H) + s(layout::HEADER_PAD_BOTTOM);
        // Button bar: separator + padding + button + padding
        let button_bar_h = s(layout::SEPARATOR_PAD) + s(layout::BTN_PAD_BOTTOM) + s(layout::BTN_H);
        // Content: depends on number of components
        let component_area = if num_components > 0 {
            s(layout::SECTION_GAP) + (num_components as i32 * (s(layout::CHK_H) + s(layout::CHK_SPACING)))
        } else {
            0
        };
        // Minimum content: directory edit or license area
        let min_content = s(layout::EDIT_H) + s(layout::SECTION_GAP) + s(layout::BTN_BROWSE_W / 3);
        let content_h = component_area.max(min_content) + s(layout::SECTION_GAP);
        // Launch checkbox
        let launch_h = s(layout::CHK_H) + s(layout::CONTROL_GAP);

        let total = header_h + content_h + launch_h + button_bar_h;
        total.max(s(layout::MIN_WIN_H))
    }
}

/// Move a window to a new position/size.
unsafe fn move_ctrl(h: HWND, x: i32, y: i32, w: i32, ht: i32) {
    let _ = MoveWindow(h, x, y, w, ht, true);
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
    _ready_title: String,
    _ready_desc: String,
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
            _ready_title: loc.get_simple("wizard_ready_title"),
            _ready_desc: loc.get("wizard_ready_subtitle", &[("app_name", app_name)]),
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
            _ready_title: "Ready to Install".into(),
            _ready_desc: format!("Click Install to begin installing {} to your computer.", app_name),
            install_title: "Installing".into(),
            install_desc: format!("Please wait while {} is installed.", app_name),
            finish_title: "Installation Complete".into(),
            finish_desc: format!("{} has been installed to your computer.", app_name),
            finish_launch: format!("Launch {}", app_name),
            msg_confirm_cancel: "Are you sure you want to cancel?".into(),
        }
    }
}

/// Extract a text file from the compressed payload.
/// Returns None if the file is not found or payload is not available.
fn extract_file_from_payload(payload_data: &[u8], target_path: &str) -> Option<String> {
    use velocity_core::extract::CompressionFormat;
    
    // Auto-detect and decompress
    let format = CompressionFormat::detect(payload_data).unwrap_or(CompressionFormat::Zstd);
    let decompressed = match format {
        CompressionFormat::Zstd => zstd::decode_all(payload_data).ok()?,
        CompressionFormat::Lzma2 => {
            let mut output = Vec::new();
            if payload_data.len() >= 6
                && payload_data[0] == 0xFD
                && payload_data[1] == 0x37
                && payload_data[2] == 0x7A
                && payload_data[3] == 0x58
                && payload_data[4] == 0x5A
                && payload_data[5] == 0x00
            {
                lzma_rs::xz_decompress(&mut std::io::Cursor::new(payload_data), &mut output).ok()?;
            } else {
                lzma_rs::lzma_decompress(&mut std::io::Cursor::new(payload_data), &mut output).ok()?;
            }
            output
        }
    };
    
    // Parse tar and find the file
    let mut archive = tar::Archive::new(decompressed.as_slice());
    let entries = archive.entries().ok()?;
    for entry_result in entries {
        let mut entry = entry_result.ok()?;
        let path = entry.path().ok()?.into_owned();
        let path_str = path.to_string_lossy();
        // Match the target path (handle both with and without leading ./)
        if path_str == target_path || path_str.trim_start_matches("./") == target_path.trim_start_matches("./") {
            let mut contents = String::new();
            std::io::Read::read_to_string(&mut entry, &mut contents).ok()?;
            return Some(contents);
        }
    }
    None
}

/// Run the native Win32 wizard.
///
/// If `payload_data` is provided, the wizard will perform extraction during
/// the Installing page and show real progress. Otherwise, it just collects
/// user choices and returns.
///
/// If `classic_style` is true, uses a simpler visual style (gray sidebar, no images)
/// for the "classic" theme. Otherwise uses the accent color and sidebar image for "modern".
pub fn run_native_wizard(
    manifest: &VelocityManifest,
    payload_data: Option<Vec<u8>>,
    classic_style: bool,
) -> Result<NativeWizardResult> {
    info!("Starting native wizard for: {} (classic_style={})", manifest.app.name, classic_style);

    let default_dir = velocity_config::VariableResolver::new(&PathBuf::from(format!(
        "C:\\Program Files\\{}",
        manifest.app.name
    )))
    .resolve(&manifest.install.default_dir);

    // Extract license text from payload (preferred) or filesystem (fallback)
    let license_text = if let Some(ref license_path) = manifest.app.license {
        let path_str = license_path.to_string_lossy();
        // First try to extract from payload
        if let Some(ref payload) = payload_data {
            if let Some(text) = extract_file_from_payload(payload, &path_str) {
                text
            } else {
                // Fallback to filesystem
                std::fs::read_to_string(license_path).unwrap_or_default()
            }
        } else {
            std::fs::read_to_string(license_path).unwrap_or_default()
        }
    } else {
        String::new()
    };

    let has_license = manifest.app.license.is_some();
    let has_components = !manifest.components.is_empty();
    let tree_nodes = velocity_core::component_tree::flatten_component_tree(&manifest.components);
    
    // Both classic and modern use the configured accent color
    let accent = parse_accent_color(&manifest.ui.accent_color);
    
    // Classic style doesn't use sidebar images
    let sidebar_image_path = if classic_style {
        None
    } else {
        manifest.ui.sidebar.as_ref().map(|p| p.to_string_lossy().to_string())
    };

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
        classic_style,
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
    classic_style: bool,
) -> Result<NativeWizardResult> {
    // SAFETY: Win32 window creation and message loop. INITCOMMONCONTROLSEX.cbSize is
    // set correctly. WizardData is heap-allocated via Box::into_raw and stored in
    // GWLP_USERDATA; reclaimed via Box::from_raw in WM_DESTROY. All child HWNDs are
    // stored in WizardData and become invalid when the window is destroyed (correct).
    // Message loop runs on the creating thread (single-threaded apartment).
    unsafe {
        // Make the process DPI-aware so we get real pixel coordinates
        let _ = SetProcessDPIAware();
        
        // Calculate DPI scale factor (1.0 = 100% = 96 DPI)
        let hdc = GetDC(None);
        let dpi_x = GetDeviceCaps(Some(hdc), LOGPIXELSX);
        let _ = ReleaseDC(None, hdc);
        let scale = dpi_x as f32 / 96.0;
        tracing::info!("DPI: {} (scale: {:.2})", dpi_x, scale);
        
        // Helper to scale a value by DPI
        let s = |v: i32| -> i32 { (v as f32 * scale) as i32 };

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
        pages.push(WizardPage::Ready);      // Summary page before installation
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
            user_cancelled: false,
            payload_data,
            wizard_state: None,
            sidebar_image_path,
            h_sidebar_bmp: HBITMAP::default(),
            strings,
            classic_style,
            dpi_scale: scale,
            h_page_title: HWND::default(),
            h_page_desc: HWND::default(),
            _h_sidebar_title: HWND::default(),
            _h_sidebar_ver: HWND::default(),
            h_license: HWND::default(),
            h_dir_edit: HWND::default(),
            h_browse: HWND::default(),
            h_components: HWND::default(),
            h_component_checks: Vec::new(),
            h_space_label: HWND::default(),
            h_progress: HWND::default(),
            h_file_label: HWND::default(),
            h_launch_chk: HWND::default(),
            h_back: HWND::default(),
            h_next: HWND::default(),
            h_cancel: HWND::default(),
            h_finish: HWND::default(),
        });
        let data_ptr = Box::into_raw(data);

        let class_name = w!("VelocityWizard");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wizard_wnd_proc),
            hInstance: hi,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: if classic_style {
                // Light gray background for classic
                CreateSolidBrush(COLORREF(0x00F5F5F5))
            } else {
                // Dark background for modern
                CreateSolidBrush(COLORREF(0x002D2D30))
            },
            lpszClassName: class_name,
            ..Default::default()
        };
        let class_atom = RegisterClassExW(&wc);
        if class_atom == 0 {
            let err = windows::core::Error::from_thread();
            tracing::error!("RegisterClassExW failed: {}", err);
        } else {
            tracing::info!("RegisterClassExW succeeded, atom = {}", class_atom);
        }

        // Calculate window dimensions using layout system
        let num_components = components.len();
        let (win_w, win_h) = if classic_style {
            let needed_h = WizardLayout::required_height(num_components, true, scale);
            (s(layout::CLASSIC_WIN_W), needed_h)
        } else {
            let needed_h = WizardLayout::required_height(num_components, false, scale);
            (s(layout::MODERN_WIN_W), needed_h)
        };

        tracing::info!("Creating window: {}x{}", win_w, win_h);
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
                tracing::info!("Window created successfully, hwnd = {:?}", hwnd.0);
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
                // Explicitly show the window (WS_OVERLAPPEDWINDOW doesn't include WS_VISIBLE)
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = UpdateWindow(hwnd);
                tracing::info!("Window shown and updated");

                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).into() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            Err(e) => {
                tracing::error!("CreateWindowExW failed: {}", e);
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
            cancelled: data.user_cancelled,
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
                let d = &*dp;
                if d.classic_style {
                    paint_classic_background(hwnd, d);
                } else {
                    paint_modern_background(hwnd, d);
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CTLCOLORSTATIC => {
            let hdc = HDC(wparam.0 as *mut _);
            let _target = HWND(lparam.0 as *mut _);
            if !dp.is_null() {
                let d = &*dp;
                if !d.classic_style {
                    // Modern dark theme: light text on dark background
                    SetTextColor(hdc, COLORREF(0x00F1F1F1)); // Light text
                    SetBkMode(hdc, TRANSPARENT);
                    let bg = COLORREF(0x002D2D30); // Dark background
                    let brush = CreateSolidBrush(bg);
                    return LRESULT(HBRUSH(brush.0).0 as isize);
                } else {
                    // Classic style: dark text on light background
                    SetTextColor(hdc, COLORREF(0x00333333));
                    SetBkMode(hdc, TRANSPARENT);
                    let brush = CreateSolidBrush(COLORREF(0x00F5F5F5));
                    return LRESULT(HBRUSH(brush.0).0 as isize);
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CTLCOLOREDIT => {
            if !dp.is_null() {
                let d = &*dp;
                let hdc = HDC(wparam.0 as *mut _);
                if !d.classic_style {
                    // Modern dark theme for edit controls
                    SetTextColor(hdc, COLORREF(0x00F1F1F1)); // Light text
                    SetBkColor(hdc, COLORREF(0x003E3E42)); // Dark input bg
                    let brush = CreateSolidBrush(COLORREF(0x003E3E42));
                    return LRESULT(HBRUSH(brush.0).0 as isize);
                } else {
                    // Classic light theme: dark text on white background
                    SetTextColor(hdc, COLORREF(0x00333333));
                    SetBkColor(hdc, COLORREF(0x00FFFFFF));
                    let brush = CreateSolidBrush(COLORREF(0x00FFFFFF));
                    return LRESULT(HBRUSH(brush.0).0 as isize);
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CTLCOLORBTN => {
            if !dp.is_null() {
                let d = &*dp;
                let hdc = HDC(wparam.0 as *mut _);
                let target = HWND(lparam.0 as *mut _);
                
                // Primary action buttons (Next/Install/Finish) get accent color in both themes
                let is_primary = target == d.h_next || target == d.h_finish;
                
                if is_primary {
                    let rgb = d.accent_rgb;
                    let accent = COLORREF(rgb[0] as u32 | (rgb[1] as u32) << 8 | (rgb[2] as u32) << 16);
                    SetTextColor(hdc, COLORREF(0x00FFFFFF));
                    SetBkColor(hdc, accent);
                    let brush = CreateSolidBrush(accent);
                    return LRESULT(HBRUSH(brush.0).0 as isize);
                } else if !d.classic_style {
                    // Modern dark theme for non-primary buttons
                    SetTextColor(hdc, COLORREF(0x00F1F1F1));
                    SetBkColor(hdc, COLORREF(0x003E3E42));
                    let brush = CreateSolidBrush(COLORREF(0x003E3E42));
                    return LRESULT(HBRUSH(brush.0).0 as isize);
                }
                // Classic non-primary: fall through to system default
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CTLCOLORLISTBOX => {
            if !dp.is_null() {
                let d = &*dp;
                if !d.classic_style {
                    // Dark theme for listbox
                    let hdc = HDC(wparam.0 as *mut _);
                    SetTextColor(hdc, COLORREF(0x00F1F1F1)); // Light text
                    SetBkColor(hdc, COLORREF(0x003E3E42)); // Dark bg
                    let brush = CreateSolidBrush(COLORREF(0x003E3E42));
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
            tracing::info!("WM_COMMAND received, button id = {}", id);
            let d = &mut *dp;
            match id {
                BTN_BACK => {
                    tracing::info!("Back button clicked");
                    if d.page_idx > 0 {
                        d.page_idx -= 1;
                        show_page(hwnd, d);
                    }
                }
                BTN_NEXT => {
                    tracing::info!("Next button clicked, page_idx = {}", d.page_idx);
                    handle_next(hwnd, d)
                },
                BTN_CANCEL => {
                    tracing::info!("Cancel button clicked");
                    let mw = wn(&d.strings.msg_confirm_cancel);
                    let tw = wn(&d.app_name);
                    let r = MessageBoxW(
                        Some(hwnd),
                        PCWSTR(mw.as_ptr()),
                        PCWSTR(tw.as_ptr()),
                        MB_YESNO | MB_ICONQUESTION,
                    );
                    if r == IDYES {
                        d.user_cancelled = true;
                        // Signal the install thread to stop (if running)
                        if let Some(ref state) = d.wizard_state {
                            state.cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
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
                BTN_FINISH => {
                    // Check launch checkbox
                    let checked = IsDlgButtonChecked(hwnd, CHK_LAUNCH_ID as i32);
                    d.launch_after = checked == 1; // BST_CHECKED
                    d.install_completed = false; // Wizard didn't extract files, runtime will
                    let _ = DestroyWindow(hwnd);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            if dp.is_null() {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            let d = &mut *dp;
            let current_page = d.pages[d.page_idx];
            // If installation is finished, just close without prompting
            if current_page == WizardPage::Finished {
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            // Prompt the user before closing
            let mw = wn(&d.strings.msg_confirm_cancel);
            let tw = wn(&d.app_name);
            let r = MessageBoxW(
                Some(hwnd),
                PCWSTR(mw.as_ptr()),
                PCWSTR(tw.as_ptr()),
                MB_YESNO | MB_ICONQUESTION,
            );
            if r == IDYES {
                d.user_cancelled = true;
                // Signal the install thread to stop (if running)
                if let Some(ref state) = d.wizard_state {
                    state.cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            } else {
                LRESULT(0) // Swallow the close — user chose to stay
            }
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

    // Compute layout from actual client area
    let mut client_rect = RECT::default();
    let _ = GetClientRect(parent, &mut client_rect);
    let lay = WizardLayout::compute(client_rect.right, client_rect.bottom, d.dpi_scale, d.classic_style);
    let s = |v: i32| -> i32 { (v as f32 * d.dpi_scale) as i32 };

    // --- Page title ---
    d.h_page_title = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("Welcome"),
        WS_CHILD | WS_VISIBLE,
        lay.title_x, lay.title_y, lay.title_w, s(layout::TITLE_H),
        Some(parent), Some(HMENU(PAGE_TITLE_ID as *mut _)), Some(hi), None,
    ).unwrap_or_default();
    set_font(d.h_page_title, true);

    // --- Page description ---
    d.h_page_desc = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!("Click Next to begin."),
        WS_CHILD | WS_VISIBLE,
        lay.desc_x, lay.desc_y, lay.desc_w, s(layout::DESC_H),
        Some(parent), Some(HMENU(PAGE_DESC_ID as *mut _)), Some(hi), None,
    ).unwrap_or_default();
    set_font(d.h_page_desc, false);

    // --- License edit (hidden) ---
    let license_style = ws(WS_CHILD.0 | WS_VSCROLL.0 | 0x0004 | 0x0800 | 0x0040);
    let license_h = lay.sep_y - lay.body_y - s(layout::CONTROL_GAP);
    d.h_license = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        w!("EDIT"),
        w!(""),
        license_style,
        lay.content_x, lay.body_y, lay.content_w, license_h.max(s(100)),
        Some(parent), Some(HMENU(EDIT_LICENSE_ID as *mut _)), Some(hi), None,
    ).unwrap_or_default();
    set_font(d.h_license, false);

    // --- Dir edit (hidden) ---
    let dw = wn(&d.default_dir);
    let dir_style = ws(WS_CHILD.0 | 0x0080);
    let browse_w = s(layout::BTN_BROWSE_W);
    let dir_w = lay.content_w - browse_w - s(layout::BROWSE_GAP);
    d.h_dir_edit = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        w!("EDIT"),
        PCWSTR(dw.as_ptr()),
        dir_style,
        lay.content_x, lay.body_y, dir_w, s(layout::EDIT_H),
        Some(parent), Some(HMENU(EDIT_DIR_ID as *mut _)), Some(hi), None,
    ).unwrap_or_default();
    set_font(d.h_dir_edit, false);

    // --- Browse button (hidden) ---
    d.h_browse = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        w!("B&rowse..."),
        WS_CHILD | WS_TABSTOP,
        lay.content_x + dir_w + s(layout::BROWSE_GAP), lay.body_y, browse_w, s(layout::EDIT_H),
        Some(parent), Some(HMENU(BTN_BROWSE as *mut _)), Some(hi), None,
    ).unwrap_or_default();
    set_font(d.h_browse, false);

    // --- Component checkboxes (hidden) ---
    let chk_style = ws(WS_CHILD.0 | WS_TABSTOP.0 | 0x00000003); // BS_AUTOCHECKBOX
    d.h_component_checks.clear();
    for (i, node) in d.all_components.iter().enumerate() {
        let display = velocity_core::component_tree::format_node_display(node);
        let cw = wn(&display);
        let h_chk = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            PCWSTR(cw.as_ptr()),
            chk_style,
            lay.content_x, lay.body_y, // Placeholder — repositioned in show_page
            lay.content_w, s(layout::CHK_H),
            Some(parent), Some(HMENU((3000 + i) as *mut _)), Some(hi), None,
        ).unwrap_or_default();
        set_font(h_chk, false);
        if node.selected {
            SendMessageW(h_chk, BM_SETCHECK, Some(WPARAM(1)), Some(LPARAM(0)));
        }
        // Disable visual styles for checkboxes so WM_CTLCOLORBTN dark theme works
        let empty = wn("");
        let _ = SetWindowTheme(h_chk, PCWSTR(empty.as_ptr()), PCWSTR(empty.as_ptr()));
        d.h_component_checks.push(h_chk);
    }
    d.h_components = HWND::default();

    // --- Disk space label (hidden) ---
    let total_size: u64 = d.all_components.iter().filter(|n| n.selected).map(|n| n.size).sum();
    let space_text = format!("Space required: {}", velocity_core::component_tree::format_size(total_size));
    d.h_space_label = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!(""),
        WS_CHILD,
        lay.content_x, lay.body_y, lay.content_w, s(layout::LABEL_H), // Placeholder
        Some(parent), Some(HMENU(STATIC_SPACE_ID as *mut _)), Some(hi), None,
    ).unwrap_or_default();
    set_txt(d.h_space_label, &space_text);
    set_font(d.h_space_label, false);

    // --- Progress bar (hidden) ---
    d.h_progress = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("msctls_progress32"),
        w!(""),
        WS_CHILD,
        lay.content_x, lay.body_y, lay.content_w, s(layout::PROGRESS_H),
        Some(parent), Some(HMENU(PROGRESS_BAR_ID as *mut _)), Some(hi), None,
    ).unwrap_or_default();
    SendMessageW(d.h_progress, PBM_SETRANGE32, Some(WPARAM(0)), Some(LPARAM(100)));

    // --- File label (hidden) ---
    d.h_file_label = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("STATIC"),
        w!(""),
        WS_CHILD,
        lay.content_x, lay.body_y + s(layout::PROGRESS_H) + s(layout::CONTROL_GAP),
        lay.content_w, s(layout::LABEL_H),
        Some(parent), Some(HMENU(STATIC_FILE_ID as *mut _)), Some(hi), None,
    ).unwrap_or_default();
    set_font(d.h_file_label, false);

    // --- Launch checkbox (hidden) ---
    let launch_style = ws(WS_CHILD.0 | WS_TABSTOP.0 | 0x00000003);
    d.h_launch_chk = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        w!("Launch application after install"),
        launch_style,
        lay.content_x, lay.body_y, // Placeholder — repositioned in show_page
        lay.content_w.min(s(350)), s(layout::CHK_H),
        Some(parent), Some(HMENU(CHK_LAUNCH_ID as *mut _)), Some(hi), None,
    ).unwrap_or_default();
    set_font(d.h_launch_chk, false);
    // Disable visual styles so dark theme works
    let empty_theme = wn("");
    let _ = SetWindowTheme(d.h_launch_chk, PCWSTR(empty_theme.as_ptr()), PCWSTR(empty_theme.as_ptr()));

    // --- Navigation buttons ---
    let back_w = wn(&d.strings.btn_back);
    let next_w = wn(&d.strings.btn_next);
    let cancel_w = wn(&d.strings.btn_cancel);

    d.h_back = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        PCWSTR(back_w.as_ptr()),
        WS_CHILD | WS_TABSTOP | WS_VISIBLE,
        lay.back_x, lay.btn_y, lay.btn_w, lay.btn_h,
        Some(parent), Some(HMENU(BTN_BACK as *mut _)), Some(hi), None,
    ).unwrap_or_default();
    set_font(d.h_back, false);

    d.h_next = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        PCWSTR(next_w.as_ptr()),
        WS_CHILD | WS_TABSTOP | WS_VISIBLE,
        lay.next_x, lay.btn_y, lay.btn_w, lay.btn_h,
        Some(parent), Some(HMENU(BTN_NEXT as *mut _)), Some(hi), None,
    ).unwrap_or_default();
    set_font(d.h_next, false);

    d.h_cancel = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        PCWSTR(cancel_w.as_ptr()),
        WS_CHILD | WS_TABSTOP | WS_VISIBLE,
        lay.cancel_x, lay.btn_y, lay.btn_w, lay.btn_h,
        Some(parent), Some(HMENU(BTN_CANCEL as *mut _)), Some(hi), None,
    ).unwrap_or_default();
    set_font(d.h_cancel, false);

    d.h_finish = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("BUTTON"),
        w!("&Finish"),
        WS_CHILD | WS_TABSTOP,
        lay.next_x, lay.btn_y, lay.btn_w, lay.btn_h,
        Some(parent), Some(HMENU(BTN_FINISH as *mut _)), Some(hi), None,
    ).unwrap_or_default();
    set_font(d.h_finish, false);
}

unsafe fn show_page(hwnd: HWND, d: &mut WizardData) {
    let page = d.pages[d.page_idx];
    let is_first = d.page_idx == 0;
    let is_install = page == WizardPage::Installing;
    let is_ready = page == WizardPage::Ready;

    // Compute layout from current client area
    let mut client_rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut client_rect);
    let lay = WizardLayout::compute(client_rect.right, client_rect.bottom, d.dpi_scale, d.classic_style);
    let s = |v: i32| -> i32 { (v as f32 * d.dpi_scale) as i32 };

    // --- Reposition header controls ---
    move_ctrl(d.h_page_title, lay.title_x, lay.title_y, lay.title_w, s(layout::TITLE_H));
    move_ctrl(d.h_page_desc, lay.desc_x, lay.desc_y, lay.desc_w, s(layout::DESC_H));

    // --- Reposition buttons to bottom ---
    move_ctrl(d.h_back, lay.back_x, lay.btn_y, lay.btn_w, lay.btn_h);
    move_ctrl(d.h_next, lay.next_x, lay.btn_y, lay.btn_w, lay.btn_h);
    move_ctrl(d.h_cancel, lay.cancel_x, lay.btn_y, lay.btn_w, lay.btn_h);
    move_ctrl(d.h_finish, lay.next_x, lay.btn_y, lay.btn_w, lay.btn_h);

    let is_finished = page == WizardPage::Finished;

    // --- Button visibility ---
    if is_first || is_install || is_finished {
        let _ = ShowWindow(d.h_back, SW_HIDE);
    } else {
        let _ = ShowWindow(d.h_back, SW_SHOW);
    }
    if is_install {
        let _ = ShowWindow(d.h_cancel, SW_HIDE);
    } else {
        let _ = ShowWindow(d.h_cancel, SW_SHOW);
    }

    let next_text = if is_ready {
        &d.strings.btn_install
    } else if page == WizardPage::Finished {
        &d.strings.btn_finish
    } else if is_install {
        &d.strings.btn_cancel
    } else {
        &d.strings.btn_next
    };
    let nw = wn(next_text);
    let _ = SetWindowTextW(d.h_next, PCWSTR(nw.as_ptr()));

    if is_install {
        let _ = ShowWindow(d.h_next, SW_HIDE);
    } else {
        let _ = ShowWindow(d.h_next, SW_SHOW);
    }

    // --- Hide all page-specific controls first ---
    for &ctrl in &[
        d.h_license, d.h_dir_edit, d.h_browse, d.h_components,
        d.h_space_label, d.h_progress, d.h_file_label, d.h_launch_chk,
    ] {
        let _ = ShowWindow(ctrl, SW_HIDE);
    }
    for &h_chk in &d.h_component_checks {
        let _ = ShowWindow(h_chk, SW_HIDE);
    }

    // --- Position and show controls for current page ---
    let body_y = lay.body_y;
    let content_x = lay.content_x;
    let content_w = lay.content_w;
    let available_h = lay.sep_y - body_y - s(layout::CONTROL_GAP);

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
            // License fills available space
            let license_h = available_h.max(s(100));
            move_ctrl(d.h_license, content_x, body_y, content_w, license_h);
            set_txt(d.h_license, &d.license_text);
            show_c(d.h_license);
        }
        WizardPage::Directory => {
            set_txt(d.h_page_title, &d.strings.dir_title);
            set_txt(d.h_page_desc, &d.strings.dir_desc);
            show_c(d.h_page_title);
            show_c(d.h_page_desc);
            // Directory edit + browse button
            let browse_w = s(layout::BTN_BROWSE_W);
            let dir_w = content_w - browse_w - s(layout::BROWSE_GAP);
            move_ctrl(d.h_dir_edit, content_x, body_y, dir_w, s(layout::EDIT_H));
            move_ctrl(d.h_browse, content_x + dir_w + s(layout::BROWSE_GAP), body_y, browse_w, s(layout::EDIT_H));
            set_txt(d.h_dir_edit, &d.install_dir);
            show_c(d.h_dir_edit);
            show_c(d.h_browse);
        }
        WizardPage::Components => {
            set_txt(d.h_page_title, &d.strings.components_title);
            set_txt(d.h_page_desc, &d.strings.components_desc);
            show_c(d.h_page_title);
            show_c(d.h_page_desc);
            // Position checkboxes starting at body_y
            let chk_h = s(layout::CHK_H);
            let chk_sp = s(layout::CHK_SPACING);
            for (i, &h_chk) in d.h_component_checks.iter().enumerate() {
                let chk_y = body_y + (i as i32 * (chk_h + chk_sp));
                move_ctrl(h_chk, content_x, chk_y, content_w, chk_h);
                show_c(h_chk);
            }
            // Space label below checkboxes
            let num_chk = d.h_component_checks.len() as i32;
            let space_y = body_y + (num_chk * (chk_h + chk_sp)) + s(layout::SECTION_GAP);
            move_ctrl(d.h_space_label, content_x, space_y, content_w, s(layout::LABEL_H));
            show_c(d.h_space_label);
        }
        WizardPage::Ready => {
            let ready_title = format!("Ready to Install {}", d.app_name);
            let num_selected = d.h_component_checks.iter().filter(|&&h| {
                SendMessageW(h, BM_GETCHECK, Some(WPARAM(0)), Some(LPARAM(0))).0 == 1
            }).count();
            let ready_desc = format!(
                "Setup has all the information it needs. Click Install to begin.\r\n\r\n\
                 Installation folder: {}\r\n\
                 Components: {} selected",
                d.install_dir, num_selected
            );
            set_txt(d.h_page_title, &ready_title);
            set_txt(d.h_page_desc, &ready_desc);
            show_c(d.h_page_title);
            show_c(d.h_page_desc);

            // Measure multi-line description text height and resize control
            let hdc = GetDC(Some(d.h_page_desc));
            let hfont = SendMessageW(d.h_page_desc, WM_GETFONT, Some(WPARAM(0)), Some(LPARAM(0)));
            let old_font = SelectObject(hdc, HGDIOBJ(hfont.0 as *mut _));
            let mut rc = RECT { left: 0, top: 0, right: lay.desc_w, bottom: 0 };
            let mut desc_buf = wn(&ready_desc);
            let _ = DrawTextW(hdc, &mut desc_buf, &mut rc, DT_WORDBREAK | DT_CALCRECT);
            let _ = SelectObject(hdc, old_font);
            let _ = ReleaseDC(Some(d.h_page_desc), hdc);
            let desc_h = (rc.bottom - rc.top).max(s(layout::DESC_H));
            move_ctrl(d.h_page_desc, lay.desc_x, lay.desc_y, lay.desc_w, desc_h);
            // Launch checkbox is NOT shown here — only on the Finished page
        }
        WizardPage::Installing => {
            set_txt(d.h_page_title, &d.strings.install_title);
            set_txt(d.h_page_desc, &d.strings.install_desc);
            show_c(d.h_page_title);
            show_c(d.h_page_desc);
            // Progress bar centered in available area
            let prog_y = body_y + s(layout::SECTION_GAP);
            move_ctrl(d.h_progress, content_x, prog_y, content_w, s(layout::PROGRESS_H));
            move_ctrl(d.h_file_label, content_x, prog_y + s(layout::PROGRESS_H) + s(layout::CONTROL_GAP), content_w, s(layout::LABEL_H));
            show_c(d.h_progress);
            show_c(d.h_file_label);
        }
        WizardPage::Finished => {
            set_txt(d.h_page_title, &d.strings.finish_title);
            set_txt(d.h_page_desc, &d.strings.finish_desc);
            show_c(d.h_page_title);
            show_c(d.h_page_desc);
            // Launch checkbox
            let launch_y = body_y + s(layout::SECTION_GAP);
            move_ctrl(d.h_launch_chk, content_x, launch_y, content_w.min(s(350)), s(layout::CHK_H));
            set_txt(d.h_launch_chk, &d.strings.finish_launch);
            show_c(d.h_launch_chk);
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
            // Read checkbox states to determine selected components
            let mut raw_ids: Vec<String> = Vec::new();
            for (i, h_chk) in d.h_component_checks.iter().enumerate() {
                if i < d.all_components.len() {
                    let checked = SendMessageW(*h_chk, BM_GETCHECK, Some(WPARAM(0)), Some(LPARAM(0)));
                    if checked.0 == 1 { // BST_CHECKED
                        raw_ids.push(d.all_components[i].id.clone());
                    }
                }
            }
            // Resolve dependencies to ensure required components are included
            d.selected_components = velocity_core::component_tree::resolve_dependencies(
                &[],
                &raw_ids,
            );
            // If resolve_dependencies with empty components returns raw_ids, use those
            if d.selected_components.is_empty() {
                d.selected_components = raw_ids;
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
        WizardPage::Ready => {
            // Start installation
            if let Some(payload) = &d.payload_data {
                start_installation(hwnd, d, payload.clone());
            }
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
            tracing::info!("handle_next: default branch, page_idx = {}, pages.len() = {}", d.page_idx, d.pages.len());
            if d.page_idx < d.pages.len() - 1 {
                d.page_idx += 1;
                tracing::info!("handle_next: advanced to page_idx = {}, page = {:?}", d.page_idx, d.pages[d.page_idx]);
                show_page(hwnd, d);
                // If we just entered the Installing page, skip to finished
                // The runtime will handle extraction after elevation
                if d.pages[d.page_idx] == WizardPage::Installing {
                    tracing::info!("handle_next: on Installing page, skipping to Finished");
                    // Don't extract in the wizard - runtime handles it after elevation
                    d.install_completed = false;
                    d.page_idx += 1;
                    show_page(hwnd, d);
                    tracing::info!("handle_next: skipped to page_idx = {}, page = {:?}", d.page_idx, d.pages[d.page_idx]);
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

unsafe fn paint_classic_background(hwnd: HWND, d: &WizardData) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);
    let s = |v: i32| -> i32 { (v as f32 * d.dpi_scale) as i32 };

    // Get actual client area dimensions
    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    let win_w = rc.right;
    let win_h = rc.bottom;

    // Classic style: light background with subtle accent bar at top
    let bg_brush = CreateSolidBrush(COLORREF(0x00F5F5F5)); // Light gray background
    let full_rect = RECT { left: 0, top: 0, right: win_w, bottom: win_h };
    let _ = FillRect(hdc, &full_rect, bg_brush);
    let _ = DeleteObject(bg_brush.into());

    // Accent bar at top
    let rgb = d.accent_rgb;
    let accent = COLORREF(rgb[0] as u32 | (rgb[1] as u32) << 8 | (rgb[2] as u32) << 16);
    let accent_rect = RECT { left: 0, top: 0, right: win_w, bottom: s(layout::ACCENT_BAR_H) };
    let accent_brush = CreateSolidBrush(accent);
    let _ = FillRect(hdc, &accent_rect, accent_brush);
    let _ = DeleteObject(accent_brush.into());

    // Separator line above button bar
    let sep_y = s(layout::BTN_PAD_BOTTOM) + s(layout::BTN_H);
    let sep_rect = RECT {
        left: s(layout::MODERN_MARGIN_L),
        top: win_h - sep_y,
        right: win_w - s(layout::MODERN_MARGIN_R),
        bottom: win_h - sep_y + 1,
    };
    let sep_pen = CreatePen(PS_SOLID, 1, COLORREF(0x00D0D0D0));
    let old_pen = SelectObject(hdc, sep_pen.into());
    let _ = MoveToEx(hdc, sep_rect.left, sep_rect.top, None);
    let _ = LineTo(hdc, sep_rect.right, sep_rect.top);
    let _ = SelectObject(hdc, old_pen);
    let _ = DeleteObject(sep_pen.into());

    // Branding text at bottom-left
    SetTextColor(hdc, COLORREF(0x00999999));
    SetBkMode(hdc, TRANSPARENT);
    let brand_text = wn(&format!("{} {}", d.app_name, d.version));
    let mut br = RECT {
        left: s(layout::MODERN_MARGIN_L),
        top: win_h - s(layout::BRAND_H) - s(4),
        right: win_w / 2,
        bottom: win_h - s(4),
    };
    let brand_slice: &mut [u16] = &mut brand_text.clone();
    let _ = DrawTextW(hdc, brand_slice, &mut br, DT_LEFT | DT_BOTTOM | DT_SINGLELINE);

    let _ = EndPaint(hwnd, &ps);
}

/// Paint the modern dark-style background with accent bar and step indicator.
unsafe fn paint_modern_background(hwnd: HWND, d: &WizardData) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);
    let s = |v: i32| -> i32 { (v as f32 * d.dpi_scale) as i32 };
    
    // Get window dimensions
    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    let win_w = rc.right;
    let win_h = rc.bottom;
    
    // 1. Fill entire background with dark color
    let bg_brush = CreateSolidBrush(COLORREF(0x002D2D30)); // VS dark
    let _ = FillRect(hdc, &rc, bg_brush);
    let _ = DeleteObject(bg_brush.into());
    
    // 2. Draw accent bar at top (thin line)
    let accent_rgb = d.accent_rgb;
    let accent_color = COLORREF(accent_rgb[0] as u32 | (accent_rgb[1] as u32) << 8 | (accent_rgb[2] as u32) << 16);
    let accent_rect = RECT {
        left: 0,
        top: 0,
        right: win_w,
        bottom: s(layout::ACCENT_BAR_H),
    };
    let accent_brush = CreateSolidBrush(accent_color);
    let _ = FillRect(hdc, &accent_rect, accent_brush);
    let _ = DeleteObject(accent_brush.into());
    
    // 3. Draw step indicator at top
    let step_y = s(layout::STEP_Y);
    let step_size = s(layout::STEP_SIZE);
    let step_spacing = s(layout::STEP_SPACING);
    let total_steps = d.pages.len() - 1; // Exclude Finished page
    let indicator_start_x = win_w - s(layout::MODERN_MARGIN_R) - (total_steps as i32 * step_spacing);
    
    for i in 0..total_steps {
        let x = indicator_start_x + (i as i32 * step_spacing);
        let step_rect = RECT {
            left: x,
            top: step_y,
            right: x + step_size,
            bottom: step_y + step_size,
        };
        
        if i < d.page_idx {
            // Completed step: filled accent color
            let brush = CreateSolidBrush(accent_color);
            let _ = FillRect(hdc, &step_rect, brush);
            let _ = DeleteObject(brush.into());
        } else if i == d.page_idx {
            // Current step: accent border, dark fill
            let brush = CreateSolidBrush(accent_color);
            let pen = CreatePen(PS_SOLID, s(2), accent_color);
            let old_brush = SelectObject(hdc, brush.into());
            let old_pen = SelectObject(hdc, pen.into());
            let _ = Rectangle(hdc, step_rect.left, step_rect.top, step_rect.right, step_rect.bottom);
            let _ = SelectObject(hdc, old_brush);
            let _ = SelectObject(hdc, old_pen);
            let _ = DeleteObject(brush.into());
            let _ = DeleteObject(pen.into());
        } else {
            // Future step: dark gray border
            let brush = CreateSolidBrush(COLORREF(0x003E3E42));
            let pen = CreatePen(PS_SOLID, s(1), COLORREF(0x00555555));
            let old_brush = SelectObject(hdc, brush.into());
            let old_pen = SelectObject(hdc, pen.into());
            let _ = Rectangle(hdc, step_rect.left, step_rect.top, step_rect.right, step_rect.bottom);
            let _ = SelectObject(hdc, old_brush);
            let _ = SelectObject(hdc, old_pen);
            let _ = DeleteObject(brush.into());
            let _ = DeleteObject(pen.into());
        }
    }
    
    // 4. Draw app branding at bottom-left
    SetTextColor(hdc, COLORREF(0x00888888)); // Muted text
    SetBkMode(hdc, TRANSPARENT);
    let brand = wn(&format!("{} v{}", d.app_name, d.version));
    let mut br = RECT {
        left: s(layout::MODERN_MARGIN_L),
        top: win_h - s(layout::BRAND_H) - s(4),
        right: s(200),
        bottom: win_h - s(4),
    };
    let brand_slice: &mut [u16] = &mut brand.clone();
    let _ = DrawTextW(hdc, brand_slice, &mut br, DT_LEFT | DT_BOTTOM | DT_SINGLELINE);
    
    // 5. Draw separator line above buttons
    let sep_y = win_h - s(layout::BTN_PAD_BOTTOM) - s(layout::BTN_H) - s(layout::SEPARATOR_PAD);
    let sep_rect = RECT {
        left: s(layout::MODERN_MARGIN_L),
        top: sep_y,
        right: win_w - s(layout::MODERN_MARGIN_R),
        bottom: sep_y + 1,
    };
    let sep_brush = CreateSolidBrush(COLORREF(0x003E3E42));
    let _ = FillRect(hdc, &sep_rect, sep_brush);
    let _ = DeleteObject(sep_brush.into());
    
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
    // Get DPI scale from the window's WizardData via parent lookup
    // Default to 1.0 if we can't get it — fonts will still render at base size
    let hdc = GetDC(Some(hwnd));
    let dpi_y = GetDeviceCaps(Some(hdc), LOGPIXELSY);
    let _ = ReleaseDC(Some(hwnd), hdc);
    let scale = dpi_y as f32 / 96.0;

    let base_size = if bold { layout::FONT_TITLE } else { layout::FONT_BODY };
    let font_size = (base_size as f32 * scale) as i32;
    let weight = if bold { 700 } else { 400 }; // FW_BOLD=700, FW_NORMAL=400
    let font = CreateFontW(
        font_size,
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
        FONT_QUALITY(2),          // CLEARTYPE_QUALITY
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
