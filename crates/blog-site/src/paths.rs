//! Relative paths inside a site, kept as `/`-separated text.

use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

/// A relative path such as `content/posts/a/index.md`.
///
/// Ordering follows Python's `pathlib`: part by part, ignoring case on Windows.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RelPath(String);

impl RelPath {
    pub fn new(path: &str) -> Self {
        RelPath(path.trim_matches('/').to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    pub fn parts(&self) -> impl Iterator<Item = &str> {
        self.0.split('/').filter(|part| !part.is_empty())
    }

    pub fn depth(&self) -> usize {
        self.parts().count()
    }

    pub fn first(&self) -> &str {
        self.parts().next().unwrap_or("")
    }

    pub fn name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or("")
    }

    /// Python `PurePath.suffix`, lower-cased.
    pub fn suffix_lower(&self) -> String {
        let name = self.name();
        match name.rfind('.') {
            Some(dot) if dot > 0 && dot < name.len() - 1 => name[dot..].to_lowercase(),
            _ => String::new(),
        }
    }

    pub fn stem(&self) -> String {
        let name = self.name();
        match name.rfind('.') {
            Some(dot) if dot > 0 && dot < name.len() - 1 => name[..dot].to_string(),
            _ => name.to_string(),
        }
    }

    pub fn parent(&self) -> RelPath {
        match self.0.rfind('/') {
            Some(slash) => RelPath(self.0[..slash].to_string()),
            None => RelPath(String::new()),
        }
    }

    /// Every ancestor directory, nearest first, excluding the root.
    pub fn ancestors(&self) -> Vec<RelPath> {
        let mut result = Vec::new();
        let mut current = self.parent();
        while !current.is_root() {
            result.push(current.clone());
            current = current.parent();
        }
        result
    }

    pub fn join(&self, name: &str) -> RelPath {
        if self.is_root() {
            RelPath::new(name)
        } else {
            RelPath(format!("{}/{}", self.0, name.trim_matches('/')))
        }
    }

    /// True when `directory` is a proper ancestor of this path.
    pub fn has_ancestor(&self, directory: &RelPath) -> bool {
        directory.is_root() || self.0.starts_with(&format!("{}/", directory.0))
    }

    /// The path without its first component, e.g. `posts/a/index.md`.
    pub fn strip_first(&self) -> &str {
        self.0.split_once('/').map_or("", |(_, rest)| rest)
    }

    pub fn under(&self, base: &Path) -> PathBuf {
        let mut path = base.to_path_buf();
        for part in self.parts() {
            path.push(part);
        }
        path
    }

    /// The platform spelling, as Python prints a `Path`.
    pub fn native(&self) -> String {
        if cfg!(windows) {
            self.0.replace('/', "\\")
        } else {
            self.0.clone()
        }
    }

    fn key(&self) -> Vec<String> {
        self.parts()
            .map(|part| {
                if cfg!(windows) {
                    part.to_lowercase()
                } else {
                    part.to_string()
                }
            })
            .collect()
    }
}

impl Ord for RelPath {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key()
            .cmp(&other.key())
            .then_with(|| self.0.cmp(&other.0))
    }
}

impl PartialOrd for RelPath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Sort names the way Python sorts sibling `Path` objects.
pub fn sort_names(names: &mut [String]) {
    names.sort_by(|a, b| {
        let fold = |s: &str| {
            if cfg!(windows) {
                s.to_lowercase()
            } else {
                s.to_string()
            }
        };
        fold(a).cmp(&fold(b)).then_with(|| a.cmp(b))
    });
}

/// True for symlinks and, on Windows, junctions and other reparse points.
pub fn is_link(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

/// Reject both ordinary symlinks and Windows junction/reparse points.
pub fn reject_link(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_link(&metadata) => {
            bail!(
                "Site staging does not accept links or reparse points: {}",
                path.display()
            )
        }
        _ => Ok(()),
    }
}

pub fn reject_link_parents(path: &Path) -> Result<()> {
    for current in path.ancestors() {
        if current.as_os_str().is_empty() {
            break;
        }
        reject_link(current)?;
    }
    Ok(())
}

/// An absolute, lexically normalized path without resolving links.
pub fn absolute(path: &Path) -> Result<PathBuf> {
    Ok(std::path::absolute(path)?)
}
