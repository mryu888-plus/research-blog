//! Create an independent research site from the files embedded in blogctl.

use crate::paths::{absolute, is_link};
use anyhow::{bail, Result};
use regex::Regex;
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

mod starter {
    include!(concat!(env!("OUT_DIR"), "/starter_files.rs"));
}

/// Every embedded file, as (relative path, contents).
pub fn starter_files() -> &'static [(&'static str, &'static [u8])] {
    starter::FILES
}

/// Treat Windows junctions and other reparse points like symlinks.
fn check_path(path: &Path) -> Result<()> {
    for component in path.ancestors() {
        if component.as_os_str().is_empty() {
            break;
        }
        if let Ok(metadata) = fs::symlink_metadata(component) {
            if is_link(&metadata) {
                bail!(
                    "符号链接或 junction 不能用于初始化：{}",
                    component.display()
                );
            }
        }
    }
    Ok(())
}

fn text(value: &str, label: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().any(|c| (c as u32) < 32) {
        bail!("{label}不能为空或包含控制字符。");
    }
    if value.chars().count() > 200 {
        bail!("{label}不能超过 200 个字符。");
    }
    Ok(value.to_string())
}

/// Python `urllib.parse.unquote`: percent-decoding as UTF-8 with replacement.
fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && bytes[index + 1].is_ascii_hexdigit()
            && bytes[index + 2].is_ascii_hexdigit()
        {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap();
            output.push(u8::from_str_radix(hex, 16).unwrap());
            index += 3;
            continue;
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

static LABEL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\z").unwrap());

/// Accept public HTTP(S) site roots, including a GitHub Pages subpath.
pub fn normalize_url(value: &str) -> Result<String> {
    if value.is_empty()
        || value
            .chars()
            .any(|c| crate::markdown::is_space(c) || (c as u32) < 32)
    {
        bail!("网站 URL 不能为空或包含空格、控制字符。");
    }
    if value.contains('\\') {
        bail!("网站 URL 不能包含反斜杠。");
    }
    let invalid = || anyhow::anyhow!("网站 URL 格式不正确。");
    let credentials = || anyhow::anyhow!("网站 URL 必须是无凭据、查询或片段的 HTTP(S) 地址。");
    let (scheme, rest) = value.split_once(':').ok_or_else(credentials)?;
    let scheme = scheme.to_ascii_lowercase();
    let after = rest.strip_prefix("//").ok_or_else(credentials)?;
    let netloc_end = after.find(['/', '?', '#']).unwrap_or(after.len());
    let netloc = &after[..netloc_end];
    let mut path = &after[netloc_end..];
    let (mut query, mut fragment) = ("", "");
    if let Some(hash) = path.find('#') {
        fragment = &path[hash + 1..];
        path = &path[..hash];
    }
    if let Some(question) = path.find('?') {
        query = &path[question + 1..];
        path = &path[..question];
    }
    if netloc.contains('[') != netloc.contains(']') {
        return Err(invalid());
    }
    let (userinfo, hostinfo) = match netloc.rfind('@') {
        Some(at) => (Some(&netloc[..at]), &netloc[at + 1..]),
        None => (None, netloc),
    };
    let (hostname, port) = match hostinfo.strip_prefix('[') {
        Some(bracketed) => {
            let (host, tail) = bracketed.split_once(']').ok_or_else(invalid)?;
            (host.to_string(), tail.strip_prefix(':'))
        }
        None => match hostinfo.split_once(':') {
            Some((host, port)) => (host.to_string(), Some(port)),
            None => (hostinfo.to_string(), None),
        },
    };
    if let Some(port) = port {
        if port.is_empty()
            || !port.bytes().all(|b| b.is_ascii_digit())
            || port.parse::<u32>().map_or(true, |p| p > 65535)
        {
            return Err(invalid());
        }
    }
    let hostname = hostname.to_lowercase();
    if !["http", "https"].contains(&scheme.as_str())
        || hostname.is_empty()
        || userinfo.is_some()
        || !query.is_empty()
        || !fragment.is_empty()
    {
        return Err(credentials());
    }
    if hostname.parse::<IpAddr>().is_err() {
        let ascii = idna::domain_to_ascii(&hostname)
            .map_err(|_| anyhow::anyhow!("网站 URL 的域名格式不正确。"))?;
        if ascii.split('.').any(|label| !LABEL.is_match(label)) {
            bail!("网站 URL 的域名格式不正确。");
        }
    }
    let mut decoded = path.to_string();
    for _ in 0..5 {
        let next = unquote(&decoded);
        if next == decoded {
            break;
        }
        decoded = next;
    }
    if decoded != unquote(&decoded) {
        bail!("网站 URL 路径包含过多嵌套转义。");
    }
    if decoded.contains('\\')
        || decoded.chars().any(|c| (c as u32) < 32)
        || decoded
            .split('/')
            .any(|segment| segment == "." || segment == "..")
    {
        bail!("网站 URL 路径不能包含目录跳转或控制字符。");
    }
    let path = format!("{}/", path.trim_end_matches('/'));
    let path = if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    };
    Ok(format!("{scheme}://{netloc}{path}"))
}

