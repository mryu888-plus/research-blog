//! The optional article assistant: a Tantivy index of the articles and a
//! local chat API that answers from it.

use anyhow::Result;
use blog_agent::{create_app, llm::LlmConfig};
use blog_build::indexer::Indexer;
use clap::Subcommand;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub enum AgentCommand {
    /// 为已发布的文章建搜索索引，写到 output/index/
    Index,
    /// 开本地问答服务；需要先建索引，模型由 LLM_ENDPOINT、LLM_API_KEY、LLM_MODEL 配置
    Serve {
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
}

pub fn run(root: &Path, command: AgentCommand) -> Result<()> {
    dotenvy::from_path(root.join(".env")).ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    match command {
        AgentCommand::Index => index(root),
        AgentCommand::Serve { port } => tokio::runtime::Runtime::new()?.block_on(serve(root, port)),
    }
}

fn articles(directory: &Path, found: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            articles(&path, found)?;
        } else if path.file_name().is_some_and(|name| name == "index.md") {
            found.push(path);
        }
    }
    Ok(())
}

fn index(root: &Path) -> Result<()> {
    let index_dir = root.join("output/index");
    let manifests = index_dir.join("manifests");
    fs::create_dir_all(&manifests)?;
    for entry in fs::read_dir(&manifests)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            fs::remove_file(path)?;
        }
    }
    let mut paths = Vec::new();
    articles(&root.join("site/content"), &mut paths)?;
    paths.sort();
    let mut indexer = Indexer::new(&index_dir)?;
    indexer.clear()?;
    let mut count = 0;
    for path in paths {
        let content = fs::read_to_string(&path)?;
        if blog_ir::parser::article_is_draft(&path, &content)? {
            continue;
        }
        let ir = blog_ir::parser::parse_article(&path, &content)?;
        indexer.index_article(&ir)?;
        fs::write(
            manifests.join(format!("{}.json", ir.metadata.slug)),
            serde_json::to_string_pretty(&ir)?,
        )?;
        count += 1;
    }
    indexer.commit()?;
    println!("已为 {count} 篇文章建索引：{}", index_dir.display());
    Ok(())
}

async fn serve(root: &Path, port: u16) -> Result<()> {
    let config = LlmConfig::from_env();
    if config.is_none() {
        eprintln!("没有配置模型。设置 LLM_MODEL 和 LLM_API_KEY 后才能回答问题。");
    }
    let app = create_app(root.join("output/index"), config).await?;
    let address = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("文章助手：http://{address}/api/chat");
    axum::serve(listener, app).await?;
    Ok(())
}
