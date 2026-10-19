//! New article drafts.

use anyhow::{bail, Result};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

static UNSAFE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"[<>:"/\\|?*\x00-\x1f]"#).unwrap());
static RESERVED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\A(con|prn|aux|nul|com[1-9¹²³]|lpt[1-9¹²³])\z").unwrap());

/// The folder name for a title: readable, replacing only characters unsafe in Windows paths.
pub fn folder_name(title: &str) -> Result<String> {
    let replaced = UNSAFE.replace_all(title, "-");
    let mut slug: String = replaced
        .trim_matches([' ', '.', '-'])
        .chars()
        .take(80)
        .collect();
    slug = slug.trim_end_matches([' ', '.']).to_string();
    if slug.is_empty() {
        bail!("标题需要包含可用于目录名的文字或数字。");
    }
    if RESERVED.is_match(slug.split('.').next().unwrap_or("")) {
        slug = format!("文章-{slug}");
    }
    Ok(slug)
}

pub fn create_article(title: &str, posts: &Path, today: &str) -> Result<PathBuf> {
    let title = title.trim();
    if title.is_empty() {
        bail!("请输入文章标题，支持中文。");
    }
    let folder = posts.join(folder_name(title)?);
    fs::create_dir_all(posts)?;
    if let Err(error) = fs::create_dir(&folder) {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            bail!(
                "这篇文章已经存在，请直接打开：{}",
                folder.join("index.md").display()
            );
        }
        return Err(error.into());
    }
    // JSON string escaping is valid for TOML basic strings. Escape '+' so a
    // title containing '+++' cannot terminate the existing frontmatter parser.
    let quoted = serde_json::to_string(title)?.replace('+', "\\u002b");
    let text = format!(
        "+++\ntitle = {quoted}\ndate = {today}\ndescription = \"\"\ndraft = true\n\n[taxonomies]\ntags = []\n+++\n\n## 问题与背景\n\n在这里开始写作。\n\n## 我的思考\n\n\n## 总结\n\n"
    );
    fs::create_dir(folder.join("figures"))?;
    let article = folder.join("index.md");
    fs::write(&article, text)?;
    Ok(article)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_names_keep_readable_titles() {
        assert_eq!(folder_name("从文字到向量").unwrap(), "从文字到向量");
        assert_eq!(folder_name(" a/b:c? ").unwrap(), "a-b-c");
        assert_eq!(folder_name("con").unwrap(), "文章-con");
        assert_eq!(folder_name("CON.txt").unwrap(), "文章-CON.txt");
        assert!(folder_name("...").is_err());
    }

    #[test]
    fn creates_draft_with_escaped_title() {
        let dir = std::env::temp_dir().join(format!("blog-article-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let article = create_article("a+++\"b", &dir, "2026-10-20").unwrap();
        let text = fs::read_to_string(&article).unwrap();
        assert!(
            text.starts_with("+++\ntitle = \"a\\u002b\\u002b\\u002b\\\"b\"\ndate = 2026-10-20\n")
        );
        assert!(article.parent().unwrap().join("figures").is_dir());
        assert!(create_article("a+++\"b", &dir, "2026-10-20").is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}
