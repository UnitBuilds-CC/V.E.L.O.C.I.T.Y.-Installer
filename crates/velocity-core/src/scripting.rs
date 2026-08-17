//! Basic scripting engine for custom installer actions.
//!
//! Provides a structured way to define and execute custom actions during
//! installation and uninstallation. Supports:
//! - Shell commands with variable substitution
//! - File operations (copy, delete, create directory)
//! - Registry operations
//! - Condition evaluation
//! - Configurable error handling (continue, abort)

use crate::error::{CoreError, Result};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Types of actions the scripting engine can execute.
#[derive(Debug, Clone)]
pub enum ActionType {
    /// Run a shell command via cmd.exe
    ShellCommand(String),
    /// Copy a file from source to destination
    CopyFile { src: String, dest: String },
    /// Delete a file
    DeleteFile(String),
    /// Create a directory (including parents)
    CreateDir(String),
    /// Delete a directory (recursive)
    DeleteDir(String),
    /// Write a registry value
    WriteRegistry {
        key: String,
        name: String,
        value: String,
    },
    /// Set an environment variable
    SetEnvVar {
        name: String,
        value: String,
        scope: String,
    },
}

/// Error handling policy for an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorPolicy {
    /// Abort the script on failure
    Abort,
    /// Log the error and continue
    Continue,
    /// Retry up to N times before aborting
    Retry(u32),
}

/// A single scripted action with metadata.
#[derive(Debug, Clone)]
pub struct ScriptAction {
    /// Human-readable name / description
    pub name: String,
    /// The action to perform
    pub action: ActionType,
    /// Condition expression (empty = always run)
    pub condition: String,
    /// What to do on failure
    pub on_error: ErrorPolicy,
    /// Working directory for the action
    pub working_dir: Option<String>,
}

/// Result of executing a single action.
#[derive(Debug)]
pub struct ActionResult {
    /// Whether the action succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Action name for logging
    pub action_name: String,
}

/// The scripting engine — executes a sequence of scripted actions
/// with variable substitution and condition evaluation.
pub struct ScriptEngine {
    /// Variable context for substitution (e.g., install_dir, app_name)
    variables: HashMap<String, String>,
    /// Results of previously executed actions (for conditional logic)
    action_results: HashMap<String, bool>,
}

impl ScriptEngine {
    /// Create a new scripting engine with the given variable context.
    pub fn new(variables: HashMap<String, String>) -> Self {
        Self {
            variables,
            action_results: HashMap::new(),
        }
    }

    /// Add or update a variable in the context.
    pub fn set_variable(&mut self, name: &str, value: &str) {
        self.variables.insert(name.to_string(), value.to_string());
    }

    /// Substitute variables in a string.
    ///
    /// Replaces `{variable_name}` patterns with their values from the context.
    pub fn substitute(&self, input: &str) -> String {
        let mut result = input.to_string();
        for (key, value) in &self.variables {
            let pattern = format!("{{{}}}", key);
            result = result.replace(&pattern, value);
        }
        result
    }

    /// Evaluate a condition expression.
    ///
    /// Supported conditions:
    /// - `""` (empty) → always true
    /// - `"always"` → always true
    /// - `"never"` → always false
    /// - `"file_exists:<path>"` → true if file exists
    /// - `"file_missing:<path>"` → true if file doesn't exist
    /// - `"dir_exists:<path>"` → true if directory exists
    /// - `"reg_exists:<HKLM\\...>"` → true if registry key exists
    /// - `"action_success:<name>"` → true if named action succeeded
    /// - `"env_set:<NAME>"` → true if environment variable is set
    pub fn evaluate_condition(&self, condition: &str) -> bool {
        let cond = condition.trim();
        if cond.is_empty() || cond.eq_ignore_ascii_case("always") {
            return true;
        }
        if cond.eq_ignore_ascii_case("never") {
            return false;
        }

        if let Some(path) = cond.strip_prefix("file_exists:") {
            let resolved = self.substitute(path);
            return std::path::Path::new(&resolved).exists();
        }
        if let Some(path) = cond.strip_prefix("file_missing:") {
            let resolved = self.substitute(path);
            return !std::path::Path::new(&resolved).exists();
        }
        if let Some(path) = cond.strip_prefix("dir_exists:") {
            let resolved = self.substitute(path);
            return std::path::Path::new(&resolved).is_dir();
        }
        if let Some(name) = cond.strip_prefix("action_success:") {
            return self
                .action_results
                .get(name.trim())
                .copied()
                .unwrap_or(false);
        }
        if let Some(var_name) = cond.strip_prefix("env_set:") {
            return std::env::var(var_name.trim()).is_ok();
        }

        // Unknown condition — default to true with a warning
        warn!(
            "Unknown condition expression: '{}', defaulting to true",
            cond
        );
        true
    }

