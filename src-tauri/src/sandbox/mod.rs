//! Sandbox path verification and security enforcement.
//!
//! All user-accessible file operations pass through [`Sandbox`] which
//! canonicalises paths and rejects escapes and dangerous patterns.

use std::path::{Path, PathBuf};
use thiserror::Error;

/// Sandbox root directory — all file operations are confined to this tree.
pub struct Sandbox {
    root: PathBuf,
}

/// Characters that are rejected in user-supplied path strings
/// (command injection prevention).
const DANGEROUS_CHARS: &[char] = &[';', '|', '&', '$', '`', '\n', '\r'];

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("path escapes sandbox: {0}")]
    Escape(String),
    #[error("path contains dangerous characters")]
    DangerousChars,
    #[error("path not found: {0}")]
    NotFound(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl Sandbox {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Canonicalised sandbox root (used for path validation).
    fn canonical_root(&self) -> Result<PathBuf, SandboxError> {
        Ok(std::fs::canonicalize(&self.root).unwrap_or_else(|_| self.root.clone()))
    }

    /// Validate and resolve a user-supplied path against the sandbox root.
    ///
    /// 1. Rejects dangerous characters (`;`, `|`, `&`, `$`, `` ` ``, newlines)
    /// 2. Rejects absolute paths (user must provide relative paths)
    /// 3. Canonicalises and verifies the result is inside the sandbox
    /// 4. Follows symlinks and verifies the *target* is inside the sandbox
    /// 5. For non-existent paths (e.g. new file creation), canonicalises the parent
    pub fn resolve(&self, user_path: &str) -> Result<PathBuf, SandboxError> {
        // Reject dangerous characters
        if user_path.chars().any(|c| DANGEROUS_CHARS.contains(&c)) {
            return Err(SandboxError::DangerousChars);
        }

        // Reject absolute paths — force relative
        let path = Path::new(user_path);
        if path.is_absolute() {
            return Err(SandboxError::Escape("absolute paths not allowed".into()));
        }

        // Canonicalise root first so comparison is symlink-safe
        let root_canonical = self.canonical_root()?;

        // Join with sandbox root
        let full = root_canonical.join(path);

        // Canonicalise to resolve .. and symlinks
        match std::fs::canonicalize(&full) {
            Ok(canonical) => {
                // Verify the canonical path starts with the canonical root
                if !canonical.starts_with(&root_canonical) {
                    return Err(SandboxError::Escape(format!(
                        "{user_path} resolves outside sandbox"
                    )));
                }
                Ok(canonical)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // File doesn't exist yet (e.g. sandbox_write creating a new file).
                // Validate by canonicalising the parent directory and joining.
                let parent = full.parent()
                    .ok_or_else(|| SandboxError::NotFound(user_path.to_string()))?;
                let parent_canon = std::fs::canonicalize(parent).map_err(|_| {
                    SandboxError::NotFound(format!("parent of {user_path}"))
                })?;
                if !parent_canon.starts_with(&root_canonical) {
                    return Err(SandboxError::Escape(format!(
                        "{user_path} resolves outside sandbox"
                    )));
                }
                Ok(parent_canon.join(
                    full.file_name()
                        .ok_or_else(|| SandboxError::NotFound(user_path.to_string()))?
                ))
            }
            Err(e) => Err(SandboxError::Io(e)),
        }
    }

    /// Return the sandbox root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Change the sandbox root at runtime.
    pub fn set_root(&mut self, path: PathBuf) {
        self.root = path;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_resolve_ok() {
        let dir = std::env::temp_dir().join("companion_sandbox_test");
        fs::create_dir_all(&dir).unwrap();
        let s = Sandbox::new(dir.clone());
        // Create a file to resolve against
        fs::write(dir.join("hello.txt"), "world").unwrap();
        let resolved = s.resolve("hello.txt").unwrap();
        // resolved is canonical; compare against canonical dir
        let canonical_dir = std::fs::canonicalize(&dir).unwrap_or(dir.clone());
        assert!(resolved.starts_with(&canonical_dir));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_reject_dangerous_chars() {
        let s = Sandbox::new(PathBuf::from("/tmp/sandbox"));
        assert!(s.resolve("file;ls").is_err());
        assert!(s.resolve("file|cat").is_err());
        assert!(s.resolve("file&cmd").is_err());
        assert!(s.resolve("$PATH").is_err());
    }

    #[test]
    fn test_resolve_new_file() {
        // sandbox_write should be able to resolve paths for new files
        let dir = std::env::temp_dir().join("companion_sandbox_newfile");
        fs::create_dir_all(&dir).unwrap();
        let s = Sandbox::new(dir.clone());
        let resolved = s.resolve("new_file.txt").unwrap();
        let canonical_dir = std::fs::canonicalize(&dir).unwrap_or(dir.clone());
        assert!(resolved.starts_with(&canonical_dir));
        assert!(resolved.ends_with("new_file.txt"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_reject_escape_dotdot() {
        let dir = std::env::temp_dir().join("companion_sandbox_escape");
        fs::create_dir_all(&dir).unwrap();
        let s = Sandbox::new(dir.clone());
        // .. should either not exist (NotFound) or escape (Escape)
        let result = s.resolve("../");
        assert!(result.is_err());
        fs::remove_dir_all(&dir).ok();
    }
}
