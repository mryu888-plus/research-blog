use anyhow::Result;
use blog_ir::{ArticleIR, Figure};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{Index, ReloadPolicy};
use tantivy_jieba::JiebaTokenizer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub article_id: String,
    pub article_title: String,
    pub section_id: String,
    pub section_title: String,
    pub section_anchor: String,
    pub content: String,
    pub score: f32,
}

pub struct Retriever {
    index: Index,
    schema: Schema,
    article_irs: HashMap<String, ArticleIR>,
}

impl Retriever {
    pub fn open(index_dir: PathBuf) -> Result<Self> {
        let index = Index::open_in_dir(&index_dir)?;
        // 必须与 blog-build 建索引时注册的分词器一致，否则中文查询会报 unknown tokenizer
        index.tokenizers().register("jieba", JiebaTokenizer {});
        let schema = index.schema();

        let article_irs = Self::load_article_irs(&index_dir)?;

        Ok(Self {
            index,
            schema,
            article_irs,
        })
    }

    fn load_article_irs(index_dir: &Path) -> Result<HashMap<String, ArticleIR>> {
        let mut map = HashMap::new();

        let manifests_path = index_dir.join("manifests");
        if !manifests_path.exists() {
            return Ok(map);
        }

        for entry in fs::read_dir(manifests_path)? {
            let entry = entry?;
            if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if let Ok(ir) = serde_json::from_str::<ArticleIR>(&content) {
                        map.insert(ir.metadata.slug.clone(), ir);
                    }
                }
            }
        }

        Ok(map)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        let searcher = reader.searcher();

        let content_field = self.schema.get_field("content")?;
        let title_field = self.schema.get_field("title")?;

        let query_parser = QueryParser::for_index(&self.index, vec![content_field, title_field]);
        let query = query_parser.parse_query(query)?;

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let article_id_field = self.schema.get_field("article_id")?;
        let section_id_field = self.schema.get_field("section_id")?;
        let section_title_field = self.schema.get_field("section_title")?;
        let section_anchor_field = self.schema.get_field("section_anchor")?;

        let mut results = Vec::new();

        for (score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;

            let article_id = retrieved_doc
                .get_first(article_id_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let article_title = retrieved_doc
                .get_first(title_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let section_id = retrieved_doc
                .get_first(section_id_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let section_title = retrieved_doc
                .get_first(section_title_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let section_anchor = retrieved_doc
                .get_first(section_anchor_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let content = retrieved_doc
                .get_first(content_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            results.push(SearchResult {
                article_id,
                article_title,
                section_id,
                section_title,
                section_anchor,
                content,
                score,
            });
        }

        Ok(results)
    }

    pub fn section_text(&self, article_id: &str, anchor: &str) -> Result<Option<String>> {
        if let Some(ir) = self.article_irs.get(article_id) {
            for section in &ir.sections {
                if section.anchor == anchor {
                    return Ok(Some(section.content.clone()));
                }
            }
        }
        Ok(None)
    }

    pub fn figure_info(&self, article_id: &str, figure_id: &str) -> Result<Option<Figure>> {
        if let Some(ir) = self.article_irs.get(article_id) {
            for figure in &ir.figures {
                if figure.id == figure_id {
                    return Ok(Some(figure.clone()));
                }
            }
        }
        Ok(None)
    }
}
