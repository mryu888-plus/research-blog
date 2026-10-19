//! Maintain a private Zola input tree with protected Markdown mathematics.

use crate::markdown::markdown_body;
use crate::math::protect_math;
use crate::paths::{absolute, is_link, reject_link, reject_link_parents, sort_names, RelPath};
use crate::references::{prepare_references, SourceFile};
use anyhow::{anyhow, bail, Context, Result};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const INPUT_DIRECTORIES: [&str; 6] = ["content", "templates", "static", "sass", "data", "themes"];
const EXCLUDED_NAMES: [&str; 3] = ["public", ".git", "article-manifest.json"];
pub const ORIGINAL_BODIES: &str = "data/original-bodies.json";
const WRITE_DIRECTORY: &str = ".prepare-tmp";

#[derive(Clone, PartialEq, Eq, Debug)]
enum Fingerprint {
    Stat { modified: u128, size: u64 },
    Digest([u8; 32]),
}

fn digest(data: &[u8]) -> Fingerprint {
    Fingerprint::Digest(Sha256::digest(data).into())
}

fn stat_fingerprint(metadata: &fs::Metadata) -> Fingerprint {
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_nanos());
    Fingerprint::Stat {
        modified,
        size: metadata.len(),
    }
}

fn is_markdown(relative: &RelPath) -> bool {
    relative.first() == "content" && relative.suffix_lower() == ".md"
}

/// Mirror approved Zola inputs into an empty, separate staging directory.
///
/// `sync` returns relative paths for changed or removed files. Only files and
/// directories created by this instance are ever removed; generated Zola
/// output in the staging tree is left alone.
pub struct PreparedSite {
    pub source: PathBuf,
    pub target: PathBuf,
    snapshot: HashMap<RelPath, Fingerprint>,
    directories: BTreeSet<RelPath>,
    texts: HashMap<RelPath, (Fingerprint, String)>,
    /// The inputs of the last successful sync; an identical scan needs no work.
    last_scan: Option<Scan>,
}

#[derive(PartialEq, Eq)]
struct Scan {
    /// Files in scan order: config, then each input directory depth-first.
    files: Vec<(RelPath, Fingerprint)>,
    directories: BTreeSet<RelPath>,
}

impl PreparedSite {
    pub fn new(source: &Path, target: &Path) -> Result<Self> {
        let source = absolute(source)?;
        let target = absolute(target)?;
        reject_link_parents(&source)?;
        reject_link_parents(&target)?;
        if source == target || target.starts_with(&source) || source.starts_with(&target) {
            bail!("Source and staging target must not overlap");
        }
        if !source.is_dir() {
            bail!("Site source is not a directory: {}", source.display());
        }
        if target.exists() && (!target.is_dir() || fs::read_dir(&target)?.next().is_some()) {
            bail!(
                "Staging target must be an empty directory: {}",
                target.display()
            );
        }
        fs::create_dir_all(&target)?;
        Ok(PreparedSite {
            source,
            target,
            snapshot: HashMap::new(),
            directories: BTreeSet::new(),
            texts: HashMap::new(),
            last_scan: None,
        })
    }

    /// Validate the complete input selection before changing the mirror.
    fn scan(&self) -> Result<Scan> {
        reject_link_parents(&self.source)?;
        let mut scan = Scan {
            files: Vec::new(),
            directories: BTreeSet::new(),
        };
        let config = self.source.join("config.toml");
        reject_link(&config)?;
        if !config.is_file() {
            bail!("Site config is missing: {}", config.display());
        }
        scan.files.push((
            RelPath::new("config.toml"),
            stat_fingerprint(&fs::metadata(&config)?),
        ));
        for name in INPUT_DIRECTORIES {
            let directory = self.source.join(name);
            let Ok(metadata) = fs::symlink_metadata(&directory) else {
                continue;
            };
            if is_link(&metadata) {
                bail!(
                    "Site staging does not accept links or reparse points: {}",
                    directory.display()
                );
            }
            if !metadata.is_dir() {
                bail!("Site input must be a directory: {}", directory.display());
            }
            self.visit(&directory, RelPath::new(name), &mut scan)?;
        }
        Ok(scan)
    }

