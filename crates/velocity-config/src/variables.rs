use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Resolves path variables like `{app}`, `{autopf}`, `{win}`, etc.
///
/// Supported variables:
/// - `{app}` — The application installation directory
/// - `{autopf}` — Program Files directory (auto-detects x86/x64)
/// - `{autopf64}` — 64-bit Program Files directory
/// - `{autopf32}` — 32-bit Program Files directory
/// - `{commonstartup}` — All Users Startup folder
/// - `{autodesktop}` — Common Desktop path
/// - `{autostartmenu}` — Common Start Menu path
/// - `{autoprograms}` — Common Programs folder
/// - `{win}` — Windows directory
/// - `{sys}` — System32 directory
/// - `{tmp}` — Temporary directory
/// - `{src}` — Source directory of the installer
/// - `{home}` — Current user's home directory
/// - `{group}` — Start Menu group folder
pub struct VariableResolver {
    /// The resolved install directory
    install_dir: PathBuf,
    /// Additional custom variables
    custom_vars: HashMap<String, String>,
}

impl VariableResolver {
    /// Create a new resolver with the given install directory.
    pub fn new(install_dir: &Path) -> Self {
        Self {
            install_dir: install_dir.to_path_buf(),
            custom_vars: HashMap::new(),
        }
    }

    /// Add a custom variable.
    pub fn set_variable(&mut self, name: &str, value: &str) {
        self.custom_vars.insert(name.to_string(), value.to_string());
    }

    /// Resolve all variables in a string.
    pub fn resolve(&self, input: &str) -> String {
        let mut result = input.to_string();

        // Built-in variables
        let builtins = self.get_builtin_variables();

        // Apply custom variables first (they take priority)
        for (key, value) in &self.custom_vars {
            result = result.replace(&format!("{{{}}}", key), value);
        }

        // Apply built-in variables
        for (key, value) in &builtins {
            result = result.replace(&format!("{{{}}}", key), value);
        }

        result
    }

    /// Resolve variables in a path string, returning a PathBuf.
    pub fn resolve_path(&self, input: &str) -> PathBuf {
        PathBuf::from(self.resolve(input))
    }

    /// Get all built-in variable values.
    fn get_builtin_variables(&self) -> HashMap<String, String> {
        let mut vars = HashMap::new();

        // {app} — install directory
        vars.insert(
            "app".to_string(),
            self.install_dir.to_string_lossy().to_string(),
        );

        // Platform-specific paths
        #[cfg(target_os = "windows")]
        {
            // {autopf} — Program Files (auto-detect architecture)
            if let Some(pf) = std::env::var("ProgramFiles")
                .ok()
                .or_else(|| Some("C:\\Program Files".to_string()))
            {
                vars.insert("autopf".to_string(), pf);
            }

            // {autopf64} — 64-bit Program Files
            if let Ok(pf64) = std::env::var("ProgramW6432") {
                vars.insert("autopf64".to_string(), pf64);
            } else {
                vars.insert("autopf64".to_string(), "C:\\Program Files".to_string());
            }

            // {autopf32} — 32-bit Program Files
            if let Ok(pf32) = std::env::var("ProgramFiles(x86)") {
                vars.insert("autopf32".to_string(), pf32);
            } else {
                vars.insert(
                    "autopf32".to_string(),
                    "C:\\Program Files (x86)".to_string(),
                );
            }

            // {win} — Windows directory
            if let Ok(win) = std::env::var("WINDIR") {
                vars.insert("win".to_string(), win);
            } else {
                vars.insert("win".to_string(), "C:\\Windows".to_string());
            }

            // {sys} — System32
            if let Some(win) = vars.get("win") {
                vars.insert("sys".to_string(), format!("{}\\System32", win));
            }

            // {home} — User home (Windows)
            if let Ok(home) = std::env::var("USERPROFILE") {
                vars.insert("home".to_string(), home);
            }

            // {autodesktop} — Common Desktop
            if let Ok(desktop) = std::env::var("PUBLIC") {
                vars.insert("autodesktop".to_string(), format!("{}\\Desktop", desktop));
            }

            // {autostartmenu} — Common Start Menu
            if let Ok(program_data) = std::env::var("ProgramData") {
                vars.insert(
                    "autostartmenu".to_string(),
                    format!("{}\\Microsoft\\Windows\\Start Menu", program_data),
                );
                vars.insert(
                    "autoprograms".to_string(),
                    format!("{}\\Microsoft\\Windows\\Start Menu\\Programs", program_data),
                );
            }

            // {commonstartup} — Common Startup folder
            if let Some(programs) = vars.get("autoprograms") {
                vars.insert(
                    "commonstartup".to_string(),
                    format!("{}\\Startup", programs),
                );
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            // {autopf} — /usr/local (Unix convention)
            vars.insert("autopf".to_string(), "/usr/local".to_string());
            vars.insert("autopf64".to_string(), "/usr/local".to_string());
            vars.insert("autopf32".to_string(), "/usr/local".to_string());

            // {home} — User home directory
            if let Ok(home) = std::env::var("HOME") {
                vars.insert("home".to_string(), home);
            }

            // {autodesktop} — Desktop directory
            if let Ok(home) = std::env::var("HOME") {
                vars.insert("autodesktop".to_string(), format!("{}/Desktop", home));
            }

            // {autostartmenu} — Applications menu (XDG)
            if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
                vars.insert(
                    "autostartmenu".to_string(),
                    format!("{}/applications", data_home),
                );
            } else if let Ok(home) = std::env::var("HOME") {
                vars.insert(
                    "autostartmenu".to_string(),
                    format!("{}/.local/share/applications", home),
                );
            }

            // {autoprograms} — Same as start menu on Unix
            if let Some(menu) = vars.get("autostartmenu").cloned() {
                vars.insert("autoprograms".to_string(), menu);
            }
        }

        // {tmp} — Temp directory (cross-platform)
        vars.insert(
            "tmp".to_string(),
            std::env::temp_dir().to_string_lossy().to_string(),
        );

        vars
    }
}

