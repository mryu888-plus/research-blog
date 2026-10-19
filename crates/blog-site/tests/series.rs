//! Ported from tests/test_series.py: series reading order and draft exclusion
//! through the pinned Zola renderer, using the site's own templates.

mod common;

use common::{read, repo_root, write, zola_build, TempDir};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

struct Site {
    dir: TempDir,
    catalog: Value,
}

impl Site {
    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write_catalog(&self) {
        write(
            self.path(),
            "data/collections.json",
            &self.catalog.to_string(),
        );
    }

    fn build(&self, drafts: bool) -> PathBuf {
        zola_build(self.path(), drafts)
    }
}

fn site() -> Site {
    let dir = TempDir::new("series");
    let root = dir.path();
    let repo = repo_root();
    write(
        root,
        "config.toml",
        "base_url = \"https://example.test/blog/\"\ndefault_language = \"zh\"\n[extra]\n",
    );
    write(root, "content/_index.md", "+++\ntitle = \"首页\"\n+++\n");
    write(
        root,
        "content/posts/_index.md",
        "+++\ntitle = \"文章\"\nsort_by = \"date\"\n+++\n",
    );
    write(
        root,
        "templates/base.html",
        "<!doctype html><html><head><title>{% block title %}{% endblock title %}</title></head><body><main>{% block content %}{% endblock content %}</main></body></html>",
    );
    write(root, "templates/index.html", "首页");
    write(root, "templates/section.html", "文章");
    write(
        root,
        "templates/page.html",
        "<h1>{{ page.title }}</h1>{{ page.content | safe }}{% include \"article-series.html\" %}",
    );
    for template in ["series.html", "article-series.html"] {
        write(
            root,
            &format!("templates/{template}"),
            &read(&repo.join("site/templates").join(template)),
        );
    }
    write(
        root,
        "content/series.md",
        &read(&repo.join("site/content/series.md")),
    );
    for (slug, title, draft) in [
        ("a", "第一篇", false),
        ("b", "第二篇", false),
        ("draft", "不可泄露的草稿标题", true),
        ("c", "另一组文章", false),
    ] {
        write(
            root,
            &format!("content/posts/{slug}/index.md"),
            &format!("+++\ntitle = \"{title}\"\ndate = 2026-09-28\ndescription = \"摘要\"\ndraft = {draft}\n+++\n正文\n"),
        );
    }
    let catalog = json!({"version": 1, "series": [{"id": "s-test", "title": "研究 <script>系列", "description": "按组阅读", "groups": [
        {"id": "g-first", "title": "基础", "description": "", "articles": ["posts/a/index.md", "posts/draft/index.md", "posts/missing/index.md", "posts/b/index.md"]},
        {"id": "g-second", "title": "拓展", "description": "", "articles": ["posts/c/index.md"]},
        {"id": "g-empty", "title": "尚未开始", "description": "", "articles": []},
    ]}]});
    let site = Site { dir, catalog };
    site.write_catalog();
    site
}

fn position(text: &str, needle: &str) -> usize {
    text.find(needle)
        .unwrap_or_else(|| panic!("missing {needle}"))
}

#[test]
fn public_series_order_counts_escaping_and_previous_next_skip_drafts() {
    let s = site();
    let output = s.build(false);
    let index = read(&output.join("series/index.html"));
    assert!(position(&index, "第一篇") < position(&index, "第二篇"));
    assert!(!index.contains("不可泄露"));
    assert!(!index.contains("/draft/"));
    assert!(!index.contains("/missing/"));
    for count in ["2 篇", "1 篇", "0 篇"] {
        assert!(index.contains(count), "{count}");
    }
    assert!(!index.contains("<script>"));
    assert!(index.contains("&lt;script&gt;"));
    let first = read(&output.join("posts/a/index.html"));
    let second = read(&output.join("posts/b/index.html"));
    assert!(first.contains("rel=\"next\" href=\"https://example.test/blog/posts/b/\""));
    assert!(second.contains("rel=\"prev\" href=\"https://example.test/blog/posts/a/\""));
    assert!(!first.contains("另一组文章"));
    assert!(first.contains("aria-current=\"page\">第一篇"));
}

#[test]
fn draft_preview_keeps_draft_in_reading_order() {
    let s = site();
    let output = s.build(true);
    let index = read(&output.join("series/index.html"));
    assert!(index.contains("不可泄露的草稿标题"));
    assert!(index.contains("series-draft"));
    assert!(index.contains("3 篇"));
    let first = read(&output.join("posts/a/index.html"));
    assert!(first.contains("rel=\"next\" href=\"https://example.test/blog/posts/draft/\""));
}

#[test]
fn empty_or_absent_catalog_has_a_readable_fallback() {
    let s = site();
    write(
        s.path(),
        "data/collections.json",
        "{\"version\":1,\"series\":[]}",
    );
    let output = s.build(false);
    assert!(read(&output.join("series/index.html")).contains("系列正在整理中"));
    std::fs::remove_file(s.path().join("data/collections.json")).unwrap();
    s.build(false);
}

#[test]
fn direct_series_articles_render_without_group_and_skip_drafts_in_navigation() {
    let mut s = site();
    {
        let series = &mut s.catalog["series"][0];
        series["articles"] = series["groups"][0]["articles"].clone();
        series["groups"] = json!([series["groups"][1].clone()]);
    }
    s.write_catalog();
    let output = s.build(false);
    let index = read(&output.join("series/index.html"));
    assert!(position(&index, "第一篇") < position(&index, "第二篇"));
    assert!(!index.contains("不可泄露"));
    let first = read(&output.join("posts/a/index.html"));
    assert!(first.contains("本系列目录 · 2 篇"));
    assert!(!first.contains("本组目录"));
    assert!(first.contains("rel=\"next\" href=\"https://example.test/blog/posts/b/\""));
    assert!(!first.contains("另一组文章"));
    s.catalog["series"][0]["groups"] = json!([]);
    s.write_catalog();
    let output = s.build(true);
    let first = read(&output.join("posts/a/index.html"));
    assert!(first.contains("本系列目录 · 3 篇"));
    assert!(first.contains("rel=\"next\" href=\"https://example.test/blog/posts/draft/\""));
}
