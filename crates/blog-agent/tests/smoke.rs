//! 端到端 smoke test：从 Markdown 源构建索引，再经检索层查回来。
//!
//! 这条链路覆盖 blog-ir 解析 → blog-build 索引 → blog-agent 检索，
//! 不依赖 LLM、Zola 或 typst CLI，因此可以在 CI 中无凭据运行。

use blog_agent::retrieval::Retriever;
use blog_build::indexer::Indexer;
use blog_ir::parser::parse_article;
use std::fs;
use std::path::Path;

const ARTICLE: &str = r#"+++
title = "商空间与技能抽象"
date = 2024-05-01
[taxonomies]
tags = ["skill"]
+++

## 行为等价

给定 MDP，我们定义两个子轨迹行为等价，当且仅当 $Q^*(s, \tau_1) = Q^*(s, \tau_2)$。

## 商空间构造

$$
\mathcal{T} / \sim
$$

商空间即为技能空间。
"#;

fn build_index(tmp: &Path) -> Retriever {
    let article_path = tmp.join("index.md");
    fs::write(&article_path, ARTICLE).expect("write article");

    let ir = parse_article(&article_path, ARTICLE).expect("parse article");
    assert!(!ir.sections.is_empty(), "sections should be extracted");
    assert!(
        ir.equations.len() >= 2,
        "expected inline + display equations, got {}",
        ir.equations.len()
    );

    let index_dir = tmp.join("index");
    let mut indexer = Indexer::new(&index_dir).expect("create indexer");
    indexer.index_article(&ir).expect("index article");
    indexer.commit().expect("commit index");

    // manifest 是 Agent 侧还原 ArticleIR 的载体
    let manifests = index_dir.join("manifests");
    fs::create_dir_all(&manifests).expect("manifest dir");
    fs::write(
        manifests.join(format!("{}.json", ir.metadata.slug)),
        serde_json::to_string_pretty(&ir).expect("serialize ir"),
    )
    .expect("write manifest");

    Retriever::open(index_dir).expect("open retriever")
}

#[test]
fn end_to_end_index_and_retrieve() {
    let tmp = std::env::temp_dir().join(format!("blog-smoke-{}", std::process::id()));
    fs::create_dir_all(&tmp).expect("tmp dir");

    let retriever = build_index(&tmp);

    let hits = retriever.search("商空间", 5).expect("search");
    assert!(!hits.is_empty(), "expected hits for 商空间");
    assert!(
        hits.iter().any(|h| h.article_title.contains("商空间")),
        "retrieved article title should match, got: {:?}",
        hits.iter().map(|h| &h.article_title).collect::<Vec<_>>()
    );

    let miss = retriever
        .search("完全不相关的查询内容xyzzy", 5)
        .expect("search miss");
    assert!(miss.is_empty(), "unrelated query should not match");

    fs::remove_dir_all(&tmp).ok();
}

/// 回归测试：tantivy 默认分词器不切分中文，会把整句当成单个 token，
/// 导致“技能”这类词级查询完全搜不到。索引与检索两端必须同时注册 jieba。
#[test]
fn end_to_end_chinese_word_level_retrieval() {
    let tmp = std::env::temp_dir().join(format!("blog-smoke-zh-{}", std::process::id()));
    fs::create_dir_all(&tmp).expect("tmp dir");

    let retriever = build_index(&tmp);

    for query in ["技能", "等价", "轨迹"] {
        let hits = retriever.search(query, 5).expect("search");
        assert!(
            !hits.is_empty(),
            "word-level Chinese query {:?} returned no hits (tokenizer regression)",
            query
        );
    }

    fs::remove_dir_all(&tmp).ok();
}

#[test]
fn reindex_replaces_existing_sections() {
    let tmp = std::env::temp_dir().join(format!("blog-reindex-{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    {
        let ir = parse_article(
            Path::new("site/content/posts/stable-slug/index.md"),
            ARTICLE,
        )
        .unwrap();
        let mut indexer = Indexer::new(&tmp).unwrap();
        indexer.index_article(&ir).unwrap();
        indexer.commit().unwrap();
        indexer.index_article(&ir).unwrap();
        indexer.commit().unwrap();
        let reader = indexer.get_index().reader().unwrap();
        assert_eq!(reader.searcher().num_docs(), ir.sections.len() as u64);
        indexer.clear().unwrap();
        indexer.commit().unwrap();
        reader.reload().unwrap();
        assert_eq!(reader.searcher().num_docs(), 0);
    }
    fs::remove_dir_all(tmp).unwrap();
}
