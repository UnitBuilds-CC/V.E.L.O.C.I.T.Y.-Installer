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
    /// Remote dependencies to download and install (e.g., VC++ Redist, DirectX)
    #[serde(default)]
    pub dependencies: Vec<DependencyEntry>,
    /// Third-party applications bundled with the installer
    #[serde(default)]
    pub bundled_apps: Vec<BundledAppEntry>,
    /// Installable components (user-selectable features)
    #[serde(default)]
    pub components: Vec<Component>,
    /// Localization / internationalization settings
    #[serde(default)]
    pub localization: LocalizationConfig,
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
    /// Overwrite mode for existing files: "always", "never", "prompt", "newer_only"
    #[serde(default = "default_overwrite_mode")]
    pub overwrite_mode: String,
    /// Reboot handling: "auto" (reboot if needed), "ask" (prompt user), "never"
    #[serde(default = "default_reboot_handling")]
    pub reboot_handling: String,
    /// Whether this is a 64-bit install (overrides arch detection)
    #[serde(default)]
    pub install_64bit: Option<bool>,
    /// Whether to show the component selection page
    #[serde(default)]
    pub show_components: bool,
    /// Whether to show the language selection page
    #[serde(default)]
    pub show_language: bool,
    /// Minimum disk space required (bytes, 0 = auto-calculate)
    #[serde(default)]
    pub min_disk_space: u64,
    /// Whether to create a desktop shortcut
    #[serde(default)]
    pub create_desktop_shortcut: bool,
    /// Whether to verify file checksums after extraction
    #[serde(default)]
    pub verify_checksums: bool,
    /// Hash algorithm for checksum verification: "sha256" or "sha512"
    #[serde(default = "default_checksum_algo")]
    pub checksum_algo: String,
    /// Password for encrypted installers (empty = no encryption)
    #[serde(default)]
    pub password: String,
    /// Installation types (e.g., "full", "compact", "custom")
    #[serde(default)]
    pub types: Vec<InstallType>,
}

fn default_checksum_algo() -> String {
    "sha256".to_string()
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
            overwrite_mode: default_overwrite_mode(),
            reboot_handling: default_reboot_handling(),
            install_64bit: None,
            show_components: false,
            show_language: false,
            min_disk_space: 0,
            create_desktop_shortcut: false,
            verify_checksums: false,
            checksum_algo: default_checksum_algo(),
            password: String::new(),
            types: vec![],
        }
    }
}

/// File inclusion configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
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
#[derive(Default)]
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
#[derive(Default)]
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

/// Remote dependency to download and install.
///
/// Supports downloading installers from URLs and running them silently.
/// Common uses: VC++ Redistributables, DirectX, .NET Framework, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEntry {
    /// Human-readable name (e.g., "VC++ 2015-2022 Redistributable")
    pub name: String,
    /// Download URL (https)
    pub url: String,
    /// Expected SHA256 hash for integrity verification
    #[serde(default)]
    pub sha256: Option<String>,
    /// Command-line arguments for silent installation
    #[serde(default)]
    pub install_args: String,
    /// Condition expression — only install if this evaluates to true.
    ///
    /// Supported conditions:
    /// - `"always"` — always install
    /// - `"registry_missing:HKLM\\Software\\..."` — install if registry key is absent
    /// - `"registry_exists:HKLM\\Software\\..."` — install if registry key exists
    /// - `"file_missing:C:\\path\\to\\file.dll"` — install if file doesn't exist
    /// - `"file_exists:C:\\path\\to\\file.dll"` — install if file exists
    /// - `"not_installed:ProductName"` — install if not in Add/Remove Programs
    #[serde(default = "default_dep_condition")]
    pub condition: String,
    /// Installation order priority (lower = earlier). Default 100.
    #[serde(default = "default_dep_priority")]
    pub priority: u32,
    /// Whether this dependency is required (fails install if it fails)
    #[serde(default = "default_true")]
    pub required: bool,
    /// File type hint: "exe", "msi", "msm" — determines how to invoke it
    #[serde(default = "default_dep_type")]
    pub file_type: String,
}

