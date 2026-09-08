use crate::{ArticleIR, Citation, CodeBlock, Equation, Figure, Metadata, ParseError, Section};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::path::Path;

/// 从 Markdown 文件解析为 ArticleIR
pub fn parse_article(path: &Path, content: &str) -> Result<ArticleIR, ParseError> {
    let (frontmatter, body, fm_end_line) = extract_frontmatter(content, path)?;
    let metadata = parse_metadata(frontmatter, path)?;

    let mut sections = Vec::new();
    let mut equations = Vec::new();
    let mut figures = Vec::new();
    let mut code_blocks = Vec::new();
    let mut citations = Vec::new();
    let mut plain_text = String::new();

    let mut current_section = Section {
        id: "intro".to_string(),
        anchor: "intro".to_string(),
        level: 0,
        title: "Introduction".to_string(),
        content: String::new(),
        line_start: fm_end_line,
        line_end: fm_end_line,
    };

    let mut line_num = fm_end_line;
    let mut in_code_block = false;
    let mut code_lang = None;
    let mut code_content = String::new();
    let mut code_start_line = 0;

    let options = Options::all();
    let parser = Parser::new_ext(body, options);

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                if !current_section.content.is_empty() {
                    sections.push(current_section.clone());
                }
                current_section = Section {
                    id: String::new(),
                    anchor: String::new(),
                    level: level as u8,
                    title: String::new(),
                    content: String::new(),
                    line_start: line_num,
                    line_end: line_num,
                };
            }
            Event::End(TagEnd::Heading(_)) => {
                let anchor = slugify(&current_section.title);
                current_section.id = format!("{}_{}", anchor, sections.len());
                current_section.anchor = anchor;
            }
            Event::Text(text) => {
                let text_str = text.to_string();

                if text_str.contains('$') {
                    extract_inline_equations(
                        &text_str,
                        &current_section.id,
                        line_num,
                        &mut equations,
                    );
                }

                extract_citations(&text_str, &current_section.id, line_num, &mut citations);

                if in_code_block {
                    code_content.push_str(&text_str);
                } else {
                    current_section.content.push_str(&text_str);
                    plain_text.push_str(&text_str);
                    plain_text.push(' ');
                }

                if current_section.title.is_empty() && current_section.level > 0 {
                    current_section.title.push_str(&text_str);
                }

                line_num += text_str.lines().count().saturating_sub(1);
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                code_lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => Some(lang.to_string()),
                    _ => None,
                };
                code_content.clear();
                code_start_line = line_num;
            }
            Event::End(TagEnd::CodeBlock) => {
                if in_code_block {
                    let id = format!("code_{}", code_blocks.len());
                    code_blocks.push(CodeBlock {
                        id,
                        language: code_lang.clone(),
                        source: code_content.clone(),
                        section_id: current_section.id.clone(),
                        line: code_start_line,
                    });
                    in_code_block = false;
                    line_num += code_content.lines().count();
                }
            }
            Event::InlineMath(src) => {
                let id = format!("eq_inline_{}_{}", current_section.id, equations.len());
                equations.push(Equation {
                    id,
                    source: src.to_string(),
                    display: false,
                    section_id: current_section.id.clone(),
                    line: line_num,
                });
                current_section.content.push('$');
                current_section.content.push_str(&src);
                current_section.content.push('$');
                plain_text.push_str(&src);
                plain_text.push(' ');
            }
            Event::DisplayMath(src) => {
                let id = format!("eq_display_{}_{}", current_section.id, equations.len());
                equations.push(Equation {
                    id,
                    source: src.to_string(),
                    display: true,
                    section_id: current_section.id.clone(),
                    line: line_num,
                });
                current_section.content.push_str("$$");
                current_section.content.push_str(&src);
                current_section.content.push_str("$$");
                plain_text.push_str(&src);
                plain_text.push(' ');
                line_num += src.lines().count();
            }
            Event::Code(code) => {
                current_section.content.push('`');
                current_section.content.push_str(&code);
                current_section.content.push('`');
            }
            Event::SoftBreak | Event::HardBreak => {
                current_section.content.push('\n');
                line_num += 1;
            }
            _ => {}
        }
    }

    if !current_section.content.is_empty() || !current_section.title.is_empty() {
        current_section.line_end = line_num;
        sections.push(current_section);
    }

    // 扫描原始正文提取图引用（必须在 sections 构建完成后，因为需要定位 section_id）
    extract_figures(body, &sections, &mut figures);

    Ok(ArticleIR {
        metadata,
        sections,
        equations,
        figures,
        code_blocks,
        citations,
        plain_text,
    })
}

