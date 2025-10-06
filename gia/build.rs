use std::process::Command;

fn main() {
    // Get git commit count
    let commit_count = get_git_commit_count().unwrap_or(0);

    // Check if repo is dirty
    let is_dirty = is_git_dirty().unwrap_or(false);

    // Generate version string: 0.1.{commit_count}[+dirty]
    let version = if is_dirty {
        format!("0.1.{}+dirty", commit_count)
    } else {
        format!("0.1.{}", commit_count)
    };

    // Set environment variables for use in the code
    println!("cargo:rustc-env=GIA_VERSION={}", version);
    println!("cargo:rustc-env=GIA_COMMIT_COUNT={}", commit_count);
    println!("cargo:rustc-env=GIA_IS_DIRTY={}", is_dirty);

    // Rerun build script if .git/HEAD changes (when commits are made)
    println!("cargo:rerun-if-changed=.git/HEAD");

    // Also watch for changes in .git/refs/ (for branch changes)
    println!("cargo:rerun-if-changed=.git/refs/");

    // Watch for changes in the index (staged files)
    println!("cargo:rerun-if-changed=.git/index");
}

fn get_git_commit_count() -> Option<u32> {
    // Try to get commit count using git rev-list --count HEAD
    match Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .output()
    {
        Ok(output) => {
            if output.status.success() {
                let count_str = String::from_utf8_lossy(&output.stdout);
                count_str.trim().parse::<u32>().ok()
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

fn is_git_dirty() -> Option<bool> {
    // Check if there are uncommitted changes using git status --porcelain
    match Command::new("git").args(["status", "--porcelain"]).output() {
        Ok(output) => {
            if output.status.success() {
                let status_output = String::from_utf8_lossy(&output.stdout);
                // If output is not empty, repo is dirty
                Some(!status_output.trim().is_empty())
            } else {
                None
            }
        }
        Err(_) => None,
    }
}