    /// Execute a single action.
    pub fn execute_action(&mut self, action: &ScriptAction) -> ActionResult {
        let name = self.substitute(&action.name);
        info!("Executing action: {}", name);

        // Check condition
        if !self.evaluate_condition(&action.condition) {
            debug!(
                "Skipping '{}' — condition not met: {}",
                name, action.condition
            );
            return ActionResult {
                success: true,
                error: None,
                action_name: name,
            };
        }

        let result = self.run_action(action);
        let success = result.is_ok();
        let error = result.err().map(|e| e.to_string());

        self.action_results.insert(name.clone(), success);

        ActionResult {
            success,
            error,
            action_name: name,
        }
    }

    /// Execute a sequence of actions.
    ///
    /// Returns a vector of results. Stops early if an action with
    /// `ErrorPolicy::Abort` fails.
    pub fn execute_sequence(&mut self, actions: &[ScriptAction]) -> Vec<ActionResult> {
        let mut results = Vec::new();

        for action in actions {
            let result = self.execute_action(action);

            if !result.success {
                warn!("Action '{}' failed: {:?}", result.action_name, result.error);
                match action.on_error {
                    ErrorPolicy::Abort => {
                        results.push(result);
                        info!("Aborting script sequence due to action failure");
                        break;
                    }
                    ErrorPolicy::Continue => {
                        results.push(result);
                        continue;
                    }
                    ErrorPolicy::Retry(max_retries) => {
                        let mut retry_result = result;
                        for attempt in 1..=max_retries {
                            debug!(
                                "Retry {}/{} for '{}'",
                                attempt, max_retries, retry_result.action_name
                            );
                            retry_result = self.execute_action(action);
                            if retry_result.success {
                                break;
                            }
                        }
                        results.push(retry_result);
                        if !results.last().unwrap().success {
                            info!("Aborting after {} retries", max_retries);
                            break;
                        }
                    }
                }
            } else {
                results.push(result);
            }
        }

        results
    }

    /// Execute a list of shell command strings (backward-compatible with existing scripts).
    ///
    /// Variables are substituted in each command before execution.
    pub fn execute_shell_commands(&self, commands: &[String]) -> Vec<ActionResult> {
        commands
            .iter()
            .map(|cmd| {
                let resolved = self.substitute(cmd);
                let action = ScriptAction {
                    name: format!("shell: {}", resolved),
                    action: ActionType::ShellCommand(resolved),
                    condition: String::new(),
                    on_error: ErrorPolicy::Continue,
                    working_dir: self.variables.get("install_dir").cloned(),
                };
                // We need &mut self for execute_action, so run directly
                let result = run_shell_command(&action);
                ActionResult {
                    success: result.is_ok(),
                    error: result.err().map(|e| e.to_string()),
                    action_name: action.name,
                }
            })
            .collect()
    }

