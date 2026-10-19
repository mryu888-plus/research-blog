//! Validate any Research Blog build: links, search data and public-only assets.
//!
//! A port of `scripts/check_site.py`. Page parsing, URL joining and JSON
//! errors follow Python 3.13 so both report the same failures in the same order.

mod entities;
mod html;
mod pyjson;
mod url;

use anyhow::{anyhow, bail, Result};
use html::{parse_page, unescape, universal_newlines, Page};
use pyjson::Json;
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use url::{unquote, urljoin, urlsplit};

pub struct CheckSummary {
    pub pages: usize,
    pub links: usize,
    pub articles: usize,
}

const JSON_ROUTES: [&str; 2] = ["search-index/index.html", "graph-data/index.html"];
const REQUIRED_ROUTES: [&str; 6] = [
    "index.html",
    "atom.xml",
    "sitemap.xml",
    "404.html",
    "graph/index.html",
    "references/index.html",
];
const PRIVATE_NAMES: [&str; 10] = [
    "contact.local.json",
    "resume-public.pdf",
    "preview-editor.json",
    "references.json",
    "original-bodies.json",
    "article-manifest.json",
    "localapikeys.json",
    "zotero.sqlite",
    "zotero.sqlite-journal",
    "zotero.sqlite-wal",
];

fn read_utf8(path: &Path, strip_bom: bool) -> Result<String> {
    let bytes = fs::read(path).map_err(|error| os_error(&error, path))?;
    let bytes = if strip_bom {
        bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes)
    } else {
        &bytes[..]
    };
    let text = std::str::from_utf8(bytes).map_err(|error| {
        let position = error.valid_up_to();
        let reason = match error.error_len() {
            None => "unexpected end of data",
            Some(_) if (0x80..0xc2).contains(&bytes[position]) || bytes[position] > 0xf4 => {
                "invalid start byte"
            }
            Some(_) => "invalid continuation byte",
        };
        anyhow!(
            "'utf-8' codec can't decode byte 0x{:02x} in position {position}: {reason}",
            bytes[position]
        )
    })?;
    Ok(universal_newlines(text))
}

/// The text of an `OSError` as Python prints it.
fn os_error(error: &std::io::Error, path: &Path) -> anyhow::Error {
    let name = url::python_repr(&path.display().to_string());
    match error.kind() {
        std::io::ErrorKind::NotFound => anyhow!("[Errno 2] No such file or directory: {name}"),
        std::io::ErrorKind::PermissionDenied => anyhow!("[Errno 13] Permission denied: {name}"),
        std::io::ErrorKind::IsADirectory => anyhow!("[Errno 21] Is a directory: {name}"),
        _ => anyhow!("{error}: {name}"),
    }
}

fn profile_enabled(config: &toml::Table) -> Result<bool> {
    match config
        .get("extra")
        .and_then(|extra| extra.get("profile_enabled"))
    {
        None => Ok(false),
        Some(toml::Value::Boolean(value)) => Ok(*value),
        Some(_) => bail!("extra.profile_enabled must be true or false"),
    }
}

fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}

fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                if !matches!(
                    result.components().next_back(),
                    Some(Component::RootDir | Component::Prefix(_)) | None
                ) {
                    result.pop();
                }
            }
            Component::CurDir => {}
            other => result.push(other),
        }
    }
    result
}

/// Python `Path.resolve()`: absolute, normalized, with the existing part
/// following links and taking its on-disk spelling.
fn resolve(path: &Path) -> PathBuf {
    let absolute = normalize(&std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()));
    let mut existing = absolute.clone();
    let mut missing = Vec::new();
    loop {
        if fs::symlink_metadata(&existing).is_ok() {
            if let Ok(real) = fs::canonicalize(&existing) {
                let mut result = strip_verbatim(real);
                for part in missing.iter().rev() {
                    result.push(part);
                }
                return result;
            }
            return absolute;
        }
        match (
            existing.file_name().map(|n| n.to_os_string()),
            existing.parent(),
        ) {
            (Some(name), Some(parent)) => {
                missing.push(name);
                existing = parent.to_path_buf();
            }
            _ => return absolute,
        }
    }
}

