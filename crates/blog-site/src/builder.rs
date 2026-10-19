//! Build the public static blog with Zola 0.23.4 and Typst 0.15.1.

use crate::paths::absolute;
use crate::prepare::PreparedSite;
use crate::references::parse_frontmatter;
use crate::{profile, site, tools, zen};
use anyhow::{bail, Context, Result};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

static FIGURE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<fig\s+[^>]*?id="([a-zA-Z0-9_-]+)""#).unwrap());

/// A directory under `parent` that is removed when dropped.
pub struct ScratchDir(PathBuf);

impl ScratchDir {
    pub fn new(parent: &Path, prefix: &str) -> Result<Self> {
        fs::create_dir_all(parent)?;
        let parent = absolute(parent)?;
        for attempt in 0u32.. {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .subsec_nanos();
            let path = parent.join(format!(
                "{prefix}{}-{nanos:08x}-{attempt}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(ScratchDir(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        unreachable!()
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Run a tool, failing with its name when it cannot start or exits non-zero.
pub fn run(command: &mut Command) -> Result<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    let status = command
        .status()
        .with_context(|| format!("无法运行 {program}"))?;
    if !status.success() {
        bail!(
            "{program} 退出，错误码 {}",
            status.code().map_or("未知".into(), |code| code.to_string())
        );
    }
    Ok(())
}

/// Every Typst figure referenced by a published article: (source, output).
pub fn figures(site: &Path, include_drafts: bool) -> Result<Vec<(PathBuf, PathBuf)>> {
    let mut articles = Vec::new();
    collect_articles(&site.join("content").join("posts"), &mut articles)?;
    articles.sort();
    let mut result = Vec::new();
    for article in articles {
        let text = crate::references::read_utf8_sig(&article)?;
        let (metadata, _) = parse_frontmatter(&text)
            .with_context(|| format!("无法读取文章开头的设置：{}", article.display()))?;
        if !include_drafts && crate::references::truthy(metadata.get("draft")) {
            continue;
        }
        let folder = article.parent().unwrap();
        for found in FIGURE.captures_iter(&text) {
            let id = &found[1];
            result.push((
                folder.join("figures").join(format!("{id}.typ")),
                folder.join(format!("{id}.svg")),
            ));
        }
    }
    Ok(result)
}

fn collect_articles(directory: &Path, articles: &mut Vec<PathBuf>) -> Result<()> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_articles(&path, articles)?;
        } else if entry.file_name() == "index.md" {
            articles.push(path);
        }
    }
    Ok(())
}

pub fn typst_figure(
    typst: &Path,
    root: &Path,
    source: &Path,
    output: &Path,
    watch: bool,
) -> Command {
    let mut command = Command::new(typst);
    command
        .arg(if watch { "watch" } else { "compile" })
        .arg(source)
        .arg(output)
        .arg("--root")
        .arg(root)
        .arg("--format")
        .arg("svg")
        .current_dir(root);
    command
}

pub fn compile_figures(site: &Path, root: &Path) -> Result<()> {
    let figures = figures(site, false)?;
    if figures.is_empty() {
        return Ok(());
    }
    let typst = tools::tool_path("typst", root)?;
    for (source, output) in figures {
        run(&mut typst_figure(&typst, root, &source, &output, false))
            .with_context(|| format!("图形编译失败：{}", source.display()))?;
    }
    Ok(())
}

/// Build protected input without ever rewriting an author's Markdown.
pub fn render_site(site: &Path, base_url: Option<&str>, root: &Path) -> Result<()> {
    let site = absolute(site)?;
    let zola = tools::tool_path("zola", root)?;
    let scratch = ScratchDir::new(&root.join(".tools"), "math-build-")?;
    let mut prepared = PreparedSite::new(&site, &scratch.path().join("site"))?;
    prepared.sync()?;
    let public = site.join("public");
    let mut command = Command::new(&zola);
    command
        .arg("--root")
        .arg(&prepared.target)
        .arg("build")
        .arg("--output-dir")
        .arg(&public)
        .arg("--force");
    if let Some(base_url) = base_url {
        command.arg("--base-url").arg(base_url);
    }
    run(&mut command)?;
    // Article manifests contain local source paths and are only needed by the Agent.
    remove_named(&public, "article-manifest.json")?;
    fs::write(public.join(".nojekyll"), b"")?;
    Ok(())
}

fn remove_named(directory: &Path, name: &str) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            remove_named(&entry.path(), name)?;
        } else if entry.file_name() == name {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

pub struct BuildOptions<'a> {
    pub base_url: Option<&'a str>,
    pub skip_figures: bool,
}

pub fn build(root: &Path, options: &BuildOptions) -> Result<()> {
    let site = root.join("site");
    let enabled = site::profile_enabled(&site::read_config(&site)?)?;
    if enabled {
        site::require_profile_source(root)?;
    }
    zen::sync(root, false)?;
    if enabled {
        profile::build_profile(root, true)?;
    }
    if !options.skip_figures {
        compile_figures(&site, root)?;
    }
    render_site(&site, options.base_url, root)
}