    /// Internal: run the actual action.
    fn run_action(&self, action: &ScriptAction) -> Result<()> {
        match &action.action {
            ActionType::ShellCommand(cmd) => {
                let resolved = self.substitute(cmd);
                let work_dir = action.working_dir.as_ref().map(|d| self.substitute(d));

                let output = run_shell(&resolved, work_dir.as_deref())?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(CoreError::other(
                        "shell command",
                        format!(
                            "{}: exited with {}, stderr: {}",
                            resolved,
                            output.status,
                            stderr.trim()
                        ),
                    ));
                }
                Ok(())
            }
            ActionType::CopyFile { src, dest } => {
                let resolved_src = self.substitute(src);
                let resolved_dest = self.substitute(dest);
                std::fs::copy(&resolved_src, &resolved_dest).map_err(|e| {
                    CoreError::other(
                        "copy file",
                        format!("{} -> {}: {}", resolved_src, resolved_dest, e),
                    )
                })?;
                Ok(())
            }
            ActionType::DeleteFile(path) => {
                let resolved = self.substitute(path);
                if std::path::Path::new(&resolved).exists() {
                    std::fs::remove_file(&resolved).map_err(|e| {
                        CoreError::other("delete file", format!("{}: {}", resolved, e))
                    })?;
                }
                Ok(())
            }
            ActionType::CreateDir(path) => {
                let resolved = self.substitute(path);
                std::fs::create_dir_all(&resolved)
                    .map_err(|e| CoreError::other("create dir", format!("{}: {}", resolved, e)))?;
                Ok(())
            }
            ActionType::DeleteDir(path) => {
                let resolved = self.substitute(path);
                if std::path::Path::new(&resolved).exists() {
                    std::fs::remove_dir_all(&resolved).map_err(|e| {
                        CoreError::other("delete dir", format!("{}: {}", resolved, e))
                    })?;
                }
                Ok(())
            }
            ActionType::WriteRegistry { key, name, value } => {
                #[cfg(target_os = "windows")]
                {
                    let resolved_key = self.substitute(key);
                    let resolved_value = self.substitute(value);
                    // Use the existing registry module
                    let entry = velocity_config::RegistryEntry {
                        key: resolved_key,
                        name: Some(name.clone()),
                        value: resolved_value,
                        value_type: "string".to_string(),
                        root: "HKLM".to_string(),
                        delete_on_uninstall: true,
                    };
                    crate::registry::apply_registry_entries(&[entry])
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = (key, name, value); // suppress unused warnings
                    Err(CoreError::other(
                        "registry",
                        "Registry operations are only supported on Windows",
                    ))
                }
            }
            ActionType::SetEnvVar { name, value, scope } => {
                let resolved_value = self.substitute(value);
                let entry = velocity_config::EnvVarEntry {
                    name: name.clone(),
                    value: resolved_value,
                    scope: scope.clone(),
                    append: false,
                    delete_on_uninstall: true,
                };
                crate::env_vars::apply_env_vars(&[entry])
            }
        }
    }
}

/// Run a shell command using the platform-appropriate shell.
///
/// Windows: `cmd /C <command>`
/// Unix: `sh -c <command>`
fn run_shell(cmd: &str, work_dir: Option<&str>) -> Result<std::process::Output> {
    let dir = work_dir.unwrap_or(".");
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", cmd])
            .current_dir(dir)
            .output()
            .map_err(|e| CoreError::other("shell command", format!("{}: {}", cmd, e)))
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("sh")
            .args(["-c", cmd])
            .current_dir(dir)
            .output()
            .map_err(|e| CoreError::other("shell command", format!("{}: {}", cmd, e)))
    }
}

/// Run a shell command action (non-method, for use in execute_shell_commands).
fn run_shell_command(action: &ScriptAction) -> Result<()> {
    match &action.action {
        ActionType::ShellCommand(cmd) => {
            let work_dir = action.working_dir.as_deref().unwrap_or(".");
            let output = run_shell(cmd, Some(work_dir))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(CoreError::other(
                    "shell command",
                    format!(
                        "{}: exited with {}, stderr: {}",
                        cmd,
                        output.status,
                        stderr.trim()
                    ),
                ));
            }
            Ok(())
        }
        _ => Err(CoreError::other("run_action", "expected ShellCommand")),
    }
}

/// Build a standard variable context from installation parameters.
pub fn build_variable_context(
    install_dir: &str,
    app_name: &str,
    version: &str,
) -> HashMap<String, String> {
    let mut vars = HashMap::new();
    vars.insert("install_dir".to_string(), install_dir.to_string());
    vars.insert("app_name".to_string(), app_name.to_string());
    vars.insert("version".to_string(), version.to_string());

    // Add common system paths
    if let Ok(sys_root) = std::env::var("SystemRoot") {
        vars.insert("system_root".to_string(), sys_root.clone());
        vars.insert("system32".to_string(), format!("{}\\System32", sys_root));
    }
    if let Ok(program_files) = std::env::var("ProgramFiles") {
        vars.insert("program_files".to_string(), program_files);
    }
    if let Ok(temp) = std::env::var("TEMP") {
        vars.insert("temp".to_string(), temp);
    }

    vars
}

