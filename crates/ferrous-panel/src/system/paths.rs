use crate::error::PanelError;
use serde::{Serialize, Serializer};
use std::fmt;
use std::path::{Component, Path, PathBuf};

/// Mandatory site containment root on Ubuntu VPS deployments.
pub const DEFAULT_WWWROOT: &str = "/www/wwwroot";

/// Validated filesystem path guaranteed to reject NUL bytes, `..` traversal,
/// lexical escape, and symlink escapes outside `/www/wwwroot` (or an explicit root).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SafePath(PathBuf);

impl SafePath {
    /// Resolves and validates `candidate` strictly inside `/www/wwwroot`.
    pub fn in_wwwroot(candidate: &str) -> Result<Self, PanelError> {
        Self::under_root(Path::new(DEFAULT_WWWROOT), candidate)
    }

    /// Resolves and validates `candidate` so it cannot escape `root`.
    ///
    /// Security invariants:
    /// 1. `root` must be an absolute path without `..`.
    /// 2. `candidate` cannot contain NUL bytes or any `..` component.
    /// 3. Resolved path must lexically stay within `root`.
    /// 4. When `root` exists on disk, every existing prefix/symlink along `resolved`
    ///    is canonicalized and verified to stay inside `canonicalize(root)`, blocking
    ///    both live symlink escapes and dangling symlink escapes.
    pub fn under_root(root: &Path, candidate: &str) -> Result<Self, PanelError> {
        if !root.is_absolute() {
            return Err(PanelError::Validation(
                "safe path root must be absolute".to_string(),
            ));
        }
        if candidate.contains('\0') {
            return Err(PanelError::Forbidden(
                "NUL byte in path is forbidden".to_string(),
            ));
        }

        let trimmed = candidate.trim();
        let candidate_path = Path::new(trimmed);

        let mut normalized_root = PathBuf::new();
        for comp in root.components() {
            match comp {
                Component::RootDir => normalized_root.push(comp.as_os_str()),
                Component::Normal(seg) => normalized_root.push(seg),
                Component::CurDir => {}
                Component::ParentDir | Component::Prefix(_) => {
                    return Err(PanelError::Validation(
                        "invalid root path component".to_string(),
                    ));
                }
            }
        }

        let mut resolved = if candidate_path.is_absolute() {
            PathBuf::new()
        } else {
            normalized_root.clone()
        };

        for comp in candidate_path.components() {
            match comp {
                Component::RootDir => {
                    if candidate_path.is_absolute() {
                        resolved.push(comp.as_os_str());
                    }
                }
                Component::CurDir => {}
                Component::Normal(seg) => resolved.push(seg),
                Component::ParentDir => {
                    return Err(PanelError::Forbidden(
                        "path traversal ('..') is forbidden".to_string(),
                    ));
                }
                Component::Prefix(_) => {
                    return Err(PanelError::Validation(
                        "unsupported path prefix".to_string(),
                    ));
                }
            }
        }

        if resolved != normalized_root && !resolved.starts_with(&normalized_root) {
            return Err(PanelError::Forbidden(
                "path escapes allowed root directory".to_string(),
            ));
        }

        // Canonicalization and symlink escape enforcement when `root` exists on disk.
        if std::fs::symlink_metadata(&normalized_root).is_ok() {
            let canon_root = std::fs::canonicalize(&normalized_root)?;
            let mut prefix = normalized_root.clone();
            let rel_tail = resolved.strip_prefix(&normalized_root).map_err(|_| {
                PanelError::Forbidden("path escapes allowed root directory".to_string())
            })?;

            let mut existing_prefix_canon = canon_root.clone();
            let mut non_existent_tail = PathBuf::new();
            let mut missing_seen = false;

            for comp in rel_tail.components() {
                if let Component::Normal(seg) = comp {
                    if missing_seen {
                        non_existent_tail.push(seg);
                        continue;
                    }
                    prefix.push(seg);
                    if std::fs::symlink_metadata(&prefix).is_ok() {
                        let canon_step = std::fs::canonicalize(&prefix).map_err(|_| {
                            PanelError::Forbidden(
                                "unresolvable or dangling symlink inside root is forbidden"
                                    .to_string(),
                            )
                        })?;
                        if canon_step != canon_root && !canon_step.starts_with(&canon_root) {
                            return Err(PanelError::Forbidden(
                                "symlink resolves outside allowed root directory".to_string(),
                            ));
                        }
                        existing_prefix_canon = canon_step;
                    } else {
                        missing_seen = true;
                        non_existent_tail.push(seg);
                    }
                }
            }

            let final_path = if non_existent_tail.as_os_str().is_empty() {
                existing_prefix_canon
            } else {
                existing_prefix_canon.join(non_existent_tail)
            };
            return Ok(Self(final_path));
        }

        Ok(Self(resolved))
    }

    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl fmt::Display for SafePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl Serialize for SafePath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let s = self
            .0
            .to_str()
            .ok_or_else(|| serde::ser::Error::custom("path contains non-UTF-8 characters"))?;
        serializer.serialize_str(s)
    }
}
