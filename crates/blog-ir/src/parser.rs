use crate::{ArticleIR, Citation, CodeBlock, Equation, Figure, Metadata, ParseError, Section};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;
use std::path::Path;

/// 从 Markdown 文件解析为 ArticleIR
pub fn parse_article(path: &Path, content: &str) -> Result<ArticleIR, ParseError> {
    let (frontmatter, body, fm_end_line) = extract_frontmatter(content, path)?;
    let metadata = parse_metadata(frontmatter, path)?;

    let mut sections = Vec::new();
    let mut equations = Vec::new();
    let mut figures = Vec::new();
    let mut code_blocks = Vec::new();
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
    let mut code_start_offset = 0;
    let mut in_heading = false;
    let mut citation_exclusions = Vec::new();
    let line_starts: Vec<_> = std::iter::once(0)
        .chain(
            body.bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        )
        .collect();

    let options = Options::all();
    let parser = Parser::new_ext(body, options);

    for (event, range) in parser.into_offset_iter() {
        line_num = source_line(range.start, &line_starts, fm_end_line);
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading = true;
                if !current_section.content.is_empty() {
                    current_section.line_end = line_num.saturating_sub(1);
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
                in_heading = false;
                let anchor = slugify(&current_section.title);
                current_section.id = format!("{}_{}", anchor, sections.len());
                current_section.anchor = anchor;
            }
            Event::Text(text) => {
                let text_str = text.to_string();

                if !in_code_block && text_str.contains('$') {
                    extract_inline_equations(
                        &text_str,
                        &current_section.id,
                        line_num,
                        &mut equations,
                    );
                }

                if in_code_block {
                    code_content.push_str(&text_str);
                } else {
                    current_section.content.push_str(&text_str);
                    plain_text.push_str(&text_str);
                    plain_text.push(' ');
                }

                if in_heading {
                    current_section.title.push_str(&text_str);
                }
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                code_lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => Some(lang.to_string()),
                    _ => None,
                };
                code_content.clear();
                code_start_line = line_num;
                code_start_offset = range.start;
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
                    citation_exclusions.push(code_start_offset..range.end);
                    in_code_block = false;
                }
            }
            Event::InlineMath(src) => {
                citation_exclusions.push(range.clone());
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
                citation_exclusions.push(range.clone());
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
            }
            Event::Code(code) => {
                citation_exclusions.push(range.clone());
                current_section.content.push('`');
                current_section.content.push_str(&code);
                current_section.content.push('`');
                if in_heading {
                    current_section.title.push_str(&code);
                }
            }
            Event::Html(_) | Event::InlineHtml(_) => citation_exclusions.push(range.clone()),
            Event::Start(Tag::Link { .. } | Tag::Image { .. }) => {
                citation_exclusions.push(range.clone());
            }
            Event::SoftBreak | Event::HardBreak => {
                current_section.content.push('\n');
            }
            _ => {}
        }
        line_num = source_line(range.end.saturating_sub(1), &line_starts, fm_end_line);
    }

    if !current_section.content.is_empty() || !current_section.title.is_empty() {
        current_section.line_end = line_num;
        sections.push(current_section);
    }

    // 扫描原始正文提取图引用（必须在 sections 构建完成后，因为需要定位 section_id）
    extract_figures(body, &sections, fm_end_line, &mut figures);
    let citations = extract_article_citations(
        body,
        &sections,
        &line_starts,
        fm_end_line,
        citation_exclusions,
    );

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
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut lines = content.split_inclusive('\n');
    let first = lines.next().unwrap_or("");
    let delimiter = first.trim_end();
    if delimiter != "+++" && delimiter != "---" {
        return Err(ParseError::FrontmatterError {
            path: path.display().to_string(),
            message: "Frontmatter must start with +++ or ---".to_string(),
        });
    }

    let mut offset = first.len();
    for (index, line) in lines.enumerate() {
        if line.trim_end() == delimiter {
            let body_offset = offset + line.len();
            return Ok((
                content[first.len()..offset].trim(),
                &content[body_offset..],
                index + 3,
            ));
        }
        offset += line.len();
    }
    Err(ParseError::FrontmatterError {
        path: path.display().to_string(),
        message: format!("No closing {} found", delimiter),
    })
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