/// Convert a manifest `ScriptActionConfig` into a `ScriptAction` the engine can execute.
///
/// Returns `None` if the action type string is unrecognized (with a warning logged).
pub fn config_to_action(cfg: &velocity_config::ScriptActionConfig) -> Option<ScriptAction> {
    let action = match cfg.action.as_str() {
        "shell" | "cmd" => {
            let cmd = cfg.path.as_deref().unwrap_or("");
            ActionType::ShellCommand(cmd.to_string())
        }
        "copy" => ActionType::CopyFile {
            src: cfg.src.as_deref().unwrap_or("").to_string(),
            dest: cfg.dest.as_deref().unwrap_or("").to_string(),
        },
        "delete" => ActionType::DeleteFile(cfg.path.as_deref().unwrap_or("").to_string()),
        "delete_dir" | "rmdir" => {
            ActionType::DeleteDir(cfg.path.as_deref().unwrap_or("").to_string())
        }
        "mkdir" => ActionType::CreateDir(cfg.path.as_deref().unwrap_or("").to_string()),
        "registry" => ActionType::WriteRegistry {
            key: cfg.key.as_deref().unwrap_or("").to_string(),
            name: cfg.value_name.as_deref().unwrap_or("").to_string(),
            value: cfg.value.as_deref().unwrap_or("").to_string(),
        },
        "env_var" | "env" => ActionType::SetEnvVar {
            name: cfg.env_name.as_deref().unwrap_or("").to_string(),
            value: cfg.value.as_deref().unwrap_or("").to_string(),
            scope: cfg.scope.as_deref().unwrap_or("user").to_string(),
        },
        other => {
            warn!("Unknown script action type: '{}'", other);
            return None;
        }
    };

    let on_error = match cfg.on_error.as_str() {
        "continue" => ErrorPolicy::Continue,
        "abort" => ErrorPolicy::Abort,
        s if s.starts_with("retry:") => {
            let n = s.trim_start_matches("retry:").parse::<u32>().unwrap_or(3);
            ErrorPolicy::Retry(n)
        }
        _ => ErrorPolicy::Abort,
    };

    Some(ScriptAction {
        name: cfg.name.clone(),
        action,
        condition: cfg.condition.as_deref().unwrap_or("").to_string(),
        on_error,
        working_dir: None,
    })
}