/// `public / text` with pathlib's rules for rooted and drive-qualified text.
fn join_like_pathlib(base: &Path, text: &str) -> PathBuf {
    let separators: &[char] = if cfg!(windows) { &['/', '\\'] } else { &['/'] };
    let mut chars = text.chars();
    let first = chars.next();
    let second = chars.next();
    if cfg!(windows) && second == Some(':') {
        return PathBuf::from(text);
    }
    match first {
        Some(c) if separators.contains(&c) => {
            if second.is_some_and(|c| separators.contains(&c)) {
                return PathBuf::from(text);
            }
            let mut root = PathBuf::new();
            for component in base.components() {
                if let Component::Prefix(prefix) = component {
                    root.push(prefix.as_os_str());
                }
            }
            root.push(text);
            root
        }
        _ => base.join(text),
    }
}

/// A comparable key: Windows paths compare without regard to case.
fn path_key(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

fn relative_to(path: &Path, base: &Path) -> Option<PathBuf> {
    let base_parts: Vec<String> = base.components().map(|c| key_of(c.as_os_str())).collect();
    let parts: Vec<Component> = path.components().collect();
    if parts.len() < base_parts.len()
        || parts
            .iter()
            .zip(&base_parts)
            .any(|(part, base)| key_of(part.as_os_str()) != *base)
    {
        return None;
    }
    Some(parts[base_parts.len()..].iter().collect())
}

fn key_of(part: &std::ffi::OsStr) -> String {
    let text = part.to_string_lossy().to_string();
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

/// `base / "a/b"` as pathlib spells it, with native separators.
fn join_posix(base: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(base.to_path_buf(), |path, part| path.join(part))
}

fn posix(relative: &Path) -> String {
    relative
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn native(relative: &Path) -> String {
    relative.display().to_string()
}

fn entries(directory: &Path) -> Vec<(PathBuf, bool, String)> {
    let Ok(reader) = fs::read_dir(directory) else {
        return Vec::new();
    };
    reader
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
            (
                entry.path(),
                is_dir,
                entry.file_name().to_string_lossy().to_string(),
            )
        })
        .collect()
}

/// `Path.rglob` in Python 3.13's order: a directory's matches are listed when
/// its parent is scanned, and directories are descended last-in, first-out.
fn rglob(root: &Path, matches: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = entries(root)
        .into_iter()
        .filter(|(_, _, name)| matches(name))
        .map(|(path, _, _)| path)
        .collect();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for (path, is_dir, _) in entries(&directory) {
            if is_dir {
                found.extend(
                    entries(&path)
                        .into_iter()
                        .filter(|(_, _, name)| matches(name))
                        .map(|(p, _, _)| p),
                );
                stack.push(path);
            }
        }
    }
    found
}

fn is_html(name: &str) -> bool {
    if cfg!(windows) {
        name.to_lowercase().ends_with(".html")
    } else {
        name.ends_with(".html")
    }
}

struct Validation<'a> {
    public: &'a Path,
    base: url::Split,
    errors: Vec<String>,
}

impl Validation<'_> {
    fn require(&mut self, condition: bool, message: impl FnOnce() -> String) {
        if !condition {
            self.errors.push(message());
        }
    }

    /// The output file a site URL points to, recording why when it does not.
    fn local_target(&mut self, url: &str, context: &str) -> Result<Option<PathBuf>> {
        let parts = urlsplit(url, "")?;
        if parts.netloc != self.base.netloc || !matches!(parts.scheme.as_str(), "http" | "https") {
            return Ok(None);
        }
        let Some(rest) = parts.path.strip_prefix(&self.base.path) else {
            self.errors
                .push(format!("{context}: escapes site prefix: {url}"));
            return Ok(None);
        };
        let mut local = resolve(&join_like_pathlib(self.public, &unquote(rest)));
        if relative_to(&local, self.public).is_none() {
            self.errors
                .push(format!("{context}: escapes output directory: {url}"));
            return Ok(None);
        }
        if local.is_dir() {
            local.push("index.html");
        }
        let exists = local.is_file();
        self.require(exists, || format!("{context}: missing target: {url}"));
        Ok(Some(local))
    }

    fn read_json(&mut self, relative: &str) -> Json {
        let path = join_posix(self.public, relative);
        let parsed = read_utf8(&path, true)
            .and_then(|text| pyjson::loads(&text).map_err(anyhow::Error::from));
        match parsed {
            Ok(value) => value,
            Err(error) => {
                self.errors
                    .push(format!("{relative}: cannot read JSON: {error}"));
                Json::Array(Vec::new())
            }
        }
    }
}