fn source_line(offset: usize, starts: &[usize], first_line: usize) -> usize {
    first_line
        + starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1)
}

fn escaped_at(source: &str, offset: usize) -> bool {
    source.as_bytes()[..offset]
        .iter()
        .rev()
        .take_while(|byte| **byte == b'\\')
        .count()
        % 2
        == 1
}

fn citation_keys(group: &str) -> Option<Vec<&str>> {
    group
        .split(';')
        .map(|part| {
            let reference = part.trim().strip_prefix('@')?;
            let (key, _) = reference.split_once(',').unwrap_or((reference, ""));
            let key = key.trim();
            (!key.is_empty()
                && !key.chars().any(|character| {
                    character.is_whitespace() || "@;,[]{}<>\"\\".contains(character)
                }))
            .then_some(key)
        })
        .collect()
}

fn bracket_math_end(body: &str, cursor: usize) -> Option<usize> {
    let opening = body.get(cursor..cursor + 2)?;
    let closing = match opening {
        "\\(" => "\\)",
        "\\[" => "\\]",
        _ => return None,
    };
    let begin = cursor + 2;
    if opening == "\\(" && body[begin..].chars().next().is_none_or(char::is_whitespace) {
        return None;
    }
    let mut candidate = begin;
    while let Some(relative) = body[candidate..].find(closing) {
        let end = candidate + relative;
        if opening == "\\(" && body[begin..end].contains(['\n', '\r']) {
            return None;
        }
        if !escaped_at(body, end) {
            return Some(end + 2);
        }
        candidate = end + 2;
    }
    None
}

fn verbatim_html_end(body: &str, cursor: usize) -> Option<usize> {
    let rest = &body[cursor..];
    let name_end = rest[1..].find(|character: char| {
        !character.is_ascii_alphanumeric() && character != '-' && character != ':'
    })? + 1;
    let name = rest[1..name_end].to_ascii_lowercase();
    if !["pre", "code", "script", "style", "textarea", "math", "svg"].contains(&name.as_str()) {
        return None;
    }
    let mut quote = None;
    let mut tag_end = None;
    for (index, character) in rest[name_end..].char_indices() {
        if let Some(delimiter) = quote {
            if character == delimiter {
                quote = None;
            }
        } else if character == '\'' || character == '"' {
            quote = Some(character);
        } else if character == '>' {
            tag_end = Some(cursor + name_end + index + 1);
            break;
        }
    }
    let after = tag_end?;
    for (relative, _) in body[after..].match_indices('<') {
        let start = after + relative;
        let closing = &body[start..];
        let prefix = format!("</{}", name);
        if closing
            .get(..prefix.len())
            .is_some_and(|text| text.eq_ignore_ascii_case(&prefix))
        {
            let remainder = &closing[prefix.len()..];
            let trimmed = remainder.trim_start();
            if trimmed.starts_with('>') {
                return Some(start + prefix.len() + remainder.len() - trimmed.len() + 1);
            }
        }
    }
    Some(body.len())
}

