use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// The complete installer manifest, parsed from `velocity.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VelocityManifest {
    /// Application metadata
    pub app: AppConfig,
    /// Installation settings
    #[serde(default)]
    pub install: InstallConfig,
    /// Files to include in the package
    #[serde(default)]
    pub files: FilesConfig,
    /// Shortcut configuration
    #[serde(default)]
    pub shortcuts: ShortcutConfig,
    /// Registry entries to create
    #[serde(default)]
    pub registry: Vec<RegistryEntry>,
    /// Uninstaller settings
    #[serde(default)]
    pub uninstall: UninstallConfig,
    /// UI/theme settings
    #[serde(default)]
    pub ui: UiConfig,
    /// Custom pages (advanced)
    #[serde(default)]
    pub pages: Vec<PageConfig>,
    /// Pre/post install scripts or commands
    #[serde(default)]
    pub scripts: ScriptsConfig,
    /// Environment variables to set
    #[serde(default)]
    pub env_vars: Vec<EnvVarEntry>,
    /// Windows services to install
    #[serde(default)]
    pub services: Vec<ServiceEntry>,
    /// File type associations
    #[serde(default)]
    pub file_associations: Vec<FileAssociationEntry>,
}

/// Application metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Display name of the application
    pub name: String,
    /// Application version (semver)
    pub version: String,
    /// Publisher / company name
    #[serde(default)]
    pub publisher: String,
    /// Application icon path (relative to project root)
    #[serde(default)]
    pub icon: Option<PathBuf>,
    /// Application website URL
    #[serde(default)]
    pub url: Option<String>,
    /// Unique application identifier (e.g., com.company.appname)
    #[serde(default)]
    pub id: Option<String>,
    /// License agreement file path
    #[serde(default)]
    pub license: Option<PathBuf>,
    /// Application description / comment
    #[serde(default)]
    pub description: Option<String>,
}

/// Installation directory and behavior settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallConfig {
    /// Default installation directory (supports path variables)
    #[serde(default = "default_install_dir")]
    pub default_dir: String,
    /// Start menu folder name
    #[serde(default)]
    pub start_menu: Option<String>,
    /// Target architecture: "x64", "x86", or "arm64"
    #[serde(default = "default_arch")]
    pub arch: String,
    /// Whether to allow the user to change the install directory
    #[serde(default = "default_true")]
    pub allow_dir_change: bool,
    /// Whether installation requires admin privileges
    #[serde(default)]
    pub require_admin: bool,
    /// Whether to close the application before installing (if running)
    #[serde(default)]
    pub close_app_before_install: bool,
    /// Executable to launch after installation
    #[serde(default)]
    pub run_after_install: Option<String>,
}

impl Default for InstallConfig {
    fn default() -> Self {
        Self {
            default_dir: default_install_dir(),
            start_menu: None,
            arch: default_arch(),
            allow_dir_change: true,
            require_admin: false,
            close_app_before_install: false,
            run_after_install: None,
        }
    }
}

/// File inclusion configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesConfig {
    /// Glob pattern(s) for source files to include
    #[serde(default)]
    pub source: Vec<String>,
    /// Base directory for source file resolution
    #[serde(default)]
    pub base_dir: Option<PathBuf>,
    /// Explicit file mappings (source -> destination)
    #[serde(default)]
    pub mappings: Vec<FileMapping>,
    /// Files/directories to exclude
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl Default for FilesConfig {
    fn default() -> Self {
        Self {
            source: Vec::new(),
            base_dir: None,
            mappings: Vec::new(),
            exclude: Vec::new(),
        }
    }
}

/// Explicit source-to-destination file mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMapping {
    /// Source file or directory path
    pub source: PathBuf,
    /// Destination path relative to install dir (supports variables)
    pub dest: String,
}

/// Shortcut creation settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutConfig {
    /// Create a desktop shortcut
    #[serde(default)]
    pub desktop: bool,
    /// Create Start Menu shortcuts
    #[serde(default)]
    pub start_menu: bool,
    /// Create a Quick Launch shortcut
    #[serde(default)]
    pub quick_launch: bool,
    /// Additional custom shortcuts
    #[serde(default)]
    pub custom: Vec<CustomShortcut>,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            desktop: false,
            start_menu: false,
            quick_launch: false,
            custom: Vec::new(),
        }
    }
}

/// A custom shortcut definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomShortcut {
    /// Shortcut name
    pub name: String,
    /// Target executable (relative to install dir)
    pub target: String,
    /// Working directory
    #[serde(default)]
    pub working_dir: Option<String>,
    /// Command-line arguments
    #[serde(default)]
    pub arguments: Option<String>,
    /// Icon file path
    #[serde(default)]
    pub icon: Option<String>,
    /// Location: "desktop", "start_menu", or custom path
    #[serde(default = "default_shortcut_location")]
    pub location: String,
}

/// Registry entry to create during installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Registry key path (e.g., "HKLM\\Software\\MyApp")
    pub key: String,
    /// Value name
    #[serde(default)]
    pub name: Option<String>,
    /// Value data
    pub value: String,
    /// Value type: "string", "dword", "expand_string", "multi_string", "binary"
    #[serde(default = "default_reg_type")]
    pub value_type: String,
    /// Registry root: "HKLM", "HKCU", "HKCR", "HKU"
    #[serde(default = "default_reg_root")]
    pub root: String,
    /// Whether to delete this key on uninstall
    #[serde(default = "default_true")]
    pub delete_on_uninstall: bool,
}

