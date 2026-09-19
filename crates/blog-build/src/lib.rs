use anyhow::{Context, Result};
use blog_ir::{parser::parse_article, ArticleIR};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("Typst not found. Install from https://typst.app")]
    TypstNotFound,

    #[error("Typst compilation failed for {path}: {stderr}")]
    TypstCompilationError { path: String, stderr: String },

    #[error("Zola not found. Install from https://getzola.org")]
    ZolaNotFound,

    #[error("Zola build failed: {0}")]
    ZolaBuildError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// 构建缓存，存储已编译的 Typst 文件哈希
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BuildCache {
    pub typst_hashes: HashMap<String, String>,
}

impl BuildCache {
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}

/// 主构建器
pub struct Builder {
    pub site_dir: PathBuf,
    pub output_dir: PathBuf,
    pub cache: BuildCache,
    pub cache_path: PathBuf,
}

impl Builder {
    pub fn new(site_dir: PathBuf, output_dir: PathBuf) -> Result<Self> {
        let cache_path = output_dir.join(".build-cache.json");
        let cache = BuildCache::load(&cache_path).unwrap_or_default();

        Ok(Self {
            site_dir,
            output_dir,
            cache,
            cache_path,
        })
    }

    /// 检查必要工具是否安装
    pub fn check_tools(&self) -> Result<(), BuildError> {
        if !self.is_command_available("typst") {
            return Err(BuildError::TypstNotFound);
        }
        if !self.is_command_available("zola") {
            return Err(BuildError::ZolaNotFound);
        }
        Ok(())
    }

    fn is_command_available(&self, cmd: &str) -> bool {
        Command::new(cmd)
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// 扫描所有文章
    pub fn scan_articles(&self) -> Result<Vec<PathBuf>> {
        let content_dir = self.site_dir.join("content");
        let mut articles = Vec::new();

        for entry in WalkDir::new(&content_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_name() == "index.md" {
                articles.push(entry.path().to_path_buf());
            }
        }

        Ok(articles)
    }

    /// 处理单篇文章
    pub fn process_article(&mut self, article_path: &Path) -> Result<ArticleIR> {
        tracing::info!("Processing article: {}", article_path.display());

        let content = fs::read_to_string(article_path)
            .with_context(|| format!("Failed to read {}", article_path.display()))?;

        let ir = parse_article(article_path, &content)
            .with_context(|| format!("Failed to parse {}", article_path.display()))?;

        let article_dir = article_path.parent().with_context(|| {
            format!(
                "Article has no parent directory: {}",
                article_path.display()
            )
        })?;

        for figure in &ir.figures {
            let typst_path = article_dir.join(&figure.typst_source);
            if typst_path.exists() {
                self.compile_typst(&typst_path, article_dir)?;
            } else {
                anyhow::bail!("Typst source missing: {}", typst_path.display());
            }
        }

        let manifest_path = article_dir.join("article-manifest.json");
        let ir_json = serde_json::to_string_pretty(&ir)?;
        fs::write(manifest_path, ir_json)?;

        Ok(ir)
    }

    /// 增量编译 Typst 到 SVG
    pub fn compile_typst(&mut self, typst_path: &Path, output_dir: &Path) -> Result<()> {
        let content = fs::read_to_string(typst_path)?;
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = hex::encode(hasher.finalize());

        let cache_key = typst_path.display().to_string();

        tracing::info!("Compiling Typst: {}", typst_path.display());

        let file_stem = typst_path
            .file_stem()
            .and_then(|s| s.to_str())
            .with_context(|| format!("Invalid file name: {}", typst_path.display()))?;
        let svg_path = output_dir.join(format!("{}.svg", file_stem));

        let output = Command::new("typst")
            .arg("compile")
            .arg(typst_path)
            .arg(&svg_path)
            .arg("--format")
            .arg("svg")
            .output()
            .map_err(|_| BuildError::TypstNotFound)?;

        if !output.status.success() {
            return Err(BuildError::TypstCompilationError {
                path: typst_path.display().to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            }
            .into());
        }

        self.cache.typst_hashes.insert(cache_key, hash);
        self.cache.save(&self.cache_path)?;

        Ok(())
    }

    /// 调用 Zola 构建
    pub fn build_zola(&self) -> Result<()> {
        // Reuse the same profile export as the editor and Pages build. Resolve
        // from the selected site, not the caller's working directory.
        let site = self.site_dir.canonicalize()?;
        if let Some(root) = site.parent() {
            if root.join("profile/cv.typ").is_file() {
                let python = std::env::var("PYTHON").unwrap_or_else(|_| {
                    if cfg!(windows) { "python" } else { "python3" }.to_owned()
                });
                let status = Command::new(python)
                    .arg(root.join("scripts/build_profile.py"))
                    .current_dir(root)
                    .status()
                    .context("Could not run the profile exporter; install Python 3.11+")?;
                anyhow::ensure!(status.success(), "Profile export failed; Zola was not run");
            }
        }
        tracing::info!("Running Zola build");

        let output = Command::new("zola")
            .arg("build")
            .current_dir(&self.site_dir)
            .output()
            .map_err(|_| BuildError::ZolaNotFound)?;

        if !output.status.success() {
            return Err(BuildError::ZolaBuildError(
                String::from_utf8_lossy(&output.stderr).to_string(),
            )
            .into());
        }

        Ok(())
    }

    /// 完整构建流程
    pub fn build_all(&mut self) -> Result<Vec<ArticleIR>> {
        self.check_tools()?;

        let articles = self.scan_articles()?;
        tracing::info!("Found {} articles", articles.len());

        let mut irs = Vec::new();
        for article_path in articles {
            if blog_ir::parser::article_is_draft(
                &article_path,
                &fs::read_to_string(&article_path)?,
            )? {
                continue;
            }
            let ir = self.process_article(&article_path)?;
            irs.push(ir);
        }

        self.build_zola()?;

        Ok(irs)
    }
}

pub mod indexer;
