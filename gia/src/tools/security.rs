use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;

/// Security context for tool execution
///
/// Implements defense-in-depth security:
/// - Path allowlisting for file operations
/// - File size limits to prevent resource exhaustion
/// - Command execution controls
/// - Blocklist of dangerous commands
pub struct SecurityContext {
    /// Allowed directories for file operations (empty = allow all)
    allowed_dirs: HashSet<PathBuf>,

    /// Maximum file size to read (bytes)
    max_file_size: usize,

    /// Enable web search
    allow_web_search: bool,

    /// Allow command execution
    allow_command_execution: bool,

    /// Command execution timeout
    command_timeout: Duration,

    /// Require user confirmation before executing commands
    confirm_commands: bool,
}

/// Blocklist of dangerous commands (case-insensitive matching)
const BLOCKED_COMMANDS: &[&str] = &[
    "rm -rf",
    "rm-rf",
    "rmdir /s",
    "dd",
    "mkfs",
    "format",
    ":(){ :|:& };:", // Fork bomb
    "chmod -r 777",
    "chmod -r 000",
    "chown -r",
    "iptables",
    "ufw disable",
    "shutdown",
    "reboot",
    "halt",
    "poweroff",
    "init 0",
    "init 6",
];

impl SecurityContext {
    /// Create a new security context with safe defaults
    pub fn new() -> Self {
        Self {
            allowed_dirs: HashSet::new(),
            max_file_size: 10 * 1024 * 1024, // 10MB
            allow_web_search: true,
            allow_command_execution: false,
            command_timeout: Duration::from_secs(30),
            confirm_commands: false,
        }
    }

    /// Add an allowed directory (builder pattern)
    pub fn with_allowed_dir(mut self, dir: impl AsRef<Path>) -> Self {
        let canonical = dir
            .as_ref()
            .canonicalize()
            .unwrap_or_else(|_| dir.as_ref().to_path_buf());
        self.allowed_dirs.insert(canonical);
        self
    }

    /// Allow current directory (builder pattern)
    pub fn allow_current_dir(mut self) -> Result<Self> {
        let cwd = std::env::current_dir()?;
        self.allowed_dirs.insert(cwd);
        Ok(self)
    }

    /// Set maximum file size (builder pattern)
    pub fn with_max_file_size(mut self, size: usize) -> Self {
        self.max_file_size = size;
        self
    }

    /// Set web search permission (builder pattern)
    pub fn with_allow_web_search(mut self, allow: bool) -> Self {
        self.allow_web_search = allow;
        self
    }

    /// Set command execution permission (builder pattern)
    pub fn with_allow_command_execution(mut self, allow: bool) -> Self {
        self.allow_command_execution = allow;
        self
    }

    /// Set command timeout (builder pattern)
    pub fn with_command_timeout(mut self, timeout: Duration) -> Self {
        self.command_timeout = timeout;
        self
    }

    /// Set command confirmation requirement (builder pattern)
    pub fn with_confirm_commands(mut self, confirm: bool) -> Self {
        self.confirm_commands = confirm;
        self
    }

    /// Check if a path is allowed for access
    ///
    /// DRY: Single validation method used by all file tools
    pub fn is_path_allowed(&self, path: &Path) -> bool {
        // If no restrictions, allow anything
        if self.allowed_dirs.is_empty() {
            return true;
        }

        // Canonicalize path to resolve symlinks and prevent traversal attacks
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                // If path doesn't exist yet (e.g., for write), check parent
                if let Some(parent) = path.parent() {
                    match parent.canonicalize() {
                        Ok(p) => p,
                        Err(_) => return false,
                    }
                } else {
                    return false;
                }
            }
        };

        // Check if path is within any allowed directory
        self.allowed_dirs
            .iter()
            .any(|allowed| canonical.starts_with(allowed))
    }

    /// Check if a command is allowed for execution
    ///
    /// DRY: Single validation method used by ExecuteCommandTool
    pub fn is_command_allowed(&self, command: &str) -> bool {
        let command_lower = command.to_lowercase();

        // Check against blocklist (case-insensitive)
        for blocked in BLOCKED_COMMANDS {
            if command_lower.contains(&blocked.to_lowercase()) {
                return false;
            }
        }

        true
    }

    /// Get maximum file size
    pub fn max_file_size(&self) -> usize {
        self.max_file_size
    }

    /// Check if web search is allowed
    pub fn is_web_search_allowed(&self) -> bool {
        self.allow_web_search
    }

    /// Check if command execution is allowed
    pub fn is_command_execution_allowed(&self) -> bool {
        self.allow_command_execution
    }

    /// Get command timeout
    pub fn command_timeout(&self) -> Duration {
        self.command_timeout
    }

    /// Check if commands require confirmation
    pub fn requires_command_confirmation(&self) -> bool {
        self.confirm_commands
    }
}

impl Default for SecurityContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_empty_allowed_dirs_allows_all() {
        let context = SecurityContext::new();
        let path = Path::new("/tmp/test.txt");
        assert!(context.is_path_allowed(path));
    }

    #[test]
    fn test_allowed_dir_validation() {
        let temp_dir = env::temp_dir();
        let context = SecurityContext::new().with_allowed_dir(&temp_dir);

        let allowed_path = temp_dir.join("test.txt");
        assert!(context.is_path_allowed(&allowed_path));

        let denied_path = Path::new("/etc/passwd");
        assert!(!context.is_path_allowed(denied_path));
    }

    #[test]
    fn test_command_blocklist() {
        let context = SecurityContext::new();

        assert!(!context.is_command_allowed("rm -rf /"));
        assert!(!context.is_command_allowed("RM -RF /"));
        assert!(!context.is_command_allowed("dd if=/dev/zero of=/dev/sda"));
        assert!(!context.is_command_allowed(":(){ :|:& };:"));
        assert!(!context.is_command_allowed("chmod -R 777 /"));

        assert!(context.is_command_allowed("ls -la"));
        assert!(context.is_command_allowed("git status"));
        assert!(context.is_command_allowed("cargo test"));
    }

    #[test]
    fn test_builder_pattern() {
        let context = SecurityContext::new()
            .with_max_file_size(5 * 1024 * 1024)
            .with_allow_web_search(false)
            .with_allow_command_execution(true)
            .with_command_timeout(Duration::from_secs(60))
            .with_confirm_commands(true);

        assert_eq!(context.max_file_size(), 5 * 1024 * 1024);
        assert!(!context.is_web_search_allowed());
        assert!(context.is_command_execution_allowed());
        assert_eq!(context.command_timeout(), Duration::from_secs(60));
        assert!(context.requires_command_confirmation());
    }
}