/// Return build counts, or fail with every problem found, one per line.
///
/// Site inputs, output directory and the build's base URL are explicit so a
/// generated starter or a preview-prefix build can be checked independently.
pub fn validate_site(
    site: &Path,
    output: Option<&Path>,
    base_url: Option<&str>,
) -> Result<CheckSummary> {
    let site = resolve(site);
    let public = resolve(&output.map_or_else(|| site.join("public"), Path::to_path_buf));
    let config: toml::Table = read_utf8(&site.join("config.toml"), true)?.parse()?;
    let enabled = profile_enabled(&config)?;
    let configured = match base_url.filter(|b| !b.is_empty()) {
        Some(base) => base.to_string(),
        None => match config.get("base_url") {
            Some(toml::Value::String(base)) => base.clone(),
            _ => bail!("KeyError: 'base_url'"),
        },
    };
    let base = format!("{}/", configured.trim_end_matches('/'));
    let base_parts = urlsplit(&base, "")?;
    if !matches!(base_parts.scheme.as_str(), "http" | "https") || base_parts.netloc.is_empty() {
        bail!("The validation base URL must be an absolute http(s) URL");
    }
    if !public.is_dir() {
        bail!("Build output is missing: {}", public.display());
    }
    let mut pages: Vec<(PathBuf, Page)> = Vec::new();
    for path in rglob(&public, is_html) {
        let relative = posix(&relative_to(&path, &public).unwrap_or_default());
        if JSON_ROUTES.contains(&relative.as_str()) {
            continue;
        }
        let page = parse_page(&read_utf8(&path, false)?);
        pages.push((path, page));
    }
    let lookup: HashMap<String, usize> = pages
        .iter()
        .enumerate()
        .map(|(i, (path, _))| (path_key(path), i))
        .collect();
    let mut check = Validation {
        public: &public,
        base: base_parts,
        errors: Vec::new(),
    };
    let mut checked = 0;

    for (path, page) in &pages {
        let relative = posix(&relative_to(path, &public).unwrap_or_default());
        let page_url = urljoin(
            &base,
            relative.strip_suffix("index.html").unwrap_or(&relative),
        )?;
        for link in &page.links {
            let absolute = urljoin(&page_url, link)?;
            let parts = urlsplit(&absolute, "")?;
            if parts.netloc != check.base.netloc
                || !matches!(parts.scheme.as_str(), "http" | "https")
            {
                continue;
            }
            let local = check.local_target(&absolute, &relative)?;
            if !parts.fragment.is_empty() {
                if let Some(target) = local.and_then(|local| lookup.get(&path_key(&local)).copied())
                {
                    let found = pages[target].1.ids.contains(&unquote(&parts.fragment));
                    check.require(found, || format!("{relative}: missing anchor: {link}"));
                }
            }
            checked += 1;
        }
    }

    let index = check.read_json("search-index/index.html");
    let graph = check.read_json("graph-data/index.html");
    check.require(matches!(index, Json::Array(_)), || {
        "Search index must contain an array".into()
    });
    check.require(matches!(graph, Json::Array(_)), || {
        "Graph data must contain an array".into()
    });
    let index = match index {
        Json::Array(items) => items,
        _ => Vec::new(),
    };
    let graph = match graph {
        Json::Array(items) => items,
        _ => Vec::new(),
    };
    for post in &index {
        let (Some(title), Some(url)) = (
            post.get("title").and_then(Json::as_str),
            post.get("url").and_then(Json::as_str),
        ) else {
            check
                .errors
                .push("Search index contains an invalid article".into());
            continue;
        };
        let target = check.local_target(url, &format!("Search article {title}"))?;
        check.require(target.is_some(), || {
            format!("Search article URL must belong to the site: {url}")
        });
    }
    let mut graph_ids: Vec<String> = Vec::new();
    for post in &graph {
        let (Some(id), Some(url)) = (
            post.get("id").and_then(Json::as_str),
            post.get("url").and_then(Json::as_str),
        ) else {
            check
                .errors
                .push("Graph data contains an invalid article".into());
            continue;
        };
        let duplicate = graph_ids.iter().any(|seen| seen == id);
        check.require(!duplicate, || format!("Duplicate graph article: {id}"));
        graph_ids.push(id.to_string());
        check.require(post.get("draft") == Some(&Json::Bool(false)), || {
            format!("Draft leaked into graph: {id}")
        });
        let target = check.local_target(url, &format!("Graph article {id}"))?;
        check.require(target.is_some(), || {
            format!("Graph article URL must belong to the site: {url}")
        });
        let valid = matches!(post.get("html"), Some(Json::Str(_)))
            && matches!(post.get("headings"), Some(Json::Array(_)));
        check.require(valid, || {
            format!("Graph article body/headings are invalid: {id}")
        });
    }

    for relative in REQUIRED_ROUTES {
        let exists = join_posix(&public, relative).is_file();
        check.require(exists, || {
            format!("Required public route is missing: {relative}")
        });
    }
    if enabled {
        let profile = match read_utf8(&join_posix(&site, "data/profile.json"), true)
            .and_then(|text| pyjson::loads(&text).map_err(anyhow::Error::from))
        {
            Ok(profile) => profile,
            Err(error) => {
                check.errors.push(format!(
                    "Profile is enabled but generated metadata is missing/invalid: {error}"
                ));
                Json::Object(Vec::new())
            }
        };
        let about_path = public.join("about").join("index.html");
        let about = lookup.get(&path_key(&about_path)).copied();
        check.require(about.is_some(), || {
            "Profile is enabled but /about/ is missing".into()
        });
        if let Some(about) = about {
            let mut links_resume = false;
            for link in &pages[about].1.links {
                if urlsplit(link, "")?.path.ends_with("/resume.pdf") {
                    links_resume = true;
                    break;
                }
            }
            check.require(links_resume, || {
                "Profile must link to the public resume".into()
            });
            let shown = match profile.get("display_name") {
                Some(Json::Str(name)) if !name.is_empty() => {
                    unescape(&read_utf8(&pages[about].0, false)?).contains(name.as_str())
                }
                _ => false,
            };
            check.require(shown, || {
                "About page must display the generated profile name".into()
            });
        }
        let resume = public.join("resume.pdf");
        let valid =
            resume.is_file() && fs::read(&resume).is_ok_and(|bytes| bytes.starts_with(b"%PDF-"));
        check.require(valid, || {
            "Profile is enabled but the public resume is missing/invalid".into()
        });
    }

    for path in rglob(&public, |_| true) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let leaked = PRIVATE_NAMES.contains(&name.as_str()) || name.starts_with(".env");
        let relative = native(&relative_to(&path, &public).unwrap_or_default());
        check.require(!leaked, || {
            format!("Private build input leaked into public output: {relative}")
        });
    }
    let profile_dir = fs::symlink_metadata(public.join("profile")).is_ok();
    check.require(!profile_dir, || {
        "Resume source and private contacts must stay outside published files".into()
    });
    let agent_configured = config
        .get("extra")
        .and_then(|extra| extra.get("agent_endpoint"))
        .is_some_and(|endpoint| match endpoint {
            toml::Value::String(s) => !s.is_empty(),
            toml::Value::Boolean(b) => *b,
            toml::Value::Integer(i) => *i != 0,
            toml::Value::Float(f) => *f != 0.0,
            toml::Value::Array(a) => !a.is_empty(),
            toml::Value::Table(t) => !t.is_empty(),
            toml::Value::Datetime(_) => true,
        });
    for (path, _) in &pages {
        let text = read_utf8(path, false)?;
        let relative = native(&relative_to(path, &public).unwrap_or_default());
        check.require(!text.contains("id=\"series-editor-script\""), || {
            format!("{relative}: local editing is enabled on the public site")
        });
        check.require(
            !text.contains("http://localhost") && !text.contains("http://127.0.0.1"),
            || format!("{relative}: local preview URL leaked into public output"),
        );
        check.require(
            !text.contains("Zotero-API-Key") && !text.contains("\"annotationPosition\""),
            || format!("{relative}: private Zotero records leaked into public output"),
        );
        if !agent_configured {
            check.require(!text.contains("id=\"agent-panel\""), || {
                format!("{relative}: an unconfigured Agent panel is enabled")
            });
        }
    }
    if !check.errors.is_empty() {
        bail!("{}", check.errors.join("\n"));
    }
    Ok(CheckSummary {
        pages: pages.len(),
        links: checked,
        articles: index.len(),
    })
}

#[cfg(test)]
mod tests;
