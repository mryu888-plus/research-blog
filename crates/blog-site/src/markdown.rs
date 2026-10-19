//! Markdown structure shared by math protection and citation formatting.
//!
//! Both transforms skip the same syntax: frontmatter, fenced and indented
//! code, link reference definitions, code spans, HTML, template tags, image
//! alt text and link destinations. Offsets are byte offsets into UTF-8 text;
//! every delimiter is ASCII, so byte comparisons never split a character.

use regex::Regex;
use std::sync::LazyLock;

/// Python `str.isspace`: Unicode White_Space plus the ASCII separators.
pub fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\x1c'..='\x1f').contains(&c)
}

/// Numeric characters that Python's `str.isdigit` rejects (fractions,
/// Roman numerals and other Numeric_Type=Numeric values), from Unicode 15.1.0.
const NOT_DIGIT: &[(u32, u32)] = &[
    (0xBC, 0xBE),
    (0x9F4, 0x9F9),
    (0xB72, 0xB77),
    (0xBF0, 0xBF2),
    (0xC78, 0xC7E),
    (0xD58, 0xD5E),
    (0xD70, 0xD78),
    (0xF2A, 0xF33),
    (0x1372, 0x137C),
    (0x16EE, 0x16F0),
    (0x17F0, 0x17F9),
    (0x2150, 0x2182),
    (0x2185, 0x2189),
    (0x2469, 0x2473),
    (0x247D, 0x2487),
    (0x2491, 0x249B),
    (0x24EB, 0x24F4),
    (0x24FE, 0x24FE),
    (0x277F, 0x277F),
    (0x2789, 0x2789),
    (0x2793, 0x2793),
    (0x2CFD, 0x2CFD),
    (0x3007, 0x3007),
    (0x3021, 0x3029),
    (0x3038, 0x303A),
    (0x3192, 0x3195),
    (0x3220, 0x3229),
    (0x3248, 0x324F),
    (0x3251, 0x325F),
    (0x3280, 0x3289),
    (0x32B1, 0x32BF),
    (0xA6E6, 0xA6EF),
    (0xA830, 0xA835),
    (0x10107, 0x10133),
    (0x10140, 0x10178),
    (0x1018A, 0x1018B),
    (0x102E1, 0x102FB),
    (0x10320, 0x10323),
    (0x10341, 0x10341),
    (0x1034A, 0x1034A),
    (0x103D1, 0x103D5),
    (0x10858, 0x1085F),
    (0x10879, 0x1087F),
    (0x108A7, 0x108AF),
    (0x108FB, 0x108FF),
    (0x10916, 0x1091B),
    (0x109BC, 0x109BD),
    (0x109C0, 0x109CF),
    (0x109D2, 0x109FF),
    (0x10A44, 0x10A48),
    (0x10A7D, 0x10A7E),
    (0x10A9D, 0x10A9F),
    (0x10AEB, 0x10AEF),
    (0x10B58, 0x10B5F),
    (0x10B78, 0x10B7F),
    (0x10BA9, 0x10BAF),
    (0x10CFA, 0x10CFF),
    (0x10E69, 0x10E7E),
    (0x10F1D, 0x10F26),
    (0x10F51, 0x10F54),
    (0x10FC5, 0x10FCB),
    (0x1105B, 0x11065),
    (0x111E1, 0x111F4),
    (0x1173A, 0x1173B),
    (0x118EA, 0x118F2),
    (0x11C5A, 0x11C6C),
    (0x11FC0, 0x11FD4),
    (0x12400, 0x1246E),
    (0x16B5B, 0x16B61),
    (0x16E80, 0x16E96),
    (0x1D2C0, 0x1D2D3),
    (0x1D2E0, 0x1D2F3),
    (0x1D360, 0x1D378),
    (0x1E8C7, 0x1E8CF),
    (0x1EC71, 0x1ECAB),
    (0x1ECAD, 0x1ECAF),
    (0x1ECB1, 0x1ECB4),
    (0x1ED01, 0x1ED2D),
    (0x1ED2F, 0x1ED3D),
    (0x1F10B, 0x1F10C),
];

