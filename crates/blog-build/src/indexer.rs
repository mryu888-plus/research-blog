use anyhow::Result;
use blog_ir::ArticleIR;
use std::path::Path;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexWriter};
use tantivy_jieba::JiebaTokenizer;

pub struct Indexer {
    index: Index,
    writer: IndexWriter,
    schema: Schema,
}

impl Indexer {
    pub fn new(index_dir: &Path) -> Result<Self> {
        let mut schema_builder = Schema::builder();

        let text_options = TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("jieba")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        );

        schema_builder.add_text_field("article_id", TEXT | STORED);
        schema_builder.add_text_field("title", text_options.clone() | STORED);
        schema_builder.add_text_field("slug", STRING | STORED);
        schema_builder.add_text_field("content", text_options.clone());
        schema_builder.add_text_field("section_id", STRING | STORED);
        schema_builder.add_text_field("section_title", text_options.clone() | STORED);
        schema_builder.add_text_field("section_anchor", STRING | STORED);
        schema_builder.add_text_field("tags", TEXT | STORED);

        let schema = schema_builder.build();

        std::fs::create_dir_all(index_dir)?;
        let index = Index::open_or_create(
            tantivy::directory::MmapDirectory::open(index_dir)?,
            schema.clone(),
        )?;
        index.tokenizers().register("jieba", JiebaTokenizer {});
        let writer = index.writer(50_000_000)?;

        Ok(Self {
            index,
            writer,
            schema,
        })
    }

    pub fn open(index_dir: &Path) -> Result<Self> {
        let index = Index::open_in_dir(index_dir)?;
        index.tokenizers().register("jieba", JiebaTokenizer {});
        let schema = index.schema();
        let writer = index.writer(50_000_000)?;

        Ok(Self {
            index,
            writer,
            schema,
        })
    }

    pub fn index_article(&mut self, article: &ArticleIR) -> Result<()> {
        let article_id = self.schema.get_field("article_id").unwrap();
        let title = self.schema.get_field("title").unwrap();
        let slug = self.schema.get_field("slug").unwrap();
        let content = self.schema.get_field("content").unwrap();
        let section_id = self.schema.get_field("section_id").unwrap();
        let section_title = self.schema.get_field("section_title").unwrap();
        let section_anchor = self.schema.get_field("section_anchor").unwrap();
        let tags = self.schema.get_field("tags").unwrap();

        self.writer
            .delete_term(tantivy::Term::from_field_text(slug, &article.metadata.slug));

        for section in &article.sections {
            let mut doc = doc!(
                article_id => article.metadata.slug.clone(),
                title => article.metadata.title.clone(),
                slug => article.metadata.slug.clone(),
                content => section.content.clone(),
                section_id => section.id.clone(),
                section_title => section.title.clone(),
                section_anchor => section.anchor.clone(),
            );

            for tag in &article.metadata.tags {
                doc.add_text(tags, tag);
            }

            self.writer.add_document(doc)?;
        }

        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        self.writer.delete_all_documents()?;
        Ok(())
    }

    pub fn commit(&mut self) -> Result<()> {
        self.writer.commit()?;
        Ok(())
    }

    pub fn get_index(&self) -> &Index {
        &self.index
    }
}
