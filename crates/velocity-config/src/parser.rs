use crate::error::{ConfigError, Result};
use crate::manifest::VelocityManifest;
use std::path::Path;
use tracing::debug;

/// Parse a `velocity.toml` file into a `VelocityManifest`.
pub fn parse_manifest(path: &Path) -> Result<VelocityManifest> {
    debug!("Parsing manifest from: {}", path.display());
    let content = std::fs::read_to_string(path)?;
    parse_manifest_str(&content)
}

/// Parse a TOML string into a `VelocityManifest`.
pub fn parse_manifest_str(content: &str) -> Result<VelocityManifest> {
    let manifest: VelocityManifest = toml::from_str(content)?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

/// Serialize a `VelocityManifest` to a TOML string.
pub fn serialize_manifest(manifest: &VelocityManifest) -> Result<String> {
    Ok(toml::to_string_pretty(manifest)?)
}

/// Write a manifest to a file.
pub fn write_manifest(manifest: &VelocityManifest, path: &Path) -> Result<()> {
    let content = serialize_manifest(manifest)?;
    std::fs::write(path, content)?;
    debug!("Wrote manifest to: {}", path.display());
    Ok(())
}

/// Validate a manifest for completeness and correctness.
fn validate_manifest(manifest: &VelocityManifest) -> Result<()> {
    // App name is required
    if manifest.app.name.is_empty() {
        return Err(ConfigError::MissingField {
            field: "app.name".to_string(),
        });
    }

    // App name must not contain path separators or null bytes
    if manifest.app.name.contains(['/', '\\', '\0']) {
        return Err(ConfigError::Validation(format!(
            "App name '{}' must not contain path separators or null bytes",
            manifest.app.name
        )));
    }

    // Version is required
    if manifest.app.version.is_empty() {
        return Err(ConfigError::MissingField {
            field: "app.version".to_string(),
        });
    }

    // Validate arch
    let valid_archs = ["x64", "x86", "arm64", "any"];
    if !valid_archs.contains(&manifest.install.arch.as_str()) {
        return Err(ConfigError::Validation(format!(
            "Invalid architecture '{}'. Must be one of: {:?}",
            manifest.install.arch, valid_archs
        )));
    }

    // Validate theme
    let valid_themes = ["modern", "classic"];
    if !valid_themes.contains(&manifest.ui.theme.as_str()) {
        return Err(ConfigError::Validation(format!(
            "Invalid theme '{}'. Must be one of: {:?}",
            manifest.ui.theme, valid_themes
        )));
    }

    // Validate registry entries
    for entry in &manifest.registry {
        let valid_roots = ["HKLM", "HKCU", "HKCR", "HKU"];
        if !valid_roots.contains(&entry.root.as_str()) {
            return Err(ConfigError::Validation(format!(
                "Invalid registry root '{}'. Must be one of: {:?}",
                entry.root, valid_roots
            )));
        }
        let valid_types = ["string", "dword", "expand_string", "multi_string", "binary"];
        if !valid_types.contains(&entry.value_type.as_str()) {
            return Err(ConfigError::Validation(format!(
                "Invalid registry value type '{}'. Must be one of: {:?}",
                entry.value_type, valid_types
            )));
        }
    }

    // Validate service entries
    for svc in &manifest.services {
        let valid_start_types = ["auto", "manual", "disabled", "delayed_auto"];
        if !valid_start_types.contains(&svc.start_type.as_str()) {
            return Err(ConfigError::Validation(format!(
                "Invalid service start type '{}'. Must be one of: {:?}",
                svc.start_type, valid_start_types
            )));
        }
        // Service name must be safe (alphanumeric, underscore, hyphen, dot)
        if svc.name.is_empty()
            || !svc
                .name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Err(ConfigError::Validation(format!(
                "Invalid service name '{}'. Must contain only alphanumeric characters, underscores, hyphens, or dots.",
                svc.name
            )));
        }
    }

    // Validate env var scopes
    for env in &manifest.env_vars {
        let valid_scopes = ["system", "user"];
        if !valid_scopes.contains(&env.scope.as_str()) {
            return Err(ConfigError::Validation(format!(
                "Invalid env var scope '{}'. Must be one of: {:?}",
                env.scope, valid_scopes
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_manifest() {
        let toml = r#"
[app]
name = "Test App"
version = "1.0.0"
"#;
        let manifest = parse_manifest_str(toml).unwrap();
        assert_eq!(manifest.app.name, "Test App");
        assert_eq!(manifest.app.version, "1.0.0");
        assert_eq!(manifest.install.arch, "x64");
        assert_eq!(manifest.ui.theme, "modern");
    }

    #[test]
    fn test_parse_full_manifest() {
        let toml = r##"
[app]
name = "My Application"
version = "2.1.0"
publisher = "Acme Corp"
icon = "assets/icon.ico"

[install]
default_dir = "{autopf}/MyApp"
start_menu = "My Application"
arch = "x64"
require_admin = true

[files]
source = ["./build/**"]
exclude = ["*.pdb", "*.tmp"]

[shortcuts]
desktop = true
start_menu = true

[[registry]]
key = "Software\\MyApp"
name = "Version"
value = "2.1.0"
root = "HKLM"

[uninstall]
add_remove = true

[ui]
theme = "modern"
accent_color = "#FF5722"
"##;
        let manifest = parse_manifest_str(toml).unwrap();
        assert_eq!(manifest.app.publisher, "Acme Corp");
        assert!(manifest.install.require_admin);
        assert!(manifest.shortcuts.desktop);
        assert_eq!(manifest.registry.len(), 1);
        assert_eq!(manifest.ui.accent_color, "#FF5722");
    }

    #[test]
    fn test_missing_app_name() {
        let toml = r#"
[app]
name = ""
version = "1.0.0"
"#;
        let result = parse_manifest_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_arch() {
        let toml = r#"
[app]
name = "Test"
version = "1.0.0"

[install]
arch = "mips"
"#;
        let result = parse_manifest_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_serialize_roundtrip() {
        let toml = r#"
[app]
name = "Roundtrip Test"
version = "3.0.0"
publisher = "Test Corp"

[install]
default_dir = "{autopf}/Roundtrip"
arch = "x64"
require_admin = true

[shortcuts]
desktop = true
start_menu = true

[uninstall]
add_remove = true
"#;
        let manifest = parse_manifest_str(toml).unwrap();
        let serialized = serialize_manifest(&manifest).unwrap();
        let reparsed = parse_manifest_str(&serialized).unwrap();

        assert_eq!(manifest.app.name, reparsed.app.name);
        assert_eq!(manifest.app.version, reparsed.app.version);
        assert_eq!(manifest.app.publisher, reparsed.app.publisher);
        assert_eq!(
            manifest.install.require_admin,
            reparsed.install.require_admin
        );
        assert_eq!(manifest.shortcuts.desktop, reparsed.shortcuts.desktop);
    }

    #[test]
    fn test_env_vars_parsing() {
        let toml = r#"
[app]
name = "Env Test"
version = "1.0.0"

[[env_vars]]
name = "MY_APP_HOME"
value = "{app}"
scope = "system"

[[env_vars]]
name = "MY_APP_DATA"
value = "{home}/.myapp"
scope = "user"
append = false
"#;
        let manifest = parse_manifest_str(toml).unwrap();
        assert_eq!(manifest.env_vars.len(), 2);
        assert_eq!(manifest.env_vars[0].name, "MY_APP_HOME");
        assert_eq!(manifest.env_vars[0].scope, "system");
        assert_eq!(manifest.env_vars[1].scope, "user");
    }

    #[test]
    fn test_services_parsing() {
        let toml = r#"
[app]
name = "Service Test"
version = "1.0.0"

[[services]]
name = "MyService"
display_name = "My Background Service"
binary_path = "myservice.exe"
start_type = "auto"
start_on_install = true
"#;
        let manifest = parse_manifest_str(toml).unwrap();
        assert_eq!(manifest.services.len(), 1);
        assert_eq!(manifest.services[0].name, "MyService");
        assert_eq!(manifest.services[0].start_type, "auto");
    }

    #[test]
    fn test_file_associations_parsing() {
        let toml = r#"
[app]
name = "Assoc Test"
version = "1.0.0"

[[file_associations]]
extension = ".myext"
description = "My Custom File"
handler = "myapp.exe"
"#;
        let manifest = parse_manifest_str(toml).unwrap();
        assert_eq!(manifest.file_associations.len(), 1);
        assert_eq!(manifest.file_associations[0].extension, ".myext");
        assert_eq!(manifest.file_associations[0].handler, "myapp.exe");
    }

    #[test]
    fn test_invalid_registry_root() {
        let toml = r#"
[app]
name = "Test"
version = "1.0.0"

[[registry]]
key = "Software\\Test"
value = "test"
root = "HKXX"
"#;
        let result = parse_manifest_str(toml);
        assert!(result.is_err());
    }

    #[test]
    fn test_default_values() {
        let toml = r#"
[app]
name = "Defaults Test"
version = "1.0.0"
"#;
        let manifest = parse_manifest_str(toml).unwrap();
        assert_eq!(manifest.install.default_dir, "{autopf}/MyApp");
        assert_eq!(manifest.install.arch, "x64");
        assert!(manifest.install.allow_dir_change);
        assert!(!manifest.install.require_admin);
        assert_eq!(manifest.ui.theme, "modern");
        assert_eq!(manifest.ui.accent_color, "#0078D4");
        assert!(manifest.uninstall.add_remove);
        assert!(manifest.scripts.post_install.is_empty());
        assert!(manifest.registry.is_empty());
        assert!(manifest.env_vars.is_empty());
        assert!(manifest.services.is_empty());
        assert!(manifest.file_associations.is_empty());
    }

    // ================================================================
    // Fuzz-like robustness tests
    //
    // These tests verify that parse_manifest_str NEVER panics on any
    // input — it always returns Ok or Err gracefully. This is the
    // key invariant for parser safety.
    // ================================================================

    /// Helper: assert that parsing returns Err (never panics).
    fn assert_parse_err(input: &str) {
        let result = parse_manifest_str(input);
        assert!(result.is_err(), "Expected Err for input, got Ok");
    }

    /// Helper: the input may or may not parse, but must not panic.
    fn assert_no_panic(input: &str) {
        let _ = parse_manifest_str(input);
    }

    #[test]
    fn fuzz_empty_and_whitespace() {
        assert_parse_err("");
        assert_parse_err("   ");
        assert_parse_err("\n\n\n");
        assert_parse_err("\t\t\t");
        assert_parse_err("\r\n\r\n");
    }

    #[test]
    fn fuzz_garbage_input() {
        assert_no_panic("not toml at all");
        assert_no_panic("{{{{{{{{{{{{");
        assert_no_panic("}}}}}}}}}}}}");
        assert_no_panic("[][][][][][]");
        assert_no_panic("=====");
        assert_no_panic("~~~~~");
        assert_no_panic("```````");
    }

    #[test]
    fn fuzz_truncated_toml() {
        // Valid TOML prefix truncated at various points
        assert_parse_err("[app");
        assert_parse_err("[app]\nname");
        assert_parse_err("[app]\nname =");
        assert_parse_err("[app]\nname = \"");
        assert_parse_err("[app]\nname = \"Test");
        assert_parse_err("[app]\nname = \"Test\"");
        // Missing version
        assert_parse_err("[app]\nname = \"Test\"");
    }

    #[test]
    fn fuzz_invalid_toml_syntax() {
        assert_parse_err("[app]\nname = \"Test\"\nversion = 1.0.0"); // unquoted version
        assert_parse_err("[app]\nname = Test"); // unquoted string
        assert_parse_err("[app]\nname = [1, 2, 3]"); // array instead of string
        assert_parse_err("[app]\nname = {{}}"); // inline table instead of string
        assert_parse_err("[app\nname = \"Test\""); // missing closing bracket
        assert_parse_err("app]\nname = \"Test\""); // missing opening bracket
    }

    #[test]
    fn fuzz_unicode_chaos() {
        // Various Unicode edge cases
        assert_no_panic("日本語テスト");
        assert_no_panic("[app]\nname = \"中文应用\"\nversion = \"1.0.0\"");
        assert_no_panic("[app]\nname = \"🔥🚀💥\"\nversion = \"1.0.0\"");
        assert_no_panic("\u{0000}\u{0001}\u{0002}");
        assert_no_panic("\u{FEFF}[app]"); // BOM
        assert_no_panic("name = \"\u{202E}rtl\u{202C}\""); // RTL override
        assert_no_panic(&"A".repeat(100_000)); // Very long string
    }

    #[test]
    fn fuzz_deeply_nested_tables() {
        // Deeply nested tables shouldn't cause stack overflow
        let mut input = String::new();
        for i in 0..50 {
            input.push_str(&format!("[level{}]\n", i));
        }
        assert_no_panic(&input);
    }

    #[test]
    fn fuzz_duplicate_keys() {
        assert_no_panic("[app]\nname = \"A\"\nname = \"B\"\nversion = \"1.0\"");
    }

    #[test]
    fn fuzz_type_mismatches() {
        // Fields with wrong types
        assert_parse_err("[app]\nname = 42\nversion = \"1.0\"");
        assert_parse_err("[app]\nname = \"Test\"\nversion = true");
        assert_parse_err("[install]\nrequire_admin = \"yes\"");
        assert_parse_err("[install]\ndefault_dir = [1, 2, 3]");
    }

    #[test]
    fn fuzz_unknown_sections() {
        // Unknown sections should be ignored by serde (or cause error, but not panic)
        assert_no_panic(
            "[app]\nname = \"Test\"\nversion = \"1.0\"\n\n[unknown_section]\nfoo = \"bar\"",
        );
    }

    #[test]
    fn fuzz_special_characters_in_strings() {
        assert_no_panic("[app]\nname = \"Test\\nWith\\nNewlines\"\nversion = \"1.0\"");
        assert_no_panic("[app]\nname = \"Test\\tWith\\tTabs\"\nversion = \"1.0\"");
        assert_no_panic("[app]\nname = \"Test\\\\With\\\\Backslashes\"\nversion = \"1.0\"");
        assert_no_panic("[app]\nname = \"Test\\\"With\\\"Quotes\"\nversion = \"1.0\"");
    }

    #[test]
    fn fuzz_massive_array() {
        // Large arrays in source field
        let sources: Vec<String> = (0..1000).map(|i| format!("\"file_{}.txt\"", i)).collect();
        let input = format!(
            "[app]\nname = \"Test\"\nversion = \"1.0\"\n\n[files]\nsource = [{}]",
            sources.join(", ")
        );
        assert_no_panic(&input);
    }

    #[test]
    fn fuzz_many_registry_entries() {
        let mut input = String::from("[app]\nname = \"Test\"\nversion = \"1.0\"\n");
        for i in 0..100 {
            input.push_str(&format!(
                "\n[[registry]]\nkey = \"Software\\\\Test{}\"\nroot = \"HKCU\"\nname = \"key{}\"\nvalue = \"val{}\"\n",
                i, i, i
            ));
        }
        let result = parse_manifest_str(&input);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().registry.len(), 100);
    }
}