fn extract_frontmatter<'a>(
    content: &'a str,
    path: &Path,
) -> Result<(&'a str, &'a str, usize), ParseError> {
    let lines: Vec<&str> = content.lines().collect();

    if lines.first() != Some(&"+++") && lines.first() != Some(&"---") {
        return Err(ParseError::FrontmatterError {
            path: path.display().to_string(),
            message: "Frontmatter must start with +++ or ---".to_string(),
        });
    }

    let delimiter = lines[0];
    if let Some(end_idx) = lines.iter().skip(1).position(|&line| line == delimiter) {
        let body_start = end_idx + 2;

        let mut parts = content.splitn(3, delimiter);
        parts.next();
        let fm = parts.next().unwrap_or("");
        let body = parts.next().unwrap_or("");

        Ok((fm.trim(), body.trim(), body_start))
    } else {
        Err(ParseError::FrontmatterError {
            path: path.display().to_string(),
            message: format!("No closing {} found", delimiter),
        })
    }
}

fn parse_metadata(frontmatter: &str, path: &Path) -> Result<Metadata, ParseError> {
    let map: toml::Table =
        frontmatter
            .parse::<toml::Table>()
            .map_err(|error| ParseError::FrontmatterError {
                path: path.display().to_string(),
                message: error.to_string(),
            })?;
    let string = |key: &str| {
        map.get(key)
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
    };
    let missing = |field: &str| ParseError::MissingMetadata {
        path: path.display().to_string(),
        field: field.to_string(),
    };
    let title = string("title").ok_or_else(|| missing("title"))?;
    #[derive(serde::Deserialize)]
    struct DateField {
        date: toml::value::Datetime,
    }
    let date = if let Some(date) = string("date") {
        date
    } else {
        toml::from_str::<DateField>(frontmatter)
            .map(|field| field.date.to_string())
            .map_err(|_| missing("date"))?
    };
    let slug = string("slug").unwrap_or_else(|| {
        path.parent()
            .filter(|_| path.file_name().is_some_and(|n| n == "index.md"))
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| slugify(&title))
    });
    let tags = map
        .get("taxonomies")
        .and_then(|v| v.get("tags"))
        .or_else(|| map.get("tags"))
        .and_then(toml::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let author = map
        .get("extra")
        .and_then(|v| v.get("author"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .or_else(|| string("author"));
    Ok(Metadata {
        title,
        slug,
        date,
        author,
        summary: string("description").or_else(|| string("summary")),
        language: string("language").unwrap_or_else(|| "zh".to_string()),
        tags,
        source_path: path.display().to_string(),
    })
}

pub fn article_is_draft(path: &Path, content: &str) -> Result<bool, ParseError> {
    let (frontmatter, _, _) = extract_frontmatter(content, path)?;
    let map: toml::Table =
        frontmatter
            .parse::<toml::Table>()
            .map_err(|error| ParseError::FrontmatterError {
                path: path.display().to_string(),
                message: error.to_string(),
            })?;
    Ok(map
        .get("draft")
        .and_then(toml::Value::as_bool)
        .unwrap_or(false))
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn extract_inline_equations(
    text: &str,
    section_id: &str,
    line: usize,
    equations: &mut Vec<Equation>,
) {
    let chars = text.chars();
    let mut in_eq = false;
    let mut eq_buf = String::new();

    for ch in chars {
        if ch == '$' {
            if in_eq {
                if !eq_buf.is_empty() {
                    let id = format!("eq_inline_{}_{}", section_id, equations.len());
                    equations.push(Equation {
                        id,
                        source: eq_buf.clone(),
                        display: false,
                        section_id: section_id.to_string(),
                        line,
                    });
                }
                eq_buf.clear();
                in_eq = false;
            } else {
                in_eq = true;
            }
        } else if in_eq {
            eq_buf.push(ch);
        }
    }
}

fn extract_citations(text: &str, section_id: &str, line: usize, citations: &mut Vec<Citation>) {
    let mut rest = text;
    while let Some(start) = rest.find("[@") {
        rest = &rest[start + 2..];
        if let Some(end) = rest.find(']') {
            let key = rest[..end].trim().to_string();
            if !key.is_empty() && !key.contains(' ') {
                citations.push(Citation {
                    key,
                    section_id: section_id.to_string(),
                    line,
                });
            }
            rest = &rest[end + 1..];
        } else {
            break;
        }
    }
}

/// 解析 Tera 组件形式的图引用（Zola 0.23+）：
/// `{{ <fig id="x" caption="..." alt="..." /> }}`
///
/// 直接扫描原始 Markdown 正文，因为 pulldown-cmark 会把组件调用
/// 拆散成多个事件，无法在 Event::Text 里可靠还原。
fn extract_figures(body: &str, sections: &[Section], figures: &mut Vec<Figure>) {
    for (idx, raw_line) in body.lines().enumerate() {
        let line_no = idx + 1;
        let mut rest = raw_line;

        while let Some(start) = rest.find("<fig") {
            rest = &rest[start + 4..];
            // 确保是 `<fig ` 或 `<fig/` 而不是 `<figure` 之类
            if !rest.starts_with(|c: char| c.is_whitespace() || c == '/') {
                continue;
            }
            let Some(end) = rest.find("/>") else { break };
            let args = &rest[..end];
            rest = &rest[end + 2..];

            let id = shortcode_arg(args, "id").unwrap_or_default();
            if id.is_empty() {
                continue;
            }
            let caption = shortcode_arg(args, "caption").unwrap_or_default();
            let alt_text = shortcode_arg(args, "alt").unwrap_or_else(|| caption.clone());

            let section_id = sections
                .iter()
                .rfind(|s| s.line_start <= line_no)
                .map(|s| s.id.clone())
                .unwrap_or_else(|| "intro".to_string());

            figures.push(Figure {
                id: id.clone(),
                typst_source: format!("figures/{}.typ", id),
                caption,
                alt_text,
                section_id,
                line: line_no,
            });
        }
    }
}

/// 从组件参数串里取出 `key="value"`。
fn shortcode_arg(args: &str, key: &str) -> Option<String> {
    let mut search = args;
    while let Some(pos) = search.find(key) {
        let after = &search[pos + key.len()..];
        let before_ok = pos == 0
            || !search[..pos]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
        let trimmed = after.trim_start();
        if before_ok {
            if let Some(v) = trimmed.strip_prefix('=') {
                let v = v.trim_start();
                if let Some(v) = v.strip_prefix('"') {
                    if let Some(close) = v.find('"') {
                        return Some(v[..close].to_string());
                    }
                }
            }
        }
        search = after;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_parse_basic_article() {
        let content = r#"+++
title = "Test Article"
date = "2024-01-01"
author = "Test Author"
+++

# Introduction

This is a test.

## Methods

Some methods here.
"#;
        let ir = parse_article(Path::new("test.md"), content).unwrap();
        assert_eq!(ir.metadata.title, "Test Article");
        assert!(!ir.sections.is_empty());
    }

    #[test]
    fn test_inline_equation_extraction() {
        let mut equations = Vec::new();
        extract_inline_equations(
            "The formula $E = mc^2$ is famous.",
            "sec_0",
            1,
            &mut equations,
        );
        assert_eq!(equations.len(), 1);
        assert_eq!(equations[0].source, "E = mc^2");
        assert!(!equations[0].display);
    }

    #[test]
    fn test_citation_extraction() {
        let mut citations = Vec::new();
        extract_citations(
            "See [@smith2023] and [@jones2024].",
            "sec_0",
            1,
            &mut citations,
        );
        assert_eq!(citations.len(), 2);
        assert_eq!(citations[0].key, "smith2023");
        assert_eq!(citations[1].key, "jones2024");
    }

    #[test]
    fn test_figure_extraction() {
        let sections = vec![Section {
            id: "sec_0".to_string(),
            anchor: "sec".to_string(),
            level: 2,
            title: "Sec".to_string(),
            content: String::new(),
            line_start: 1,
            line_end: 3,
        }];
        let mut figures = Vec::new();
        extract_figures(
            "intro text\n{{ <fig id=\"my-plot\" caption=\"A nice plot\" alt=\"Plot showing results\" /> }}\n",
            &sections,
            &mut figures,
        );
        assert_eq!(figures.len(), 1);
        assert_eq!(figures[0].id, "my-plot");
        assert_eq!(figures[0].caption, "A nice plot");
        assert_eq!(figures[0].alt_text, "Plot showing results");
        assert_eq!(figures[0].typst_source, "figures/my-plot.typ");
        assert_eq!(figures[0].section_id, "sec_0");
    }

    #[test]
    fn test_figure_extraction_ignores_html_figure_tag() {
        let mut figures = Vec::new();
        extract_figures(
            "<figure class=\"x\">not a component</figure>",
            &[],
            &mut figures,
        );
        assert!(figures.is_empty());
    }

    #[test]
    fn test_figure_alt_falls_back_to_caption() {
        let mut figures = Vec::new();
        extract_figures("{{ <fig id=\"p\" caption=\"Cap\" /> }}", &[], &mut figures);
        assert_eq!(figures.len(), 1);
        assert_eq!(figures[0].alt_text, "Cap");
    }

    #[test]
    fn test_math_events_populate_ir() {
        let md = "+++\ntitle = \"t\"\ndate = 2024-01-01\n+++\n## Sec\n\nInline $a = b$ here.\n\n$$\nE = m c^2\n$$\n";
        let ir = parse_article(Path::new("t.md"), md).expect("parse");
        let inline: Vec<_> = ir.equations.iter().filter(|e| !e.display).collect();
        let display: Vec<_> = ir.equations.iter().filter(|e| e.display).collect();
        assert_eq!(inline.len(), 1, "expected one inline equation");
        assert_eq!(inline[0].source.trim(), "a = b");
        assert_eq!(display.len(), 1, "expected one display equation");
        assert!(display[0].source.contains("m c^2"));
    }
}

#[cfg(test)]
mod metadata_regressions {
    use super::*;
    #[test]
    fn zola_metadata_and_bundle_slug_match_frontend() {
        let source = "+++\ntitle = \"Chinese title\"\ndate = 2026-09-08\ndescription = \"summary\"\n[taxonomies]\ntags = [\"agent\", \"skill\"]\n[extra]\nauthor = \"Writer\"\n+++\n\n# Title\nBody";
        let ir = parse_article(Path::new("site/content/posts/my-note/index.md"), source).unwrap();
        assert_eq!(ir.metadata.slug, "my-note");
        assert_eq!(ir.metadata.tags, vec!["agent", "skill"]);
        assert_eq!(ir.metadata.author.as_deref(), Some("Writer"));
        assert_eq!(ir.metadata.summary.as_deref(), Some("summary"));
        assert_eq!(ir.metadata.date, "2026-09-08");
        assert!(!article_is_draft(Path::new("a.md"), source).unwrap());
        let draft = source.replace("description =", "draft = true\ndescription =");
        assert!(article_is_draft(Path::new("a.md"), &draft).unwrap());
    }
}
