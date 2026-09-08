use serde::{Deserialize, Serialize};

/// 文章中间表示，统一供构建、检索和 Agent 使用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleIR {
    pub metadata: Metadata,
    pub sections: Vec<Section>,
    pub equations: Vec<Equation>,
    pub figures: Vec<Figure>,
    pub code_blocks: Vec<CodeBlock>,
    pub citations: Vec<Citation>,
    pub plain_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub title: String,
    pub slug: String,
    pub date: String,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub language: String,
    pub tags: Vec<String>,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub id: String,
    pub anchor: String,
    pub level: u8,
    pub title: String,
    pub content: String,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Equation {
    pub id: String,
    pub source: String,
    pub display: bool,
    pub section_id: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Figure {
    pub id: String,
    pub typst_source: String,
    pub caption: String,
    pub alt_text: String,
    pub section_id: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlock {
    pub id: String,
    pub language: Option<String>,
    pub source: String,
    pub section_id: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub key: String,
    pub section_id: String,
    pub line: usize,
}

/// 解析错误
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Failed to read source file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid frontmatter in {path}: {message}")]
    FrontmatterError { path: String, message: String },

    #[error("Missing required metadata field '{field}' in {path}")]
    MissingMetadata { path: String, field: String },
}

pub mod parser;