/// Uninstaller configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UninstallConfig {
    /// Add entry to Add/Remove Programs
    #[serde(default = "default_true")]
    pub add_remove: bool,
    /// Uninstaller display icon
    #[serde(default)]
    pub icon: Option<PathBuf>,
    /// Custom uninstaller name in Add/Remove Programs
    #[serde(default)]
    pub display_name: Option<String>,
    /// URL for help/support
    #[serde(default)]
    pub help_url: Option<String>,
    /// URL for updates
    #[serde(default)]
    pub update_url: Option<String>,
}

impl Default for UninstallConfig {
    fn default() -> Self {
        Self {
            add_remove: true,
            icon: None,
            display_name: None,
            help_url: None,
            update_url: None,
        }
    }
}

/// UI and theme configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Theme: "modern" or "classic"
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Modern theme color scheme (CSS hex color)
    #[serde(default = "default_accent_color")]
    pub accent_color: String,
    /// Background image for the wizard
    #[serde(default)]
    pub background: Option<PathBuf>,
    /// Sidebar image for the wizard
    #[serde(default)]
    pub sidebar: Option<PathBuf>,
    /// Welcome page text / message
    #[serde(default)]
    pub welcome_text: Option<String>,
    /// Finish page text / message
    #[serde(default)]
    pub finish_text: Option<String>,
    /// Installer window title override
    #[serde(default)]
    pub window_title: Option<String>,
    /// Window width (modern theme only)
    #[serde(default = "default_window_width")]
    pub window_width: u32,
    /// Window height (modern theme only)
    #[serde(default = "default_window_height")]
    pub window_height: u32,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            accent_color: default_accent_color(),
            background: None,
            sidebar: None,
            welcome_text: None,
            finish_text: None,
            window_title: None,
            window_width: default_window_width(),
            window_height: default_window_height(),
        }
    }
}

/// Custom wizard page definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageConfig {
    /// Page type: "text", "checkbox", "input", "dropdown"
    pub page_type: String,
    /// Page title
    pub title: String,
    /// Page description / subtitle
    #[serde(default)]
    pub description: Option<String>,
    /// Page-specific data (depends on page_type)
    #[serde(default)]
    pub data: HashMap<String, serde_json::Value>,
}

/// Pre/post install script configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptsConfig {
    /// Commands to run before installation
    #[serde(default)]
    pub pre_install: Vec<String>,
    /// Commands to run after installation
    #[serde(default)]
    pub post_install: Vec<String>,
    /// Commands to run before uninstallation
    #[serde(default)]
    pub pre_uninstall: Vec<String>,
    /// Commands to run after uninstallation
    #[serde(default)]
    pub post_uninstall: Vec<String>,
}

impl Default for ScriptsConfig {
    fn default() -> Self {
        Self {
            pre_install: Vec::new(),
            post_install: Vec::new(),
            pre_uninstall: Vec::new(),
            post_uninstall: Vec::new(),
        }
    }
}

/// Environment variable to set during installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarEntry {
    /// Variable name
    pub name: String,
    /// Variable value
    pub value: String,
    /// Scope: "system" or "user"
    #[serde(default = "default_env_scope")]
    pub scope: String,
    /// Whether to append to existing value (e.g., for PATH)
    #[serde(default)]
    pub append: bool,
    /// Whether to remove on uninstall
    #[serde(default = "default_true")]
    pub delete_on_uninstall: bool,
}

/// Windows service to install.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEntry {
    /// Service name (internal)
    pub name: String,
    /// Display name
    pub display_name: String,
    /// Service description
    #[serde(default)]
    pub description: Option<String>,
    /// Path to the service executable (relative to install dir)
    pub binary_path: String,
    /// Start type: "auto", "manual", "disabled", "delayed_auto"
    #[serde(default = "default_start_type")]
    pub start_type: String,
    /// Account to run the service under
    #[serde(default)]
    pub account: Option<String>,
    /// Service dependencies
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Whether to start the service after installation
    #[serde(default = "default_true")]
    pub start_on_install: bool,
    /// Whether to stop and remove on uninstall
    #[serde(default = "default_true")]
    pub remove_on_uninstall: bool,
}

/// File type association.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAssociationEntry {
    /// File extension (e.g., ".myext")
    pub extension: String,
    /// File type description
    pub description: String,
    /// Icon for the file type
    #[serde(default)]
    pub icon: Option<String>,
    /// Executable to open this file type (relative to install dir)
    pub handler: String,
    /// Command-line format string (%1 = file path)
    #[serde(default = "default_open_command")]
    pub open_command: String,
}

// ─── Default value functions ─────────────────────────────────────────────────

fn default_install_dir() -> String {
    "{autopf}/MyApp".to_string()
}

fn default_arch() -> String {
    "x64".to_string()
}

fn default_true() -> bool {
    true
}

fn default_theme() -> String {
    "modern".to_string()
}

fn default_accent_color() -> String {
    "#0078D4".to_string()
}

fn default_window_width() -> u32 {
    640
}

fn default_window_height() -> u32 {
    480
}

fn default_shortcut_location() -> String {
    "start_menu".to_string()
}

fn default_reg_type() -> String {
    "string".to_string()
}

fn default_reg_root() -> String {
    "HKLM".to_string()
}

fn default_env_scope() -> String {
    "user".to_string()
}

fn default_start_type() -> String {
    "auto".to_string()
}

fn default_open_command() -> String {
    "\"%1\"".to_string()
}
