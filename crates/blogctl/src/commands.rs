use anyhow::Result;
use blog_agent::{create_app, llm::LlmConfig};
use blog_build::{indexer::Indexer, Builder};
use std::fs;
use std::path::PathBuf;

pub fn new_article(slug: &str) -> Result<()> {
    if slug.is_empty()
        || slug.starts_with('-')
        || slug.ends_with('-')
        || !slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        anyhow::bail!("Use a slug containing only lowercase letters, digits and internal hyphens.");
    }
    let article_dir = PathBuf::from("site/content/posts").join(slug);

    if article_dir.exists() {
        anyhow::bail!("Article already exists: {}", article_dir.display());
    }

    fs::create_dir_all(&article_dir)?;
    fs::create_dir_all(article_dir.join("figures"))?;

    let now = chrono::Utc::now().format("%Y-%m-%d");
    let title = slug.replace('-', " ");

    let template = format!(
        r#"+++
title = "{}"
date = "{}"
description = ""
draft = true
[taxonomies]
tags = []
+++

# Introduction

Write your content here.

Inline math: $x^2 + y^2 = z^2$

Display math:

$$
\int_0^\infty e^{{-x^2}} dx = \frac{{\sqrt{{\pi}}}}{{2}}
$$

## Example Figure

{{{{ <fig id="example" caption="Example figure caption" alt="Example figure description" /> }}}}

## Code Example

```rust
fn main() {{
    println!("Hello, world!");
}}
```
"#,
        title, now
    );

    fs::write(article_dir.join("index.md"), template)?;

    let typst_template = r#"#import "@preview/cetz:0.4.2"

#set page(width: auto, height: auto, margin: 1em)

#cetz.canvas({
  import cetz.draw: *
  rect((0, 0), (4, 3), stroke: 2pt + blue)
  content((2, 1.5), [Example])
})
"#;

    fs::write(article_dir.join("figures/example.typ"), typst_template)?;

    tracing::info!("Created article: {}", article_dir.display());

    Ok(())
}

pub async fn build() -> Result<()> {
    let site_dir = PathBuf::from("site");
    let output_dir = PathBuf::from("output");

    fs::create_dir_all(&output_dir)?;

    let mut builder = Builder::new(site_dir, output_dir.clone())?;

    tracing::info!("Building blog...");
    let irs = builder.build_all()?;

    tracing::info!("Built {} articles", irs.len());

    let index_dir = output_dir.join("index");
    fs::create_dir_all(&index_dir)?;

    let mut indexer = Indexer::new(&index_dir)?;
    indexer.clear()?;

    let manifests_dir = index_dir.join("manifests");
    fs::create_dir_all(&manifests_dir)?;
    for entry in fs::read_dir(&manifests_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            fs::remove_file(path)?;
        }
    }

    for ir in &irs {
        indexer.index_article(ir)?;
        let manifest_json = serde_json::to_string_pretty(ir)?;
        fs::write(
            manifests_dir.join(format!("{}.json", ir.metadata.slug)),
            manifest_json,
        )?;
    }

    indexer.commit()?;

    tracing::info!("Build complete");

    Ok(())
}

pub fn check() -> Result<()> {
    let site_dir = PathBuf::from("site");
    let output_dir = PathBuf::from("output");

    let builder = Builder::new(site_dir, output_dir)?;

    tracing::info!("Checking tools...");
    builder.check_tools()?;

    tracing::info!("✓ Typst found");
    tracing::info!("✓ Zola found");

    if let Some(config) = LlmConfig::from_env() {
        tracing::info!("✓ LLM configured: model={}", config.model);
    } else {
        tracing::warn!("⚠ LLM not configured");
    }

    Ok(())
}

pub fn index() -> Result<()> {
    let output_dir = PathBuf::from("output");
    let index_dir = output_dir.join("index");

    fs::create_dir_all(&index_dir)?;

    let manifests_dir = index_dir.join("manifests");
    if !manifests_dir.exists() {
        anyhow::bail!("No manifests found. Run 'blogctl build' first.");
    }

    let mut indexer = Indexer::new(&index_dir)?;
    indexer.clear()?;
    let mut count = 0;

    for entry in fs::read_dir(manifests_dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
            let content = fs::read_to_string(entry.path())?;
            let ir: blog_ir::ArticleIR = serde_json::from_str(&content)?;
            indexer.index_article(&ir)?;
            count += 1;
        }
    }

    indexer.commit()?;

    tracing::info!("Indexed {} articles", count);

    Ok(())
}

pub async fn serve(port: u16) -> Result<()> {
    tracing::info!("Starting server on port {}", port);

    let llm_config = LlmConfig::from_env();
    if llm_config.is_none() {
        tracing::warn!("LLM not configured. Set LLM_MODEL and LLM_API_KEY.");
    }

    let index_dir = PathBuf::from("output/index");
    let app = create_app(index_dir, llm_config).await?;

    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("Listening on http://{}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_path_traversal_and_invalid_slugs_before_writing() {
        for slug in [
            "../escape",
            "a/b",
            "a\\b",
            "",
            "-bad",
            "bad-",
            "Bad",
            "a\"b",
        ] {
            assert!(new_article(slug).is_err(), "accepted {slug}");
        }
    }
}