pub struct InitSummary {
    pub destination: PathBuf,
    pub name: String,
    pub url: String,
    pub author: String,
    pub files: usize,
    pub dry_run: bool,
}

impl InitSummary {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "destination": self.destination.display().to_string(),
            "name": self.name,
            "url": self.url,
            "author": self.author,
            "files": self.files,
            "dry_run": self.dry_run,
        })
    }
}

fn is_empty_dir(path: &Path) -> Result<bool> {
    Ok(path.is_dir() && fs::read_dir(path)?.next().is_none())
}

pub fn initialize(
    destination: &Path,
    name: &str,
    url: &str,
    author: &str,
    dry_run: bool,
) -> Result<InitSummary> {
    let name = text(name, "网站标题")?;
    let author = text(author, "作者名称")?;
    let url = normalize_url(url)?;
    let destination = absolute(destination)?;
    check_path(&destination)?;
    if destination.exists() && !is_empty_dir(&destination)? {
        bail!(
            "目标必须不存在或为空目录，不会覆盖已有文件：{}",
            destination.display()
        );
    }
    let files = starter_files();
    let summary = InitSummary {
        destination: destination.clone(),
        name: name.clone(),
        url: url.clone(),
        author: author.clone(),
        files: files.len(),
        dry_run,
    };
    if dry_run {
        return Ok(summary);
    }
    let parent = destination
        .parent()
        .ok_or_else(|| anyhow::anyhow!("目标目录无效。"))?;
    fs::create_dir_all(parent)?;
    check_path(parent)?;
    let stage = crate::builder::ScratchDir::new(parent, ".research-blog-init-")?;
    for (relative, data) in files {
        let target = relative
            .split('/')
            .fold(stage.path().to_path_buf(), |path, part| path.join(part));
        fs::create_dir_all(target.parent().unwrap())?;
        fs::write(&target, data)?;
    }
    let config = stage.path().join("site").join("config.toml");
    let content = fs::read_to_string(&config)?;
    let json = |value: &str| serde_json::to_string(value).unwrap();
    // The template placeholders occupy complete TOML values. Do one pass so
    // a user's literal placeholder-looking title is never interpolated.
    let placeholders = Regex::new("__SITE_TITLE__|__SITE_URL__|__SITE_AUTHOR__").unwrap();
    let content = placeholders.replace_all(&content, |found: &regex::Captures| match &found[0] {
        "__SITE_TITLE__" => json(&name),
        "__SITE_URL__" => json(&url),
        _ => json(&author),
    });
    fs::write(&config, content.as_bytes())?;
    check_path(&destination)?;
    if destination.exists() {
        if !is_empty_dir(&destination)? {
            bail!("初始化期间目标目录发生变化；没有覆盖已有文件。");
        }
        fs::remove_dir(&destination)?; // Remove only the verified empty directory.
    }
    fs::rename(stage.path(), &destination)?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_site_urls() {
        assert_eq!(
            normalize_url("https://example.org").unwrap(),
            "https://example.org/"
        );
        assert_eq!(
            normalize_url("https://user.github.io/notes").unwrap(),
            "https://user.github.io/notes/"
        );
        assert_eq!(
            normalize_url("http://127.0.0.1:8080/a//").unwrap(),
            "http://127.0.0.1:8080/a/"
        );
        assert_eq!(
            normalize_url("https://例子.测试/x").unwrap(),
            "https://例子.测试/x/"
        );
        for bad in [
            "",
            "ftp://x.y/",
            "https://u@x.y/",
            "https://x.y/?q",
            "https://x.y:/",
            "https://x.y/#f",
            "https://x.y/a/../b",
            "https://x.y/%252e%252e/",
            "https://x.y:99999/",
            "https://-bad-.y/",
            "https://x.y\\a",
            "https://x y/",
        ] {
            assert!(normalize_url(bad).is_err(), "accepted {bad}");
        }
    }

    #[test]
    fn embeds_the_starter() {
        let names: Vec<&str> = starter_files().iter().map(|(name, _)| *name).collect();
        for required in [
            "ZEN.md",
            "site/config.toml",
            "site/templates/base.html",
            "site/static/zen.css",
        ] {
            assert!(names.contains(&required), "missing {required}");
        }
        assert!(starter_files()
            .iter()
            .all(|(_, data)| !data.windows(2).any(|w| w == b"\r\n")));
    }

    #[test]
    fn initializes_once_and_never_overwrites() {
        let base = std::env::temp_dir().join(format!("blog-init-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let target = base.join("site");
        let summary = initialize(
            &target,
            "演示 \"站\"",
            "https://example.invalid/notes",
            "Researcher",
            false,
        )
        .unwrap();
        assert_eq!(summary.files, starter_files().len());
        let config = fs::read_to_string(target.join("site/config.toml")).unwrap();
        assert!(config.contains("\"演示 \\\"站\\\"\""));
        assert!(config.contains("\"https://example.invalid/notes/\""));
        assert!(initialize(&target, "x", "https://example.invalid/", "y", false).is_err());
        fs::remove_dir_all(&base).unwrap();
    }
}