/// Third-party application bundled with the installer.
///
/// The installer file is included in the payload and executed during install.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundledAppEntry {
    /// Human-readable name (e.g., "7-Zip")
    pub name: String,
    /// Path to the installer file relative to the project root.
    /// This file will be included in the payload automatically.
    pub installer: String,
    /// Command-line arguments for silent installation
    #[serde(default)]
    pub install_args: String,
    /// Condition expression (same format as DependencyEntry)
    #[serde(default = "default_dep_condition")]
    pub condition: String,
    /// Installation order priority (lower = earlier). Default 200.
    #[serde(default = "default_bundled_priority")]
    pub priority: u32,
    /// Whether this bundled app is required
    #[serde(default)]
    pub required: bool,
    /// Working directory for the installer (default: temp dir)
    #[serde(default)]
    pub working_dir: Option<String>,
}

/// Installable component for user-selectable feature installation.
///
/// Components allow users to choose which parts of the application to install.
/// Each component can have its own files, registry entries, shortcuts, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    /// Unique component identifier (e.g., "core", "docs", "sdk")
    pub id: String,
    /// Display name shown to the user
    pub name: String,
    /// Description shown in the component selection UI
    #[serde(default)]
    pub description: Option<String>,
    /// Whether this component is selected by default
    #[serde(default = "default_true")]
    pub selected_by_default: bool,
    /// Whether this component is mandatory (cannot be deselected)
    #[serde(default)]
    pub mandatory: bool,
    /// Disk space required by this component (bytes, estimated)
    #[serde(default)]
    pub size: u64,
    /// Group name for organizing components in the UI
    #[serde(default)]
    pub group: Option<String>,
    /// File patterns specific to this component (relative to base source)
    #[serde(default)]
    pub files: Vec<String>,
    /// Subdirectory within the install dir for this component's files
    #[serde(default)]
    pub install_subdir: Option<String>,
    /// Registry entries specific to this component
    #[serde(default)]
    pub registry: Vec<RegistryEntry>,
    /// Shortcuts specific to this component
    #[serde(default)]
    pub shortcuts: Vec<CustomShortcut>,
    /// Child components (for tree-based selection UI)
    #[serde(default)]
    pub children: Vec<Component>,
    /// Dependencies on other component IDs (must be installed if those are)
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// Installation type definition (e.g., "Full", "Compact", "Custom").
///
/// Types group components into predefined installation profiles.
/// For example, a "Full" type might include all components, while "Compact"
/// only includes the mandatory ones.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallType {
    /// Type identifier (e.g., "full", "compact", "custom")
    pub id: String,
    /// Display name shown to the user (e.g., "Full Installation")
    pub name: String,
    /// Description shown in the UI
    #[serde(default)]
    pub description: Option<String>,
    /// Component IDs included in this type
    #[serde(default)]
    pub components: Vec<String>,
    /// Whether this is the default type
    #[serde(default)]
    pub is_default: bool,
}

/// Localization configuration for multi-language installer UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalizationConfig {
    /// Default language code (e.g., "en", "de", "fr", "ja")
    #[serde(default = "default_language")]
    pub default_language: String,
    /// Available languages with their display names
    #[serde(default)]
    pub languages: Vec<LanguageEntry>,
    /// Custom string overrides for the default language
    #[serde(default)]
    pub strings: HashMap<String, String>,
}

impl Default for LocalizationConfig {
    fn default() -> Self {
        Self {
            default_language: default_language(),
            languages: Vec::new(),
            strings: HashMap::new(),
        }
    }
}

/// A language entry for multi-language support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageEntry {
    /// Language code (e.g., "en", "de", "fr")
    pub code: String,
    /// Display name (e.g., "English", "Deutsch", "Francais")
    pub name: String,
    /// Localized UI strings for this language
    #[serde(default)]
    pub strings: HashMap<String, String>,
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

fn default_dep_condition() -> String {
    "always".to_string()
}

fn default_dep_priority() -> u32 {
    100
}

fn default_bundled_priority() -> u32 {
    200
}

fn default_dep_type() -> String {
    "exe".to_string()
}

fn default_language() -> String {
    "en".to_string()
}

fn default_overwrite_mode() -> String {
    "always".to_string()
}

fn default_reboot_handling() -> String {
    "ask".to_string()
}