/// Python `str.isdigit`: decimal digits and digit forms such as superscripts.
pub fn is_digit(c: char) -> bool {
    let code = c as u32;
    c.is_numeric()
        && NOT_DIGIT
            .binary_search_by(|&(low, high)| {
                if high < code {
                    std::cmp::Ordering::Less
                } else if low > code {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_err()
}

pub fn is_blank(s: &str) -> bool {
    s.chars().all(is_space)
}

pub fn char_at(text: &str, at: usize) -> Option<char> {
    text.get(at..).and_then(|rest| rest.chars().next())
}

pub fn char_before(text: &str, at: usize) -> Option<char> {
    text.get(..at).and_then(|head| head.chars().next_back())
}

/// Python `str.expandtabs(4)`.
fn expand_tabs(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let mut column = 0;
    for c in line.chars() {
        match c {
            '\t' => {
                let spaces = 4 - column % 4;
                result.extend(std::iter::repeat_n(' ', spaces));
                column += spaces;
            }
            '\n' | '\r' => {
                result.push(c);
                column = 0;
            }
            _ => {
                result.push(c);
                column += 1;
            }
        }
    }
    result
}

/// Python `str.splitlines(keepends=True)`.
pub fn split_lines_keep(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((index, c)) = chars.next() {
        let end = match c {
            '\r' => match chars.peek() {
                Some((next, '\n')) => {
                    let next = *next;
                    chars.next();
                    next + 1
                }
                _ => index + 1,
            },
            '\n' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}'
            | '\u{2029}' => index + c.len_utf8(),
            _ => continue,
        };
        lines.push(&text[start..end]);
        start = end;
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

static FRONTMATTER_OPEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A\x{feff}?(\+\+\+|---)[ \t]*\r?\n").unwrap());
static TOML_CLOSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\+\+\+[ \t]*(?:\r?\n|$)").unwrap());
static YAML_CLOSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^---[ \t]*(?:\r?\n|$)").unwrap());

pub struct Frontmatter<'a> {
    pub delimiter: &'a str,
    pub raw: &'a str,
    /// Byte offset where the Markdown body starts.
    pub body_start: usize,
}

pub enum FrontmatterScan<'a> {
    None,
    Unclosed,
    Found(Frontmatter<'a>),
}

pub fn frontmatter(text: &str) -> FrontmatterScan<'_> {
    let Some(opening) = FRONTMATTER_OPEN.captures(text) else {
        return FrontmatterScan::None;
    };
    let delimiter = opening.get(1).unwrap().as_str();
    let after = opening.get(0).unwrap().end();
    let closing = if delimiter == "+++" {
        &TOML_CLOSE
    } else {
        &YAML_CLOSE
    };
    match closing.find(&text[after..]) {
        None => FrontmatterScan::Unclosed,
        Some(found) => FrontmatterScan::Found(Frontmatter {
            delimiter,
            raw: &text[after..after + found.start()],
            body_start: after + found.end(),
        }),
    }
}

/// The Markdown body without TOML or YAML frontmatter.
pub fn markdown_body(text: &str) -> &str {
    match frontmatter(text) {
        FrontmatterScan::Found(found) => &text[found.body_start..],
        _ => text,
    }
}

pub fn escaped(text: &str, index: usize) -> bool {
    let bytes = text.as_bytes();
    let mut preceding = index;
    while preceding > 0 && bytes[preceding - 1] == b'\\' {
        preceding -= 1;
    }
    (index - preceding) % 2 == 1
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Container {
    Quote,
    /// Content indentation, in characters.
    List(usize),
}

static QUOTE_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A {0,3}> ?").unwrap());
static LIST_MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A {0,3}(?:[-+*]|\d{1,9}[.)])( +)").unwrap());
static FENCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A {0,3}(`{3,}|~{3,})([^\r\n]*)").unwrap());
static LINK_DEFINITION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A {0,3}\[[^\]\r\n]+\]:").unwrap());
static QUOTE_IN_MATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A[ \t]*>[ \t]?").unwrap());

fn skip_chars(line: &str, count: usize) -> &str {
    match line.char_indices().nth(count) {
        Some((index, _)) => &line[index..],
        None => "",
    }
}

/// Consume existing list/quote prefixes in their original nesting order.
fn continue_containers<'a>(
    mut line: &'a str,
    containers: &[Container],
) -> (&'a str, Vec<Container>) {
    let mut matched = Vec::new();
    for &container in containers {
        match container {
            Container::Quote => match QUOTE_PREFIX.find(line) {
                Some(prefix) => line = &line[prefix.end()..],
                None => break,
            },
            // A blank line may remain in a list, but an omitted quote marker
            // still ends a quoted code block nested inside that list.
            Container::List(_) if is_blank(line) => {}
            Container::List(width) => {
                if line.chars().take(width).filter(|&c| c == ' ').count() == width {
                    line = skip_chars(line, width);
                } else {
                    break;
                }
            }
        }
        matched.push(container);
    }
    (line, matched)
}

/// Recognize additional quote/list prefixes, only outside fenced code.
fn open_containers<'a>(mut line: &'a str, containers: &[Container]) -> (&'a str, Vec<Container>) {
    let mut opened = containers.to_vec();
    loop {
        if let Some(quote) = QUOTE_PREFIX.find(line) {
            opened.push(Container::Quote);
            line = &line[quote.end()..];
            continue;
        }
        if let Some(marker) = LIST_MARKER.captures(line) {
            // Five or more spaces after a marker mean one padding space plus
            // an indented code block, rather than arbitrarily deep padding.
            let padding = marker.get(1).unwrap();
            let start = line[..padding.start()].chars().count();
            let spaces = padding.as_str().len();
            let width = start + if spaces <= 4 { spaces } else { 1 };
            opened.push(Container::List(width));
            line = skip_chars(line, width);
            continue;
        }
        break;
    }
    (line, opened)
}

pub fn unquote_math(tex: &str, depth: usize) -> String {
    if depth == 0 {
        return tex.to_string();
    }
    let lines = split_lines_keep(tex);
    let mut result = String::with_capacity(tex.len());
    for (number, &line) in lines.iter().enumerate() {
        let mut line = line;
        if number > 0 {
            // A quote inside an ordered or nested list can be preceded by
            // more than three indentation spaces in the original source.
            for _ in 0..depth {
                match QUOTE_IN_MATH.find(line) {
                    Some(found) => line = &line[found.end()..],
                    None => break,
                }
            }
        }
        result.push_str(line);
    }
    result
}

/// Byte offset of the closing delimiter, or `None` when the math is not closed.
pub fn math_end(
    text: &str,
    start: usize,
    opening: &str,
    closing: &str,
    display: bool,
) -> Option<usize> {
    let begin = start + opening.len();
    if !display && char_at(text, begin).is_none_or(is_space) {
        return None;
    }
    let bytes = text.as_bytes();
    let mut candidate = begin;
    while let Some(found) = text[candidate..].find(closing) {
        candidate += found;
        if !display && text[begin..candidate].contains(['\n', '\r']) {
            return None;
        }
        if !escaped(text, candidate) {
            let after = candidate + closing.len();
            if closing == "$" {
                // Single dollars neither consume display delimiters nor turn
                // prices like "$5 and $10" into a formula.
                if candidate > begin
                    && !char_before(text, candidate).is_some_and(is_space)
                    && bytes[candidate - 1] != b'$'
                    && char_at(text, after).is_none_or(|c| c != '$' && !is_digit(c))
                {
                    return Some(candidate);
                }
            } else if closing != "$$"
                || (bytes[candidate - 1] != b'$' && bytes.get(after) != Some(&b'$'))
            {
                return Some(candidate);
            }
        }
        candidate += closing.len();
    }
    None
}

static HTML_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A</?([a-zA-Z][a-zA-Z0-9:-]*)").unwrap());
static AUTOLINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\A<(?:[a-zA-Z][a-zA-Z0-9+.\-]*:[^<>\s]*|[^<>\s]+@[^<>\s]+)>").unwrap()
});

/// Skip tag attributes, comments, autolinks, and verbatim HTML elements.
pub fn html_end(text: &str, start: usize) -> Option<usize> {
    let rest = &text[start..];
    if rest.starts_with("<!--") {
        return Some(
            text[start + 4..]
                .find("-->")
                .map_or(text.len(), |end| start + 4 + end + 3),
        );
    }
    if rest.starts_with("<![CDATA[") {
        return Some(
            text[start + 9..]
                .find("]]>")
                .map_or(text.len(), |end| start + 9 + end + 3),
        );
    }
    let tag = HTML_TAG.captures(rest).filter(|tag| {
        char_at(rest, tag.get(0).unwrap().end())
            .is_some_and(|c| is_space(c) || c == '/' || c == '>')
    });
    let Some(tag) = tag else {
        return AUTOLINK.find(rest).map(|found| start + found.end());
    };
    let name = tag.get(1).unwrap().as_str();
    let bytes = text.as_bytes();
    let mut quote = None;
    let mut end = start + tag.get(0).unwrap().end();
    while end < bytes.len() {
        let character = bytes[end];
        if let Some(open) = quote {
            if character == open {
                quote = None;
            }
        } else if character == b'"' || character == b'\'' {
            quote = Some(character);
        } else if character == b'>' {
            end += 1;
            break;
        }
        end += 1;
    }
    let verbatim = ["pre", "code", "script", "style", "textarea", "math", "svg"];
    if !rest.starts_with("</") && verbatim.contains(&name.to_ascii_lowercase().as_str()) {
        let closing = Regex::new(&format!(r"(?i)</{}\s*>", regex::escape(name))).unwrap();
        return Some(
            closing
                .find(&text[end..])
                .map_or(text.len(), |found| end + found.end()),
        );
    }
    Some(end)
}

pub fn destination_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut depth = 1;
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => {
                cursor += 2;
                continue;
            }
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return cursor + 1;
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    start + 1
}

/// End of a code span opened by the backtick run at `cursor`: the next run of
/// exactly the same length, or just past the opening run when there is none.
fn code_span_end(text: &str, cursor: usize) -> usize {
    let bytes = text.as_bytes();
    let run = bytes[cursor..].iter().take_while(|&&b| b == b'`').count();
    let mut index = cursor + run;
    while index < bytes.len() {
        if bytes[index] == b'`' {
            let length = bytes[index..].iter().take_while(|&&b| b == b'`').count();
            if length == run {
                return index + length;
            }
            index += length;
        } else {
            index += 1;
        }
    }
    cursor + run
}

/// First `]` not preceded by a backslash, searching from `from`.
fn image_alt_end(text: &str, from: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    (from..bytes.len())
        .find(|&i| bytes[i] == b']' && (i == from || bytes[i - 1] != b'\\'))
        .map(|i| i + 1)
}

pub struct MathSpan {
    pub opening: &'static str,
    pub closing: &'static str,
    pub display: bool,
    /// Byte offset of the closing delimiter.
    pub end: usize,
}

/// Math beginning at `cursor`, if a closed formula starts there.
pub fn math_at(text: &str, cursor: usize) -> Option<MathSpan> {
    if escaped(text, cursor) {
        return None;
    }
    let rest = &text[cursor..];
    let bytes = text.as_bytes();
    let (opening, closing, display) =
        if rest.starts_with("$$") && bytes.get(cursor + 2) != Some(&b'$') {
            ("$$", "$$", true)
        } else if rest.starts_with('$') && (cursor == 0 || bytes[cursor - 1] != b'$') {
            ("$", "$", false)
        } else if rest.starts_with("\\(") {
            ("\\(", "\\)", false)
        } else if rest.starts_with("\\[") {
            ("\\[", "\\]", true)
        } else {
            return None;
        };
    math_end(text, cursor, opening, closing, display).map(|end| MathSpan {
        opening,
        closing,
        display,
        end,
    })
}

/// Line-level Markdown state: containers, fenced and indented code.
#[derive(Default)]
pub struct Scanner {
    fence: Option<(u8, usize, Vec<Container>)>,
    code_indent: bool,
    previous_blank: bool,
    containers: Vec<Container>,
    /// Blockquote depth of the current line, used to unquote multi-line math.
    pub quote_depth: usize,
}

impl Scanner {
    pub fn new() -> Self {
        Scanner {
            previous_blank: true,
            ..Default::default()
        }
    }

    /// At a line start, the end of the line when the whole line is literal
    /// (code or a link reference definition). Otherwise updates the state.
    pub fn literal_line(&mut self, text: &str, cursor: usize) -> Option<usize> {
        if cursor != 0 && text.as_bytes()[cursor - 1] != b'\n' {
            return None;
        }
        let line_end = text[cursor..]
            .find('\n')
            .map_or(text.len(), |end| cursor + end + 1);
        let expanded = expand_tabs(&text[cursor..line_end]);
        let (mut logical, continued) = continue_containers(&expanded, &self.containers);
        if self
            .fence
            .as_ref()
            .is_some_and(|fence| fence.2 != continued)
        {
            // CommonMark closes a fenced block when its containing quote
            // or list item ends, even without an explicit closing fence.
            self.fence = None;
            self.previous_blank = true;
        }
        self.containers = continued;
        let mut opened_container = false;
        if self.fence.is_none() {
            let (rest, opened) = open_containers(logical, &self.containers);
            logical = rest;
            opened_container = opened != self.containers;
            self.containers = opened;
        }
        self.quote_depth = self
            .containers
            .iter()
            .filter(|c| **c == Container::Quote)
            .count();
        let indent = logical.len() - logical.trim_start_matches(' ').len();
        let blank = is_blank(logical);
        let possible_fence = FENCE.captures(logical);
        if let Some((character, length, _)) = &self.fence {
            if let Some(found) = &possible_fence {
                let marker = found.get(1).unwrap().as_str();
                if marker.as_bytes()[0] == *character
                    && marker.len() >= *length
                    && is_blank(found.get(2).unwrap().as_str())
                {
                    self.fence = None;
                }
            }
            self.previous_blank = blank || self.fence.is_none();
            return Some(line_end);
        }
        if let Some(found) = &possible_fence {
            let marker = found.get(1).unwrap().as_str();
            let info = found.get(2).unwrap().as_str();
            if !(marker.starts_with('`') && info.contains('`')) {
                self.fence = Some((marker.as_bytes()[0], marker.len(), self.containers.clone()));
                self.code_indent = false;
                self.previous_blank = blank;
                return Some(line_end);
            }
        }
        if (self.code_indent && (blank || indent >= 4))
            || ((self.previous_blank || opened_container) && indent >= 4)
        {
            self.code_indent = true;
            self.previous_blank = blank;
            return Some(line_end);
        }
        self.code_indent = false;
        self.previous_blank = blank;
        if LINK_DEFINITION.is_match(logical) {
            return Some(line_end);
        }
        None
    }
}

/// Inline syntax at `cursor` that must be copied verbatim: code spans, HTML,
/// template tags, image alt text and link destinations.
pub fn literal_inline(text: &str, cursor: usize) -> Option<usize> {
    let rest = &text[cursor..];
    let bytes = text.as_bytes();
    match bytes[cursor] {
        b'`' => Some(code_span_end(text, cursor)),
        b'<' => html_end(text, cursor),
        b'{' if rest.starts_with("{{") || rest.starts_with("{%") => {
            let close = if rest.starts_with("{{") { "}}" } else { "%}" };
            text[cursor + 2..]
                .find(close)
                .map(|end| cursor + 2 + end + 2)
        }
        b'!' if rest.starts_with("![") => image_alt_end(text, cursor + 2),
        b'(' if cursor > 0 && bytes[cursor - 1] == b']' => Some(destination_end(text, cursor)),
        _ => None,
    }
}

pub fn next_char_end(text: &str, cursor: usize) -> usize {
    cursor + char_at(text, cursor).map_or(1, char::len_utf8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_lines_like_python() {
        assert_eq!(
            split_lines_keep("a\r\nb\rc\nd"),
            ["a\r\n", "b\r", "c\n", "d"]
        );
        assert_eq!(split_lines_keep("x\u{2028}y"), ["x\u{2028}", "y"]);
    }

    #[test]
    fn expands_tabs_by_column() {
        assert_eq!(expand_tabs("a\tb\n\tc"), "a   b\n    c");
    }

    #[test]
    fn code_span_needs_equal_run() {
        assert_eq!(code_span_end("``a`b``c", 0), 7);
        assert_eq!(code_span_end("`a", 0), 1);
    }
}