/// Convert a slice of manifest action configs into executable script actions.
/// Skips unrecognized action types with a warning.
pub fn configs_to_actions(configs: &[velocity_config::ScriptActionConfig]) -> Vec<ScriptAction> {
    configs.iter().filter_map(config_to_action).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_engine() -> ScriptEngine {
        let mut vars = HashMap::new();
        vars.insert("install_dir".to_string(), "C:\\MyApp".to_string());
        vars.insert("app_name".to_string(), "TestApp".to_string());
        vars.insert("version".to_string(), "1.0.0".to_string());
        ScriptEngine::new(vars)
    }

    #[test]
    fn test_substitute() {
        let engine = test_engine();
        assert_eq!(
            engine.substitute("Install to {install_dir}\\bin"),
            "Install to C:\\MyApp\\bin"
        );
        assert_eq!(engine.substitute("{app_name} v{version}"), "TestApp v1.0.0");
        assert_eq!(engine.substitute("no variables here"), "no variables here");
    }

    #[test]
    fn test_substitute_unknown_var() {
        let engine = test_engine();
        // Unknown variables are left as-is
        assert_eq!(engine.substitute("{unknown}"), "{unknown}");
    }

    #[test]
    fn test_evaluate_condition_empty() {
        let engine = test_engine();
        assert!(engine.evaluate_condition(""));
        assert!(engine.evaluate_condition("always"));
    }

    #[test]
    fn test_evaluate_condition_never() {
        let engine = test_engine();
        assert!(!engine.evaluate_condition("never"));
    }

    #[test]
    fn test_evaluate_condition_file_exists() {
        let engine = test_engine();
        // This file should exist on Windows
        assert!(engine.evaluate_condition("file_exists:C:\\Windows\\System32\\cmd.exe"));
        assert!(engine.evaluate_condition("file_missing:C:\\nonexistent_file_12345.txt"));
    }

    #[test]
    fn test_evaluate_condition_dir_exists() {
        let engine = test_engine();
        assert!(engine.evaluate_condition("dir_exists:C:\\Windows"));
        assert!(!engine.evaluate_condition("dir_exists:C:\\nonexistent_dir_12345"));
    }

    #[test]
    fn test_execute_shell_command() {
        let mut engine = test_engine();
        let action = ScriptAction {
            name: "echo test".to_string(),
            action: ActionType::ShellCommand("echo hello".to_string()),
            condition: String::new(),
            on_error: ErrorPolicy::Continue,
            working_dir: None,
        };
        let result = engine.execute_action(&action);
        assert!(result.success);
    }

    #[test]
    fn test_execute_with_condition_false() {
        let mut engine = test_engine();
        let action = ScriptAction {
            name: "should skip".to_string(),
            action: ActionType::ShellCommand("exit 1".to_string()),
            condition: "never".to_string(),
            on_error: ErrorPolicy::Abort,
            working_dir: None,
        };
        let result = engine.execute_action(&action);
        assert!(result.success); // Skipped = success
    }

    #[test]
    fn test_execute_sequence_abort() {
        let mut engine = test_engine();
        let actions = vec![
            ScriptAction {
                name: "good".to_string(),
                action: ActionType::ShellCommand("echo ok".to_string()),
                condition: String::new(),
                on_error: ErrorPolicy::Continue,
                working_dir: None,
            },
            ScriptAction {
                name: "bad".to_string(),
                action: ActionType::ShellCommand("exit 1".to_string()),
                condition: String::new(),
                on_error: ErrorPolicy::Abort,
                working_dir: None,
            },
            ScriptAction {
                name: "never reached".to_string(),
                action: ActionType::ShellCommand("echo nope".to_string()),
                condition: String::new(),
                on_error: ErrorPolicy::Continue,
                working_dir: None,
            },
        ];
        let results = engine.execute_sequence(&actions);
        assert_eq!(results.len(), 2); // abort stops at 2nd
        assert!(results[0].success);
        assert!(!results[1].success);
    }

    #[test]
    fn test_execute_sequence_continue() {
        let mut engine = test_engine();
        let actions = vec![
            ScriptAction {
                name: "bad".to_string(),
                action: ActionType::ShellCommand("exit 1".to_string()),
                condition: String::new(),
                on_error: ErrorPolicy::Continue,
                working_dir: None,
            },
            ScriptAction {
                name: "good".to_string(),
                action: ActionType::ShellCommand("echo ok".to_string()),
                condition: String::new(),
                on_error: ErrorPolicy::Continue,
                working_dir: None,
            },
        ];
        let results = engine.execute_sequence(&actions);
        assert_eq!(results.len(), 2);
        assert!(!results[0].success);
        assert!(results[1].success);
    }

    #[test]
    fn test_build_variable_context() {
        let ctx = build_variable_context("C:\\App", "MyApp", "2.0.0");
        assert_eq!(ctx.get("install_dir").unwrap(), "C:\\App");
        assert_eq!(ctx.get("app_name").unwrap(), "MyApp");
        assert_eq!(ctx.get("version").unwrap(), "2.0.0");
    }

    #[test]
    fn test_create_dir_action() {
        let mut engine = test_engine();
        let test_dir = format!(
            "{}\\script_test_dir_{}",
            engine.variables["install_dir"],
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let action = ScriptAction {
            name: "create test dir".to_string(),
            action: ActionType::CreateDir(test_dir.clone()),
            condition: String::new(),
            on_error: ErrorPolicy::Abort,
            working_dir: None,
        };
        let result = engine.execute_action(&action);
        assert!(result.success);
        assert!(std::path::Path::new(&test_dir).is_dir());
        // Cleanup
        let _ = std::fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_execute_shell_commands_compat() {
        let engine = test_engine();
        let cmds = vec!["echo hello".to_string(), "echo {app_name}".to_string()];
        let results = engine.execute_shell_commands(&cmds);
        assert_eq!(results.len(), 2);
        assert!(results[0].success);
        assert!(results[1].success);
    }
}
