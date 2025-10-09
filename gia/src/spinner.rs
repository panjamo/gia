use std::process::{Child, Command};

/// Manages the giagui spinner process lifecycle
pub struct SpinnerProcess {
    child: Option<Child>,
}

impl SpinnerProcess {
    /// Attempts to start the giagui spinner process.
    /// Returns a SpinnerProcess instance whether successful or not.
    /// Errors are silently ignored.
    pub fn start() -> Self {
        let child = Command::new("giagui").arg("--spinner").spawn().ok(); // Convert Result to Option, silently ignoring errors

        SpinnerProcess { child }
    }
}

impl Drop for SpinnerProcess {
    /// Automatically kills the spinner process when dropped
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill(); // Silently ignore kill errors
            let _ = child.wait(); // Silently ignore wait errors
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_process_creation() {
        // This test just verifies that creating a SpinnerProcess doesn't panic
        // even when giagui is not available
        let _spinner = SpinnerProcess::start();
    }

    #[test]
    fn test_spinner_process_drop() {
        // This test verifies that dropping a SpinnerProcess doesn't panic
        let spinner = SpinnerProcess::start();
        drop(spinner);
    }
}