/// Extract all variable names from a string (e.g., "{app}" -> "app").
pub fn extract_variables(input: &str) -> Vec<String> {
    let mut vars = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            let mut var_name = String::new();
            for inner in chars.by_ref() {
                if inner == '}' {
                    if !var_name.is_empty() {
                        vars.push(var_name);
                    }
                    break;
                }
                var_name.push(inner);
            }
        }
    }

    vars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_variables() {
        let input = "{autopf}/MyApp/{version}";
        let vars = extract_variables(input);
        assert_eq!(vars, vec!["autopf", "version"]);
    }

    #[test]
    fn test_resolve_custom_variable() {
        let resolver = VariableResolver::new(Path::new("C:\\MyApp"));
        let mut resolver = resolver;
        resolver.set_variable("version", "2.0");
        let result = resolver.resolve("Version is {version}");
        assert_eq!(result, "Version is 2.0");
    }

    #[test]
    fn test_resolve_app_variable() {
        let resolver = VariableResolver::new(Path::new("C:\\Program Files\\MyApp"));
        let result = resolver.resolve("{app}\\bin\\app.exe");
        assert_eq!(result, "C:\\Program Files\\MyApp\\bin\\app.exe");
    }

    #[test]
    fn test_resolve_tmp_variable() {
        let resolver = VariableResolver::new(Path::new("/opt/myapp"));
        let result = resolver.resolve("{tmp}/installer.log");
        // {tmp} should resolve to the system temp directory
        assert!(result.contains("installer.log"));
        assert!(!result.contains("{tmp}"));
    }

    #[test]
    fn test_resolve_multiple_variables() {
        let resolver = VariableResolver::new(Path::new("/opt/myapp"));
        let mut resolver = resolver;
        resolver.set_variable("version", "1.0");
        let result = resolver.resolve("{app}/bin/app-{version}");
        assert_eq!(result, "/opt/myapp/bin/app-1.0");
    }

    #[test]
    fn test_resolve_unknown_variable() {
        let resolver = VariableResolver::new(Path::new("/opt/myapp"));
        let result = resolver.resolve("{unknown}/path");
        // Unknown variables should remain unresolved
        assert_eq!(result, "{unknown}/path");
    }

    #[test]
    fn test_extract_variables_empty() {
        let vars = extract_variables("no variables here");
        assert!(vars.is_empty());
    }

    #[test]
    fn test_extract_variables_nested_braces() {
        let vars = extract_variables("{a}{b}{c}");
        assert_eq!(vars, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_extract_variables_unclosed_brace() {
        let vars = extract_variables("{unclosed");
        assert!(vars.is_empty());
    }

    #[test]
    fn test_resolve_path() {
        let resolver = VariableResolver::new(Path::new("/opt/myapp"));
        let path = resolver.resolve_path("{app}/bin");
        assert_eq!(path, PathBuf::from("/opt/myapp/bin"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_windows_autopf() {
        let resolver = VariableResolver::new(Path::new("C:\\MyApp"));
        let result = resolver.resolve("{autopf}");
        // Should resolve to a Program Files path on Windows
        assert!(result.contains("Program Files") || result.contains("C:\\"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_unix_autopf() {
        let resolver = VariableResolver::new(Path::new("/opt/myapp"));
        let result = resolver.resolve("{autopf}");
        assert_eq!(result, "/usr/local");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_unix_home() {
        // Set HOME for this test
        std::env::set_var("HOME", "/home/testuser");
        let resolver = VariableResolver::new(Path::new("/opt/myapp"));
        let result = resolver.resolve("{home}");
        assert_eq!(result, "/home/testuser");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_unix_desktop() {
        std::env::set_var("HOME", "/home/testuser");
        let resolver = VariableResolver::new(Path::new("/opt/myapp"));
        let result = resolver.resolve("{autodesktop}");
        assert_eq!(result, "/home/testuser/Desktop");
    }
}
