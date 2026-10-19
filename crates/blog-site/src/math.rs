//! Protect TeX before Zola parses Markdown or expands template shortcodes.
//!
//! Only the prepared build copy is transformed. Numeric entities keep every TeX
//! character (including table pipes and template braces) out of Markdown's grammar.

use crate::markdown::{
    frontmatter, literal_inline, math_at, next_char_end, unquote_math, FrontmatterScan, Scanner,
};
use std::fmt::Write;

fn entities(value: &str, output: &mut String) {
    for character in value.chars() {
        write!(output, "&#{};", character as u32).unwrap();
    }
}

fn placeholder(tex: &str, display: bool, opening: &str, closing: &str, output: &mut String) {
    output.push_str("<span class=\"math-source\" data-math-tex=\"");
    entities(tex, output);
    output.push_str("\" data-math-display=\"");
    output.push_str(if display { "true" } else { "false" });
    output.push_str("\">");
    entities(opening, output);
    entities(tex, output);
    entities(closing, output);
    output.push_str("</span>");
}

/// Protect dollar and LaTeX bracket math, leaving code and metadata alone.
pub fn protect_math(text: &str) -> String {
    let mut cursor = match frontmatter(text) {
        FrontmatterScan::Unclosed => return text.to_string(),
        FrontmatterScan::Found(found) => found.body_start,
        FrontmatterScan::None => 0,
    };
    let mut result = String::with_capacity(text.len() + text.len() / 4);
    result.push_str(&text[..cursor]);
    let mut scanner = Scanner::new();
    while cursor < text.len() {
        if let Some(end) = scanner.literal_line(text, cursor) {
            result.push_str(&text[cursor..end]);
            cursor = end;
            continue;
        }
        if let Some(end) = literal_inline(text, cursor) {
            result.push_str(&text[cursor..end]);
            cursor = end;
            continue;
        }
        if let Some(math) = math_at(text, cursor) {
            let tex = unquote_math(
                &text[cursor + math.opening.len()..math.end],
                scanner.quote_depth,
            );
            placeholder(&tex, math.display, math.opening, math.closing, &mut result);
            cursor = math.end + math.closing.len();
            continue;
        }
        let next = next_char_end(text, cursor);
        result.push_str(&text[cursor..next]);
        cursor = next;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protects_inline_and_display_math() {
        assert_eq!(
            protect_math("$x$"),
            "<span class=\"math-source\" data-math-tex=\"&#120;\" data-math-display=\"false\">&#36;&#120;&#36;</span>"
        );
        assert!(protect_math("$$\na\n$$").contains("data-math-display=\"true\""));
    }

    #[test]
    fn leaves_currency_code_and_frontmatter() {
        for source in [
            "$5 and $10, then $20",
            "\\$100",
            "$ unfinished",
            "$x",
            "$ x $",
            "$x $",
            "$x\ny$",
        ] {
            assert_eq!(protect_math(source), source);
        }
        let text = "+++\ntitle = \"$x$\"\n+++\n```\n$x$\n```\n`$y$`\n";
        assert_eq!(protect_math(text), text);
        assert_eq!(protect_math("+++\ntitle = \"$x$\""), "+++\ntitle = \"$x$\"");
    }
}