    fn visit(&self, directory: &Path, relative: RelPath, scan: &mut Scan) -> Result<()> {
        scan.directories.insert(relative.clone());
        let mut entries = HashMap::new();
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|name| anyhow!("Unsupported file name: {name:?}"))?;
            if EXCLUDED_NAMES.contains(&name.as_str()) {
                continue;
            }
            entries.insert(name, entry);
        }
        let mut names: Vec<String> = entries.keys().cloned().collect();
        sort_names(&mut names);
        for name in names {
            let entry = &entries[&name];
            let path = entry.path();
            // On Windows the directory listing already carries attributes,
            // so this costs no extra system call per file.
            let metadata = entry.metadata()?;
            if is_link(&metadata) {
                bail!(
                    "Site staging does not accept links or reparse points: {}",
                    path.display()
                );
            }
            let child = relative.join(&name);
            if metadata.is_dir() {
                self.visit(&path, child, scan)?;
            } else if metadata.is_file() {
                scan.files.push((child, stat_fingerprint(&metadata)));
            } else {
                bail!("Unsupported site input: {}", path.display());
            }
        }
        Ok(())
    }

    /// The staging path for `relative`, refusing links between the target
    /// and the file. The target's own ancestors are checked once per sync.
    fn destination(&self, relative: &RelPath) -> Result<PathBuf> {
        let mut current = self.target.clone();
        for part in relative.parts() {
            if part == ".." || part == "." {
                bail!("Staging path escaped its target: {}", relative.as_str());
            }
            current.push(part);
            reject_link(&current)?;
        }
        Ok(current)
    }

    fn read_text(&mut self, relative: &RelPath, fingerprint: &Fingerprint) -> Result<String> {
        if let Some((cached, text)) = self.texts.get(relative) {
            if cached == fingerprint {
                return Ok(text.clone());
            }
        }
        let path = relative.under(&self.source);
        let bytes = fs::read(&path)?;
        let text = String::from_utf8(bytes)
            .map_err(|_| anyhow!("{} is not valid UTF-8", path.display()))?;
        self.texts
            .insert(relative.clone(), (fingerprint.clone(), text.clone()));
        Ok(text)
    }

    pub fn sync(&mut self) -> Result<Vec<RelPath>> {
        let scan = self.scan()?;
        if self.last_scan.as_ref() == Some(&scan) {
            return Ok(Vec::new());
        }
        self.last_scan = None;
        let scanned = Scan {
            files: scan.files.clone(),
            directories: scan.directories.clone(),
        };
        let mut texts = Vec::new();
        for (relative, fingerprint) in &scan.files {
            if is_markdown(relative) {
                texts.push((relative.clone(), self.read_text(relative, fingerprint)?));
            }
        }
        self.texts
            .retain(|relative, _| texts.iter().any(|(r, _)| r == relative));
        let file_list: Vec<RelPath> = scan.files.iter().map(|(r, _)| r.clone()).collect();
        let sources: Vec<SourceFile> = texts
            .iter()
            .map(|(relative, text)| SourceFile { relative, text })
            .collect();
        let references = prepare_references(&self.source, &file_list, &sources)?;
        reject_link_parents(&self.target)?;
        if !self.target.is_dir() {
            bail!("Staging target is missing: {}", self.target.display());
        }
        // Reading-time templates always use this private generated input. A new
        // site does not need to supply a placeholder or even a data directory.
        // Source Markdown and any legacy placeholder remain untouched.
        let original_bodies = RelPath::new(ORIGINAL_BODIES);
        let mut directories = scan.directories;
        if directories.contains(&original_bodies) {
            bail!("Reserved reading data path must be a file: {ORIGINAL_BODIES}");
        }
        let bodies: serde_json::Map<String, serde_json::Value> = texts
            .iter()
            .map(|(relative, text)| {
                (
                    relative.strip_first().to_string(),
                    markdown_body(text).into(),
                )
            })
            .collect();
        let original_data =
            crate::json::python_dumps(&serde_json::Value::Object(bodies)).into_bytes();
        let mut files: BTreeMap<RelPath, Fingerprint> = scan.files.into_iter().collect();
        files.insert(original_bodies.clone(), digest(&original_data));
        directories.insert(original_bodies.parent());
        for relative in &references.excluded {
            files.remove(relative);
        }
        for (relative, data) in &references.overrides {
            files.insert(relative.clone(), digest(data));
            directories.extend(relative.ancestors());
        }
        let mut changed = Vec::new();
        // Deletions first also support changing a file into a directory.
        let removed: BTreeSet<RelPath> = self
            .snapshot
            .keys()
            .filter(|relative| !files.contains_key(*relative))
            .cloned()
            .collect();
        for relative in removed {
            let destination = self.destination(&relative)?;
            if destination.exists() {
                if !destination.is_file() {
                    bail!("Unexpected staging directory: {}", destination.display());
                }
                fs::remove_file(&destination)?;
            }
            self.snapshot.remove(&relative);
            changed.push(relative);
        }

        // Only remove formerly owned, empty directories. Zola may have written
        // other files into the staging tree, which this helper never owns.
        let mut stale: Vec<RelPath> = self.directories.difference(&directories).cloned().collect();
        stale.sort_by_key(|relative| std::cmp::Reverse(relative.depth()));
        for relative in stale {
            let destination = self.destination(&relative)?;
            if destination.exists() {
                if !destination.is_dir() {
                    bail!("Unexpected staging file: {}", destination.display());
                }
                if fs::read_dir(&destination)?.next().is_none() {
                    fs::remove_dir(&destination)?;
                    self.directories.remove(&relative);
                }
            }
        }

        let mut created: Vec<&RelPath> = directories.iter().collect();
        created.sort_by_key(|relative| relative.depth());
        for relative in created {
            if !self.directories.contains(relative) || !relative.under(&self.target).is_dir() {
                fs::create_dir_all(self.destination(relative)?)?;
            }
            self.directories.insert(relative.clone());
        }

        // Publish reading data before content so a live rebuild sees its matching
        // original body as soon as the transformed Markdown changes.
        let mut order: Vec<(&RelPath, &Fingerprint)> = files.iter().collect();
        order.sort_by(|a, b| (*a.0 != original_bodies, a.0).cmp(&(*b.0 != original_bodies, b.0)));
        let texts: HashMap<&RelPath, &String> = texts.iter().map(|(r, t)| (r, t)).collect();
        for (relative, fingerprint) in order {
            // An unchanged file is not rewritten, so it needs no link checks.
            if self.snapshot.get(relative) == Some(fingerprint)
                && relative.under(&self.target).is_file()
            {
                continue;
            }
            let destination = self.destination(relative)?;
            if destination.exists() && !self.snapshot.contains_key(relative) {
                bail!(
                    "Refusing to replace an unowned staging file: {}",
                    destination.display()
                );
            }
            let data = if *relative == original_bodies {
                original_data.clone()
            } else if let Some(data) = references.overrides.get(relative) {
                data.clone()
            } else if let Some(text) = texts.get(relative) {
                text.as_bytes().to_vec()
            } else {
                fs::read(relative.under(&self.source))?
            };
            let data = if is_markdown(relative) {
                let text = String::from_utf8(data)
                    .map_err(|_| anyhow!("{} is not valid UTF-8", relative.native()))?;
                protect_math(&text).into_bytes()
            } else {
                data
            };
            self.write_atomic(&destination, &data)?;
            // Keep successful work owned even if a later input fails to
            // transform, so a preview can recover on its next sync.
            self.snapshot.insert(relative.clone(), fingerprint.clone());
            changed.push(relative.clone());
        }

        self.snapshot = files.into_iter().collect();
        self.directories = directories;
        self.last_scan = Some(scanned);
        changed.sort();
        Ok(changed)
    }

    fn write_atomic(&self, destination: &Path, data: &[u8]) -> Result<()> {
        // Zola 0.23.4 ignores paired Linux rename events. Stage writes in an
        // unwatched child of its non-recursive root watch, so publishing
        // produces a supported RenameMode::To while remaining atomic.
        let directory = self.destination(&RelPath::new(WRITE_DIRECTORY))?;
        fs::create_dir_all(&directory)?;
        let mut temporary = tempfile_in(&directory)?;
        let result = (|| {
            temporary.1.write_all(data)?;
            drop(temporary.1);
            fs::rename(&temporary.0, destination)
                .with_context(|| format!("cannot replace {}", destination.display()))
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary.0);
        }
        result
    }
}

fn tempfile_in(directory: &Path) -> Result<(PathBuf, fs::File)> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    loop {
        let unique = format!(
            ".prepare-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let path = directory.join(unique);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
}