/// Scan the source once: offset spans preserve CommonMark exclusions, while the
/// original bracket group survives pulldown-cmark's split Text events. Byte
/// offsets always advance on UTF-8 character boundaries. Section/line metadata
/// is assigned only after the complete section list exists.
fn extract_article_citations(
    body: &str,
    sections: &[Section],
    line_starts: &[usize],
    first_line: usize,
    mut excluded: Vec<Range<usize>>,
) -> Vec<Citation> {
    excluded.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in excluded {
        if let Some(previous) = merged
            .last_mut()
            .filter(|previous| previous.end >= range.start)
        {
            previous.end = previous.end.max(range.end);
        } else {
            merged.push(range);
        }
    }
    let mut citations = Vec::new();
    let mut cursor = 0;
    let mut exclusion = 0;
    while cursor < body.len() {
        while exclusion < merged.len() && merged[exclusion].end <= cursor {
            exclusion += 1;
        }
        // Inline HTML events contain the opening tag only. Protect complete
        // verbatim elements before jumping over that opening event's span.
        if body.as_bytes()[cursor] == b'<' {
            if let Some(end) = verbatim_html_end(body, cursor) {
                cursor = end;
                continue;
            }
        }
        if exclusion < merged.len() && merged[exclusion].start <= cursor {
            cursor = merged[exclusion].end;
            continue;
        }
        if cursor == 0 || body.as_bytes()[cursor - 1] == b'\n' {
            let end = body[cursor..]
                .find('\n')
                .map_or(body.len(), |end| cursor + end + 1);
            let line = &body[cursor..end];
            let logical = line.trim_start_matches(' ');
            if line.len() - logical.len() <= 3 && logical.starts_with('[') && logical.contains("]:")
            {
                cursor = end;
                continue;
            }
        }
        if body[cursor..].starts_with("{{") || body[cursor..].starts_with("{%") {
            let close = if body[cursor..].starts_with("{{") {
                "}}"
            } else {
                "%}"
            };
            if let Some(end) = body[cursor + 2..].find(close) {
                cursor += end + 4;
                continue;
            }
        }
        let starts_bracket_math =
            body[cursor..].starts_with("\\(") || body[cursor..].starts_with("\\[");
        if (starts_bracket_math || body.as_bytes()[cursor] == b'[') && !escaped_at(body, cursor) {
            if let Some(end) = bracket_math_end(body, cursor) {
                cursor = end;
                continue;
            }
            if body.as_bytes()[cursor] == b'['
                && (cursor == 0 || body.as_bytes()[cursor - 1] != b'!')
            {
                if let Some(relative_end) = body[cursor + 1..].find(']') {
                    let end = cursor + 1 + relative_end;
                    let group = &body[cursor + 1..end];
                    let after = body.as_bytes().get(end + 1);
                    if !group.contains(['\n', '\r']) && after != Some(&b'(') && after != Some(&b'[')
                    {
                        if let Some(keys) = citation_keys(group) {
                            let line = source_line(cursor, line_starts, first_line);
                            let section_id = sections
                                .iter()
                                .rfind(|section| section.line_start <= line)
                                .map_or("intro", |section| section.id.as_str());
                            citations.extend(keys.into_iter().map(|key| Citation {
                                key: key.to_string(),
                                section_id: section_id.to_string(),
                                line,
                            }));
                            cursor = end + 1;
                            continue;
                        }
                    }
                }
            }
        }
        cursor += body[cursor..]
            .chars()
            .next()
            .expect("valid source cursor")
            .len_utf8();
    }
    citations
}

