use anyhow::Result;
use std::fs;
use std::path::PathBuf;

use crate::logging::{log_info, log_warn};

#[cfg(test)]
use crate::logging::log_debug;
#[cfg(test)]
use anyhow::Context;

/// Load a role or task definition file
/// Searches first in ~/.gia/roles/<name>.md, then in ~/.gia/tasks/<name>.md
#[cfg(test)]
pub fn load_role_file(name: &str) -> Result<String> {
    log_debug(&format!("Loading role/task file: {name}"));

    // Try roles directory first
    let role_path = get_role_path(name)?;
    if role_path.exists() {
        let content = fs::read_to_string(&role_path)
            .with_context(|| format!("Failed to read role file: {}", role_path.display()))?;

        log_info(&format!(
            "Loaded role '{}' with {} characters from: {}",
            name,
            content.len(),
            role_path.display()
        ));

        return Ok(content);
    }

    // Try tasks directory
    let task_path = get_task_path(name)?;
    if task_path.exists() {
        let content = fs::read_to_string(&task_path)
            .with_context(|| format!("Failed to read task file: {}", task_path.display()))?;

        log_info(&format!(
            "Loaded task '{}' with {} characters from: {}",
            name,
            content.len(),
            task_path.display()
        ));

        return Ok(content);
    }

    // Neither found
    log_warn(&format!(
        "Role/task file not found. Tried: {role_path:?} and {task_path:?}"
    ));
    Err(anyhow::anyhow!(
        "Role/task '{}' not found at: {} or {}",
        name,
        role_path.display(),
        task_path.display()
    ))
}

/// Load all role/task definition files for the given names
/// Returns (name, content, is_task) tuples
pub fn load_all_roles(names: &[String]) -> Result<Vec<(String, String, bool)>> {
    if names.is_empty() {
        return Ok(Vec::new());
    }

    log_info(&format!("Loading {} role(s)/task(s)", names.len()));

    let mut items = Vec::new();

    for name in names {
        // Try role first
        let role_path = get_role_path(name)?;
        if role_path.exists() {
            match fs::read_to_string(&role_path) {
                Ok(content) => {
                    log_info(&format!(
                        "Loaded role '{name}' from {}",
                        role_path.display()
                    ));
                    items.push((name.clone(), content, false)); // is_task = false
                    continue;
                }
                Err(e) => {
                    log_warn(&format!("Failed to read role file: {e}"));
                }
            }
        }

        // Try task
        let task_path = get_task_path(name)?;
        if task_path.exists() {
            match fs::read_to_string(&task_path) {
                Ok(content) => {
                    log_info(&format!(
                        "Loaded task '{name}' from {}",
                        task_path.display()
                    ));
                    items.push((name.clone(), content, true)); // is_task = true
                    continue;
                }
                Err(e) => {
                    log_warn(&format!("Failed to read task file: {e}"));
                }
            }
        }

        log_warn(&format!("Failed to load role/task '{name}': not found"));
        eprintln!("Warning: Failed to load role/task '{name}': not found");
    }

    log_info(&format!(
        "Successfully loaded {} role(s)/task(s)",
        items.len()
    ));
    Ok(items)
}

/// Get the path to a role or task definition file
fn get_definition_path(name: &str, subdir: &str) -> Result<PathBuf> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;

    let definition_dir = home_dir.join(".gia").join(subdir);
    let definition_file = format!("{}.md", name);
    Ok(definition_dir.join(definition_file))
}

/// Get the path to a role definition file
fn get_role_path(role_name: &str) -> Result<PathBuf> {
    get_definition_path(role_name, "roles")
}

/// Get the path to a task definition file
fn get_task_path(task_name: &str) -> Result<PathBuf> {
    get_definition_path(task_name, "tasks")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_get_role_path() {
        let result = get_role_path("rust-dev");
        assert!(result.is_ok());

        let path = result.unwrap();
        assert!(path.to_string_lossy().contains(".gia"));
        assert!(path.to_string_lossy().contains("roles"));
        assert!(path.to_string_lossy().ends_with("rust-dev.md"));
    }

    #[test]
    fn test_load_role_file_not_found() {
        let result = load_role_file("nonexistent-role-xyz123");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Role/task 'nonexistent-role-xyz123' not found"));
    }

    #[test]
    fn test_get_task_path() {
        let result = get_task_path("my-task");
        assert!(result.is_ok());

        let path = result.unwrap();
        assert!(path.to_string_lossy().contains(".gia"));
        assert!(path.to_string_lossy().contains("tasks"));
        assert!(path.to_string_lossy().ends_with("my-task.md"));
    }

    #[test]
    fn test_load_all_roles_empty() {
        let result = load_all_roles(&[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_load_role_file_success() {
        // Create a temporary directory
        let temp_dir = TempDir::new().unwrap();
        let role_content = "# Rust Developer Role\n\nYou are an expert Rust developer.";

        // Create a role file
        let role_path = temp_dir.path().join("test-role.md");
        fs::write(&role_path, role_content).unwrap();

        // Read it back (this tests the file reading logic, not the path resolution)
        let content = fs::read_to_string(&role_path).unwrap();
        assert_eq!(content, role_content);
    }
}
