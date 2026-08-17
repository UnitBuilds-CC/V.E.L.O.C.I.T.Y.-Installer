//! Dependency condition resolver.
//!
//! Delegates to the unified `condition` module for evaluating condition
//! expressions. This module provides a convenience wrapper that returns
//! `bool` (defaulting to `true` on error) for use in dependency resolution.
//!
//! Cross-platform: delegates to condition.rs which supports cross-platform
//! conditions (file, dir, env, arch) and returns errors for Windows-only
//! conditions on Unix.

use crate::condition;
use tracing::debug;

/// Evaluate a dependency condition string.
/// Returns `true` if the dependency SHOULD be installed.
///
/// On parse errors, defaults to `true` (safe fallback — install the dependency).
pub fn evaluate_condition(condition_str: &str) -> bool {
    match condition::evaluate_condition(condition_str) {
        Ok(result) => result,
        Err(e) => {
            debug!("Condition evaluation error: {}. Defaulting to install.", e);
            true
        }
    }
}

/// Evaluate multiple conditions (all must be true).
pub fn evaluate_all_conditions(conditions: &[String]) -> bool {
    match condition::evaluate_all_conditions(conditions) {
        Ok(result) => result,
        Err(e) => {
            debug!("Condition evaluation error: {}. Defaulting to install.", e);
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_always_condition() {
        assert!(evaluate_condition("always"));
    }

    #[test]
    fn test_never_condition() {
        assert!(!evaluate_condition("never"));
    }

    #[test]
    fn test_file_missing_condition() {
        assert!(evaluate_condition(
            "file_missing:/nonexistent_file_xyz_12345.dll"
        ));
    }

    #[test]
    fn test_dir_exists_condition() {
        assert!(evaluate_condition("dir_exists:."));
    }

    #[test]
    fn test_unknown_condition_defaults_to_true() {
        // Unknown condition types return an error, which defaults to true
        assert!(evaluate_condition("some_unknown_condition"));
    }

    #[test]
    fn test_evaluate_all_conditions() {
        let conditions = vec!["always".to_string(), "dir_exists:.".to_string()];
        assert!(evaluate_all_conditions(&conditions));

        let conditions_with_false = vec!["always".to_string(), "never".to_string()];
        assert!(!evaluate_all_conditions(&conditions_with_false));
    }

    #[test]
    fn test_env_condition() {
        assert!(evaluate_condition("env:PATH"));
        assert!(!evaluate_condition("env:VELOCITY_NONEXISTENT_VAR_XYZ"));
    }
}