/// 解析 Tera 组件形式的图引用（Zola 0.23+）：
/// `{{ <fig id="x" caption="..." alt="..." /> }}`
///
/// 直接扫描原始 Markdown 正文，因为 pulldown-cmark 会把组件调用
/// 拆散成多个事件，无法在 Event::Text 里可靠还原。
fn extract_figures(body: &str, sections: &[Section], first_line: usize, figures: &mut Vec<Figure>) {
    for (idx, raw_line) in body.lines().enumerate() {
        let line_no = idx + first_line;
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
        let citations = extract_article_citations(
            "See [@smith2023] and [@jones2024].",
            &[],
            &[0],
            1,
            Vec::new(),
        );
        assert_eq!(citations.len(), 2);
        assert_eq!(citations[0].key, "smith2023");
        assert_eq!(citations[1].key, "jones2024");
    }

    #[test]
    fn grouped_citations_and_locators_keep_utf8_source_lines_and_sections() {
        let source = "+++\ntitle = \"中文 [@frontmatter]\"\ndate = 2026-10-12\ndescription = \"+++ is a literal string\"\n+++\n\n正文 [@intro]。\n\n## **研究** Methods\n\n中文正文 [@paper.a; @论文二, p. 3]，再引 [@paper.a]。\n";
        let ir = parse_article(Path::new("中文/index.md"), source).unwrap();
        let keys: Vec<_> = ir
            .citations
            .iter()
            .map(|citation| citation.key.as_str())
            .collect();
        assert_eq!(keys, ["intro", "paper.a", "论文二", "paper.a"]);
        assert_eq!(ir.citations[0].line, 7);
        assert_eq!(ir.citations[0].section_id, "intro");
        let methods = ir
            .sections
            .iter()
            .find(|section| section.title == "研究 Methods")
            .unwrap();
        assert_eq!(methods.line_start, 9);
        for citation in &ir.citations[1..] {
            assert_eq!(citation.line, 11);
            assert_eq!(citation.section_id, methods.id);
        }
    }

    #[test]
    fn citations_ignore_code_math_escapes_html_shortcodes_links_and_emails() {
        let body = concat!(
            "`[@inline]`\n\n",
            "```md\n[@fenced] $[@code_math]$\n```\n\n",
            "    [@indented]\n\n",
            "- > ~~~\n  > [@nested]\n  > ~~~\n\n",
            "普通段落\n\n",
            "$x=[@dollar]$\n\n$$\n[@display]\n$$\n\n",
            "\\([@paren]\\) \\[ [@bracket] \\]\n\n",
            "\\[@escaped] ![@image](file.png) [@link](https://example.org/)\n\n",
            "<code>[@html_code]</code> <!-- [@comment] -->\n\n",
            "<script>[@script]</script>\n\n",
            "{{ <fig id=\"[@shortcode]\" /> }}\n\n",
            "<a title=\"[@attribute]\">普通文字</a>\n\n",
            "reader@example.org <reader@example.org> [@reader@example.org]\n\n",
            "[ordinary]: https://example.org/[@destination]\n\n",
            "实际引用 [@visible, pp. 3-5; @second]。\n"
        );
        let source = format!("+++\ntitle = \"Article\"\ndate = 2026-10-12\n+++\n{body}");
        let ir = parse_article(Path::new("a.md"), &source).unwrap();
        let keys: Vec<_> = ir
            .citations
            .iter()
            .map(|citation| citation.key.as_str())
            .collect();
        assert_eq!(keys, ["visible", "second"]);
        assert_eq!(ir.code_blocks.len(), 3);
    }

    #[test]
    fn citations_do_not_use_decoded_or_split_markdown_text() {
        let source = "+++\ntitle = \"Article\"\ndate = 2026-10-12\n+++\n正文 [@a; @b, p. 3]。&#91;@entity&#93; \\[@escape]\n";
        let ir = parse_article(Path::new("a.md"), source).unwrap();
        let keys: Vec<_> = ir
            .citations
            .iter()
            .map(|citation| citation.key.as_str())
            .collect();
        assert_eq!(keys, ["a", "b"]);
        assert!(ir.citations.iter().all(|citation| citation.line == 5));
    }

    #[test]
    fn crlf_bom_and_blank_lines_preserve_real_citation_and_figure_positions() {
        let source = "\u{feff}+++\r\ntitle = \"Article\"\r\ndate = 2026-10-12\r\n+++\r\n\r\n\r\n## Method\r\n\r\n[@a]\r\n\r\n{{ <fig id=\"plot\" /> }}\r\n";
        let ir = parse_article(Path::new("a.md"), source).unwrap();
        assert_eq!(ir.citations[0].line, 9);
        let section = ir
            .sections
            .iter()
            .find(|section| section.title == "Method")
            .unwrap();
        assert_eq!(section.line_start, 7);
        assert_eq!(ir.citations[0].section_id, section.id);
        assert_eq!(ir.figures[0].line, 11);
        assert_eq!(ir.figures[0].section_id, section.id);
    }

    #[test]
    fn malformed_groups_are_ignored_without_partial_keys_or_utf8_panics() {
        let text = "中文 [@a; missing] [@a b] [@a; @] [@a@domain] [@good:2026, p. 三]";
        let citations = extract_article_citations(text, &[], &[0], 1, Vec::new());
        assert_eq!(citations.len(), 1);
        assert_eq!(citations[0].key, "good:2026");
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
            1,
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
            1,
            &mut figures,
        );
        assert!(figures.is_empty());
    }

    #[test]
    fn test_figure_alt_falls_back_to_caption() {
        let mut figures = Vec::new();
        extract_figures(
            "{{ <fig id=\"p\" caption=\"Cap\" /> }}",
            &[],
            1,
            &mut figures,
        );
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
