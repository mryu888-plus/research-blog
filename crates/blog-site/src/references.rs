//! Offline citations for the prepared Zola tree, with a public-only bibliography.
//!
//! The authoring tree remains untouched. Zotero exports the small, explicit CSL
//! subset below; neither attachments nor annotations belong in that snapshot.
//! Article-local refs.bib is also accepted for existing manuscripts. This is a
//! strict parser of standard BibTeX entry/value syntax, not a TeX interpreter.

use crate::markdown::{
    frontmatter, is_space, literal_inline, math_at, next_char_end, split_lines_keep,
    FrontmatterScan, Scanner,
};
use crate::paths::RelPath;
use anyhow::{anyhow, bail, Result};
use regex::Regex;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

pub const SNAPSHOT: &str = "data/references.json";
pub const INDEX_PAGE: &str = "content/references.md";
const ITEM_FIELDS: [&str; 13] = [
    "id",
    "citekey",
    "title",
    "author",
    "issued",
    "type",
    "container-title",
    "DOI",
    "URL",
    "volume",
    "issue",
    "page",
    "publisher",
];
const KEY: &str = r#"[^\s@;,\[\]{}<>"\\]+"#;
static KEY_FULL: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"\A{KEY}\z")).unwrap());
static CITATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"\A\[\s*@({KEY})([^\]\r\n]*)\]")).unwrap());
static PART: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"\A\s*@({KEY})\s*(?:,\s*(.*))?\s*\z")).unwrap());

pub type Item = Map<String, Value>;
pub type Items = HashMap<String, Item>;

/// Python `repr` of a string, as used in error messages.
fn repr(value: &str) -> String {
    if value.contains('\'') && !value.contains('"') {
        format!("\"{value}\"")
    } else {
        format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
    }
}

pub fn escape_html(value: &str) -> String {
    // Also prevent shortcode expansion and later TeX transformation in generated
    // Markdown HTML. HTML escaping alone does not protect braces or dollars.
    let mut result = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#x27;"),
            '{' => result.push_str("&#123;"),
            '}' => result.push_str("&#125;"),
            '$' => result.push_str("&#36;"),
            _ => result.push(c),
        }
    }
    result
}

pub fn anchor(key: &str) -> String {
    let digest = hex::encode(Sha256::digest(key.as_bytes()));
    format!("ref-{}", &digest[..20])
}

fn text_field<'a>(value: Option<&'a Value>, context: &str) -> Result<&'a str> {
    match value {
        Some(Value::String(text))
            if !text
                .chars()
                .any(|c| (c as u32) < 32 && !"\n\r\t".contains(c)) =>
        {
            Ok(text)
        }
        _ => bail!("{context} must be text without control characters"),
    }
}

fn python_strip(value: &str) -> &str {
    value.trim_matches(is_space)
}

/// Reject rather than silently discard private/unknown snapshot fields.
pub fn validate_snapshot(value: &Value) -> Result<Vec<(String, Item)>> {
    let object = value
        .as_object()
        .filter(|o| o.len() == 2 && o.contains_key("version") && o.contains_key("items"));
    let Some(object) = object else {
        bail!("references.json requires only version and items; private fields are forbidden");
    };
    let version_ok = object["version"].as_i64() == Some(1) || object["version"].as_u64() == Some(1);
    let Some(items) = object["items"].as_array().filter(|_| version_ok) else {
        bail!("references.json must have version 1 and an items list");
    };
    let mut result: Vec<(String, Item)> = Vec::new();
    let mut seen = HashSet::new();
    for (number, item) in items.iter().enumerate() {
        let context = format!("references.json item {}", number + 1);
        let Some(item) = item.as_object() else {
            bail!("{context}: forbidden fields: non-object");
        };
        let mut unknown: Vec<&str> = item
            .keys()
            .map(String::as_str)
            .filter(|key| !ITEM_FIELDS.contains(key))
            .collect();
        if !unknown.is_empty() {
            unknown.sort();
            bail!("{context}: forbidden fields: {}", unknown.join(", "));
        }
        for required in ["id", "citekey", "title"] {
            if python_strip(text_field(
                item.get(required),
                &format!("{context}.{required}"),
            )?)
            .is_empty()
            {
                bail!("{context}.{required} cannot be empty");
            }
        }
        let key = item["citekey"].as_str().unwrap();
        if !KEY_FULL.is_match(key) {
            bail!("{context}: invalid citekey {}", repr(key));
        }
        if !seen.insert(key.to_string()) {
            bail!("references.json: duplicate citekey {}", repr(key));
        }
        for name in ITEM_FIELDS
            .iter()
            .filter(|name| !["author", "issued"].contains(name))
        {
            if item.contains_key(*name) {
                text_field(item.get(*name), &format!("{context}.{name}"))?;
            }
        }
        if let Some(authors) = item.get("author") {
            let Some(authors) = authors.as_array() else {
                bail!("{context}.author must be a list");
            };
            for author in authors {
                let valid = author.as_object().filter(|a| {
                    !a.is_empty()
                        && a.keys()
                            .all(|k| ["family", "given", "literal"].contains(&k.as_str()))
                        && (!a.contains_key("literal") || a.len() == 1)
                });
                let Some(author) = valid else {
                    bail!("{context}.author: unsupported or private name fields");
                };
                for (name, text) in author {
                    text_field(Some(text), &format!("{context}.author.{name}"))?;
                }
            }
        }
        if let Some(issued) = item.get("issued") {
            let parts = issued
                .as_object()
                .filter(|o| o.len() == 1)
                .and_then(|o| o.get("date-parts"))
                .and_then(Value::as_array)
                .filter(|parts| !parts.is_empty());
            let Some(parts) = parts else {
                bail!("{context}.issued requires date-parts only");
            };
            for date in parts {
                let valid = date.as_array().is_some_and(|d| {
                    (1..=3).contains(&d.len()) && d.iter().all(|p| p.is_i64() || p.is_u64())
                });
                if !valid {
                    bail!("{context}.issued: invalid date-parts");
                }
            }
        }
        result.push((key.to_string(), item.clone()));
    }
    Ok(result)
}

/// The parts of Python's `urlsplit` that decide whether a URL is public.
fn public_url(value: &str) -> Option<&str> {
    if value.chars().any(|c| is_space(c) || (c as u32) < 32) {
        return None;
    }
    let mut rest = value;
    let mut scheme = String::new();
    if let Some(colon) = value.find(':') {
        let candidate = &value[..colon];
        if colon > 0
            && candidate.as_bytes()[0].is_ascii_alphabetic()
            && candidate
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
        {
            scheme = candidate.to_ascii_lowercase();
            rest = &value[colon + 1..];
        }
    }
    let netloc = match rest.strip_prefix("//") {
        Some(after) => &after[..after.find(['/', '?', '#']).unwrap_or(after.len())],
        None => "",
    };
    if netloc.contains('[') != netloc.contains(']') {
        return None;
    }
    let (userinfo, hostinfo) = match netloc.rfind('@') {
        Some(at) => (Some(&netloc[..at]), &netloc[at + 1..]),
        None => (None, netloc),
    };
    let hostname = match hostinfo.split_once('[') {
        Some((_, bracketed)) => bracketed.split(']').next().unwrap_or(""),
        None => hostinfo.split(':').next().unwrap_or(""),
    }
    .to_lowercase();
    let (username, password) = match userinfo {
        Some(info) => match info.split_once(':') {
            Some((user, password)) => (user, Some(password)),
            None => (info, None),
        },
        None => ("", None),
    };
    if !["http", "https"].contains(&scheme.as_str())
        || hostname.is_empty()
        || !username.is_empty()
        || password.is_some_and(|p| !p.is_empty())
    {
        return None;
    }
    // DOI/URL fields containing localhost or local file paths are never linked.
    if ["localhost", "127.0.0.1", "::1", "0.0.0.0"].contains(&hostname.as_str()) {
        return None;
    }
    Some(value)
}

static DOI_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\A(?:doi:\s*|https?://(?:dx\.)?doi\.org/)").unwrap());
static DOI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A10\.\d{4,9}/\S+\z").unwrap());

fn doi_url(value: &str) -> Option<String> {
    let stripped = DOI_PREFIX.replace(value, "");
    let value = python_strip(&stripped);
    if !DOI.is_match(value) {
        return None;
    }
    let mut url = String::from("https://doi.org/");
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"_.-~/():;".contains(&byte) {
            url.push(byte as char);
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    Some(url)
}

fn str_of(item: &Item, key: &str) -> Option<String> {
    match item.get(key)? {
        Value::String(text) => Some(text.clone()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

/// Render a readable numeric bibliography using only allowlisted fields.
pub fn format_item(item: &Item) -> String {
    let mut pieces = Vec::new();
    let authors: Vec<String> = item
        .get("author")
        .and_then(Value::as_array)
        .map(|authors| {
            authors
                .iter()
                .map(|author| {
                    let get = |key| author.get(key).and_then(Value::as_str).unwrap_or("");
                    if !get("literal").is_empty() {
                        get("literal").to_string()
                    } else {
                        [get("given"), get("family")]
                            .into_iter()
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join(" ")
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    if !authors.is_empty() {
        pieces.push(escape_html(&authors.join("; ")) + ".");
    }
    if let Some(year) = item
        .get("issued")
        .and_then(|i| i.get("date-parts"))
        .and_then(|p| p.get(0))
        .and_then(|d| d.get(0))
    {
        pieces.push(escape_html(&year.to_string()) + ".");
    }
    pieces.push(format!(
        "<span class=\"reference-title\">{}</span>.",
        escape_html(&str_of(item, "title").unwrap_or_default())
    ));
    let field = |key: &str| str_of(item, key).filter(|s| !s.is_empty());
    if let Some(container) = field("container-title") {
        pieces.push(format!("<em>{}</em>.", escape_html(&container)));
    }
    if let Some(volume) = field("volume") {
        let mut volume = escape_html(&volume);
        if let Some(issue) = field("issue") {
            volume += &format!("({})", escape_html(&issue));
        }
        pieces.push(volume + ".");
    } else if let Some(issue) = field("issue") {
        pieces.push(escape_html(&issue) + ".");
    }
    for key in ["page", "publisher"] {
        if let Some(value) = field(key) {
            pieces.push(escape_html(&value) + ".");
        }
    }
    let mut links = Vec::new();
    if let Some(url) = field("DOI").and_then(|doi| doi_url(&doi)) {
        links.push(format!(
            "<a href=\"{}\" rel=\"noopener noreferrer\">DOI</a>",
            escape_html(&url)
        ));
    }
    if let Some(url) = field("URL").filter(|url| public_url(url).is_some()) {
        links.push(format!(
            "<a href=\"{}\" rel=\"noopener noreferrer\">原文</a>",
            escape_html(&url)
        ));
    }
    if !links.is_empty() {
        pieces.push(format!(
            "<span class=\"reference-links\">{}</span>",
            links.join(" · ")
        ));
    }
    pieces.join(" ")
}

pub type Metadata = toml::Table;

static YAML_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\A(title|slug|path|draft|render):\s*(.*?)\s*\z").unwrap());

/// Python `str.splitlines()` without line endings.
fn split_lines(text: &str) -> impl Iterator<Item = &str> {
    split_lines_keep(text).into_iter().map(|line| {
        let trimmed = line.strip_suffix("\r\n").unwrap_or(line);
        if trimmed.len() < line.len() {
            trimmed
        } else {
            let mut chars = line.chars();
            match chars.next_back() {
                Some(
                    '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}'
                    | '\u{2028}' | '\u{2029}',
                ) => chars.as_str(),
                _ => line,
            }
        }
    })
}

/// Frontmatter metadata and the byte offset of the Markdown body.
pub fn parse_frontmatter(text: &str) -> Result<(Metadata, usize)> {
    let found = match frontmatter(text) {
        FrontmatterScan::None => return Ok((Metadata::new(), 0)),
        FrontmatterScan::Unclosed => bail!("Unclosed Markdown frontmatter"),
        FrontmatterScan::Found(found) => found,
    };
    if found.delimiter == "+++" {
        return Ok((found.raw.parse::<Metadata>()?, found.body_start));
    }
    // Only routing/publication metadata is needed from YAML. Flat YAML scalars
    // cover existing authoring conventions without adding a runtime dependency.
    let mut metadata = Metadata::new();
    for line in split_lines(found.raw) {
        let Some(captures) = YAML_LINE.captures(line) else {
            continue;
        };
        let key = captures[1].to_string();
        let value = &captures[2];
        let parsed = if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
            toml::Value::Boolean(value.eq_ignore_ascii_case("true"))
        } else if value.starts_with('"') {
            toml::Value::String(serde_json::from_str::<String>(value)?)
        } else if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
            toml::Value::String(value[1..value.len() - 1].replace("''", "'"))
        } else {
            toml::Value::String(value.split(" #").next().unwrap().to_string())
        };
        metadata.insert(key, parsed);
    }
    Ok((metadata, found.body_start))
}

pub fn truthy(value: Option<&toml::Value>) -> bool {
    match value {
        None => false,
        Some(toml::Value::Boolean(b)) => *b,
        Some(toml::Value::String(s)) => !s.is_empty(),
        Some(toml::Value::Integer(i)) => *i != 0,
        Some(toml::Value::Float(f)) => *f != 0.0,
        Some(toml::Value::Array(a)) => !a.is_empty(),
        Some(toml::Value::Table(t)) => !t.is_empty(),
        Some(toml::Value::Datetime(_)) => true,
    }
}

fn reference_index(metadata: &Metadata) -> bool {
    truthy(
        metadata
            .get("extra")
            .and_then(|extra| extra.as_table())
            .and_then(|extra| extra.get("reference_index")),
    )
}

/// Replace visible citation groups, preserving code, math and metadata.
pub fn transform_citations(
    text: &str,
    items: &Items,
    source: &str,
) -> Result<(String, Vec<String>)> {
    let (_, mut cursor) = parse_frontmatter(text)?;
    let mut result = String::with_capacity(text.len());
    result.push_str(&text[..cursor]);
    let mut order: Vec<String> = Vec::new();
    let mut mentions: HashMap<String, Vec<String>> = HashMap::new();
    let mut scanner = Scanner::new();
    while cursor < text.len() {
        let literal = scanner
            .literal_line(text, cursor)
            .or_else(|| literal_inline(text, cursor));
        if let Some(end) = literal {
            result.push_str(&text[cursor..end]);
            cursor = end;
            continue;
        }
        if let Some(math) = math_at(text, cursor) {
            let end = math.end + math.closing.len();
            result.push_str(&text[cursor..end]);
            cursor = end;
            continue;
        }
        if !crate::markdown::escaped(text, cursor) {
            if let Some(citation) = CITATION.find(&text[cursor..]) {
                let citation_end = cursor + citation.end();
                let next = text.as_bytes().get(citation_end);
                let inner = &text[cursor + 1..citation_end - 1];
                let parts: Option<Vec<_>> =
                    inner.split(';').map(|part| PART.captures(part)).collect();
                if let (false, Some(parts)) = (matches!(next, Some(b'(' | b'[')), parts) {
                    let mut links = Vec::new();
                    for part in parts {
                        let key = part[1].to_string();
                        let locator = part.get(2).map(|m| m.as_str()).filter(|l| !l.is_empty());
                        if !items.contains_key(&key) {
                            let line = text[..cursor].matches('\n').count() + 1;
                            bail!(
                                "{source}:{line}: missing reference @{key}; export it to data/references.json or add refs.bib"
                            );
                        }
                        let number = match order.iter().position(|k| *k == key) {
                            Some(index) => index + 1,
                            None => {
                                order.push(key.clone());
                                order.len()
                            }
                        };
                        let ids = mentions.entry(key.clone()).or_default();
                        let cite_id = format!("cite-{}-{}", anchor(&key), ids.len() + 1);
                        ids.push(cite_id.clone());
                        let label = match locator {
                            Some(locator) => format!("{number}, {}", python_strip(locator)),
                            None => number.to_string(),
                        };
                        links.push(format!(
                            "<a id=\"{cite_id}\" href=\"#{}\" aria-label=\"参考文献 {number}\">{}</a>",
                            anchor(&key),
                            escape_html(&label)
                        ));
                    }
                    result.push_str(&format!(
                        "<span class=\"citation\">[{}]</span>",
                        links.join("; ")
                    ));
                    cursor = citation_end;
                    continue;
                }
            }
        }
        let next = next_char_end(text, cursor);
        result.push_str(&text[cursor..next]);
        cursor = next;
    }
    if !order.is_empty() {
        let rows: Vec<String> = order
            .iter()
            .enumerate()
            .map(|(index, key)| {
                let number = index + 1;
                let ids = &mentions[key];
                let backlinks: Vec<String> = ids
                    .iter()
                    .enumerate()
                    .map(|(i, id)| {
                        let suffix = if ids.len() > 1 { (i + 1).to_string() } else { String::new() };
                        format!("<a href=\"#{id}\" aria-label=\"返回正文引用 {number}，第 {} 处\">↩{suffix}</a>", i + 1)
                    })
                    .collect();
                format!(
                    "<li id=\"{}\">{} <span class=\"reference-backlinks\">{}</span></li>",
                    anchor(key),
                    format_item(&items[key]),
                    backlinks.join(" ")
                )
            })
            .collect();
        result.push_str(
            "\n\n<section class=\"article-references\" aria-labelledby=\"reference-heading\">\n<h2 id=\"reference-heading\">参考文献</h2>\n<ol>\n",
        );
        result.push_str(&rows.join("\n"));
        result.push_str("\n</ol>\n</section>\n");
    }
    Ok((result, order))
}

static ACCENT: LazyLock<fancy_regex::Regex> = LazyLock::new(|| {
    fancy_regex::Regex::new(r#"\\(['`"^~=\.]|[vuHc](?=[{\s]))\s*\{?([A-Za-z])\}?"#).unwrap()
});
static TEX_COMMAND: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\(?:[A-Za-z]+|.)").unwrap());
static TEX_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\[A-Za-z]+").unwrap());

/// Decode common BibTeX text accents; fail on unhandled TeX commands.
fn tex_text(value: &str) -> Result<String> {
    let accented = ACCENT.replace_all(value, |captures: &fancy_regex::Captures| {
        let mark = match &captures[1] {
            "'" => '\u{301}',
            "`" => '\u{300}',
            "\"" => '\u{308}',
            "^" => '\u{302}',
            "~" => '\u{303}',
            "=" => '\u{304}',
            "." => '\u{307}',
            "v" => '\u{30c}',
            "u" => '\u{306}',
            "H" => '\u{30b}',
            _ => '\u{327}',
        };
        format!("{}{mark}", &captures[2]).nfc().collect::<String>()
    });
    let substituted = TEX_COMMAND.replace_all(&accented, |captures: &regex::Captures| {
        let command = &captures[0];
        match command {
            "\\&" => "&",
            "\\%" => "%",
            "\\_" => "_",
            "\\#" => "#",
            "\\$" => "$",
            "\\{" => "{",
            "\\}" => "}",
            "\\LaTeX" => "LaTeX",
            "\\TeX" => "TeX",
            "\\ss" => "ß",
            "\\ae" => "æ",
            "\\AE" => "Æ",
            "\\oe" => "œ",
            "\\OE" => "Œ",
            "\\o" => "ø",
            "\\O" => "Ø",
            "\\l" => "ł",
            "\\L" => "Ł",
            _ => command,
        }
        .to_string()
    });
    if TEX_WORD.is_match(&substituted) {
        bail!(
            "Unsupported TeX command in BibTeX public text: {}; export CSL JSON instead",
            repr(&substituted)
        );
    }
    let cleaned: String = substituted
        .chars()
        .filter(|c| *c != '{' && *c != '}')
        .collect();
    Ok(python_strip(&cleaned.replace('~', " ")).to_string())
}

struct BibParser<'a> {
    text: Vec<char>,
    source: &'a str,
    cursor: usize,
    strings: HashMap<String, String>,
}

type BibEntries = Vec<(String, String, HashMap<String, String>)>;

impl<'a> BibParser<'a> {
    fn new(text: &str, source: &'a str) -> Self {
        let strings = [
            "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
        ]
        .iter()
        .map(|m| {
            (
                m.to_string(),
                format!("{}{}", m[..1].to_uppercase(), &m[1..]),
            )
        })
        .collect();
        BibParser {
            text: text.chars().collect(),
            source,
            cursor: 0,
            strings,
        }
    }

    fn error(&self, message: String) -> anyhow::Error {
        let end = self.cursor.min(self.text.len());
        let line = self.text[..end].iter().filter(|c| **c == '\n').count() + 1;
        anyhow!("{}:{}: {}", self.source, line, message)
    }

    fn peek(&self) -> Option<char> {
        self.text.get(self.cursor).copied()
    }

    fn whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if is_space(c) {
                self.cursor += 1;
            } else if c == '%' {
                match self.text[self.cursor..].iter().position(|c| *c == '\n') {
                    Some(offset) => self.cursor += offset + 1,
                    None => self.cursor = self.text.len(),
                }
            } else {
                break;
            }
        }
    }

    fn word(&mut self) -> Result<String> {
        self.whitespace();
        let start = self.cursor;
        while self
            .peek()
            .is_some_and(|c| !is_space(c) && !"=,{}()#\"@".contains(c))
        {
            self.cursor += 1;
        }
        if start == self.cursor {
            return Err(self.error("Expected a BibTeX identifier".into()));
        }
        Ok(self.text[start..self.cursor].iter().collect())
    }

    fn expect(&mut self, token: char) -> Result<()> {
        self.whitespace();
        if self.peek() != Some(token) {
            return Err(self.error(format!("Expected {}", repr(&token.to_string()))));
        }
        self.cursor += 1;
        Ok(())
    }

    fn enclosed(&mut self, opening: char) -> Result<String> {
        self.cursor += 1;
        let start = self.cursor;
        let mut depth: i64 = 0;
        while self.cursor < self.text.len() {
            let character = self.text[self.cursor];
            if character == '\\' {
                self.cursor += 2;
                continue;
            }
            if character == '{' {
                depth += 1;
            } else if character == '}' {
                if opening == '{' && depth == 0 {
                    let value = self.text[start..self.cursor].iter().collect();
                    self.cursor += 1;
                    return Ok(value);
                }
                depth -= 1;
            } else if opening == '"' && character == '"' && depth == 0 {
                let value = self.text[start..self.cursor].iter().collect();
                self.cursor += 1;
                return Ok(value);
            }
            self.cursor += 1;
        }
        Err(self.error("Unclosed BibTeX value".into()))
    }

    fn value(&mut self) -> Result<String> {
        let mut values = Vec::new();
        loop {
            self.whitespace();
            let Some(c) = self.peek() else {
                return Err(self.error("Missing BibTeX field value".into()));
            };
            if c == '{' || c == '"' {
                values.push(self.enclosed(c)?);
            } else {
                let word = self.word()?;
                if word.chars().all(crate::markdown::is_digit) {
                    values.push(word);
                } else if let Some(value) = self.strings.get(&word.to_lowercase()) {
                    values.push(value.clone());
                } else {
                    return Err(self.error(format!("Unknown BibTeX string {}", repr(&word))));
                }
            }
            self.whitespace();
            if self.peek() != Some('#') {
                return Ok(values.concat());
            }
            self.cursor += 1;
        }
    }

    fn parse(&mut self) -> Result<BibEntries> {
        let mut entries: BibEntries = Vec::new();
        loop {
            self.whitespace();
            if self.cursor >= self.text.len() {
                break;
            }
            // Text outside entries is a standard BibTeX comment.
            let Some(found) = self.text[self.cursor..].iter().position(|c| *c == '@') else {
                break;
            };
            self.cursor += found + 1;
            let kind = self.word()?.to_lowercase();
            self.whitespace();
            let opening = self.peek();
            let close = match opening {
                Some('{') => '}',
                Some('(') => ')',
                _ => return Err(self.error("Expected an entry delimiter".into())),
            };
            if kind == "comment" {
                if opening == Some('{') {
                    self.enclosed('{')?;
                } else {
                    match self.text[self.cursor + 1..].iter().position(|c| *c == ')') {
                        Some(offset) => self.cursor += offset + 2,
                        None => return Err(self.error("Unclosed comment".into())),
                    }
                }
                continue;
            }
            self.cursor += 1;
            if kind == "preamble" {
                self.value()?;
                self.expect(close)?;
                continue;
            }
            let key = self.word()?;
            if kind == "string" {
                self.expect('=')?;
                let value = self.value()?;
                self.strings.insert(key.to_lowercase(), value);
                self.whitespace();
                if self.peek() == Some(',') {
                    self.cursor += 1;
                }
                self.expect(close)?;
                continue;
            }
            self.expect(',')?;
            let mut fields = HashMap::new();
            loop {
                self.whitespace();
                if self.peek() == Some(close) {
                    self.cursor += 1;
                    break;
                }
                let field = self.word()?.to_lowercase();
                self.expect('=')?;
                if fields.contains_key(&field) {
                    return Err(self.error(format!("Duplicate BibTeX field {}", repr(&field))));
                }
                let value = self.value()?;
                fields.insert(field, value);
                self.whitespace();
                if self.peek() == Some(close) {
                    self.cursor += 1;
                    break;
                }
                self.expect(',')?;
            }
            if entries.iter().any(|(k, _, _)| *k == key) {
                return Err(self.error(format!("Duplicate BibTeX key {}", repr(&key))));
            }
            entries.push((key, kind, fields));
        }
        Ok(entries)
    }
}

static AUTHOR_SPLIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)[{}]|\s+and\s+").unwrap());

fn name(family: String, given: String) -> Value {
    let mut author = Map::new();
    author.insert("family".into(), Value::String(family));
    author.insert("given".into(), Value::String(given));
    Value::Object(author)
}

fn authors(value: &str) -> Result<Vec<Value>> {
    // Split "and"/commas outside braces so organizations remain literal names.
    let mut names = Vec::new();
    let (mut start, mut depth) = (0, 0i64);
    for found in AUTHOR_SPLIT.find_iter(value) {
        match found.as_str() {
            "{" => depth += 1,
            "}" => depth -= 1,
            _ if depth == 0 => {
                names.push(python_strip(&value[start..found.start()]));
                start = found.end();
            }
            _ => {}
        }
    }
    names.push(python_strip(&value[start..]));
    let mut result = Vec::new();
    for name_text in names.into_iter().filter(|n| !n.is_empty()) {
        if name_text.starts_with('{') && name_text.ends_with('}') {
            let mut literal = Map::new();
            literal.insert("literal".into(), Value::String(tex_text(name_text)?));
            result.push(Value::Object(literal));
        } else if name_text.contains(',') {
            let parts = name_text
                .split(',')
                .map(|part| tex_text(python_strip(part)))
                .collect::<Result<Vec<_>>>()?;
            match parts.as_slice() {
                [family, given] => result.push(name(family.clone(), given.clone())),
                [last, jr, given] => result.push(name(format!("{last}, {jr}"), given.clone())),
                _ => bail!("Unsupported BibTeX author name: {}", repr(name_text)),
            }
        } else {
            let text = tex_text(name_text)?;
            let parts: Vec<&str> = text.split(is_space).filter(|p| !p.is_empty()).collect();
            if parts.is_empty() {
                result.push(name(String::new(), String::new()));
                continue;
            }
            let split = parts[..parts.len() - 1]
                .iter()
                .position(|part| part.chars().next().is_some_and(char::is_lowercase))
                .unwrap_or(parts.len() - 1);
            result.push(name(parts[split..].join(" "), parts[..split].join(" ")));
        }
    }
    Ok(result)
}

static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A[0-9]{1,4}\z").unwrap());

pub fn read_bibtex(text: &str, source: &str) -> Result<Vec<(String, Item)>> {
    let entries = BibParser::new(text, source).parse()?;
    let lookup: HashMap<&str, &HashMap<String, String>> =
        entries.iter().map(|(k, _, f)| (k.as_str(), f)).collect();
    fn resolve(
        key: &str,
        chain: &mut Vec<String>,
        lookup: &HashMap<&str, &HashMap<String, String>>,
        source: &str,
    ) -> Result<HashMap<String, String>> {
        if chain.iter().any(|k| k == key) || !lookup.contains_key(key) {
            bail!("{source}: missing or cyclic crossref {}", repr(key));
        }
        let own = lookup[key];
        match own.get("crossref").filter(|c| !c.is_empty()) {
            Some(parent) => {
                chain.push(key.to_string());
                let mut merged = resolve(parent, chain, lookup, source)?;
                chain.pop();
                merged.extend(own.iter().map(|(k, v)| (k.clone(), v.clone())));
                Ok(merged)
            }
            None => Ok(own.clone()),
        }
    }
    let mut items = Vec::new();
    for (key, kind, _) in &entries {
        let fields = resolve(key, &mut Vec::new(), &lookup, source)?;
        let present = |name: &str| fields.get(name).filter(|v| !v.is_empty());
        let Some(title) = present("title") else {
            bail!("{source}: @{key} is missing title");
        };
        let mut item = Map::new();
        item.insert("id".into(), Value::String(key.clone()));
        item.insert("citekey".into(), Value::String(key.clone()));
        item.insert("title".into(), Value::String(tex_text(title)?));
        item.insert("type".into(), Value::String(kind.clone()));
        if let Some(author) = present("author") {
            item.insert("author".into(), Value::Array(authors(author)?));
        }
        if let Some(year) = present("year") {
            let year = python_strip(year);
            if !YEAR.is_match(year) {
                bail!("{source}: @{key} year must be numeric; export CSL JSON instead");
            }
            let year: i64 = year.parse()?;
            item.insert("issued".into(), serde_json::json!({"date-parts": [[year]]}));
        }
        for (original, target) in [
            ("journal", "container-title"),
            ("booktitle", "container-title"),
            ("doi", "DOI"),
            ("url", "URL"),
            ("volume", "volume"),
            ("number", "issue"),
            ("pages", "page"),
            ("publisher", "publisher"),
        ] {
            if let Some(value) = present(original) {
                let value = if ["DOI", "URL"].contains(&target) {
                    python_strip(value).to_string()
                } else {
                    tex_text(value)?
                };
                item.insert(target.into(), Value::String(value));
            }
        }
        items.push(Value::Object(item));
    }
    validate_snapshot(&serde_json::json!({"version": 1, "items": items}))
}

fn article_link(title: &str, source: &str, key: &str) -> String {
    // Zola owns routing, including Unicode transliteration, slug/path overrides,
    // parent sections and deployment base URLs. Never guess its slug algorithm.
    let json = |value: &str| serde_json::to_string(value).unwrap();
    format!(
        "{{{{ <reference_link path={} anchor={} label={} /> }}}}",
        json(source),
        json(&anchor(key)),
        json(&escape_html(title))
    )
}

/// Python `str.casefold` for sorting titles.
fn casefold(value: &str) -> String {
    value.to_lowercase().replace('ß', "ss")
}

/// Python's display of a TOML scalar used as a title.
fn python_str(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => s.clone(),
        toml::Value::Boolean(true) => "True".into(),
        toml::Value::Boolean(false) => "False".into(),
        other => other.to_string(),
    }
}

/// Generated files and raw bibliographies excluded from staging.
pub struct ReferencePreparation {
    pub overrides: BTreeMap<RelPath, Vec<u8>>,
    pub excluded: HashSet<RelPath>,
}

/// Inputs to citation formatting, read once per sync.
pub struct SourceFile<'a> {
    pub relative: &'a RelPath,
    pub text: &'a str,
}

pub fn read_utf8_sig(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
    String::from_utf8(bytes.to_vec()).map_err(|_| anyhow!("{} is not valid UTF-8", path.display()))
}

/// Return generated bytes and excluded raw bibliographies for staging.
///
/// `files` lists every staged input in scan order; `texts` are the Markdown
/// sources under `content/` in the same order.
pub fn prepare_references(
    source: &Path,
    files: &[RelPath],
    texts: &[SourceFile],
) -> Result<ReferencePreparation> {
    let snapshot_path = RelPath::new(SNAPSHOT);
    let index_page = RelPath::new(INDEX_PAGE);
    let bibliographies: HashSet<RelPath> = files
        .iter()
        .filter(|p| p.first() == "content" && p.suffix_lower() == ".bib")
        .cloned()
        .collect();
    let metadata: HashMap<&RelPath, Metadata> = texts
        .iter()
        .map(|t| Ok((t.relative, parse_frontmatter(t.text)?.0)))
        .collect::<Result<_>>()?;
    let has_snapshot = files.contains(&snapshot_path);
    let mut snapshot = Vec::new();
    if has_snapshot {
        let text = read_utf8_sig(&snapshot_path.under(source))?;
        snapshot = validate_snapshot(&serde_json::from_str(&text)?)?;
    }
    let mut parsed_bibs: HashMap<RelPath, Vec<(String, Item)>> = HashMap::new();
    for path in bibliographies
        .iter()
        .filter(|p| p.name().to_lowercase() == "refs.bib")
    {
        let text = read_utf8_sig(&path.under(source))?;
        parsed_bibs.insert(path.clone(), read_bibtex(&text, &path.native())?);
    }
    let mut overrides = BTreeMap::new();
    let mut published: Vec<(String, Item)> = Vec::new();
    let mut backlinks: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut active = has_snapshot || !bibliographies.is_empty();
    for source_file in texts {
        let relative = source_file.relative;
        let meta = &metadata[relative];
        if *relative == index_page && reference_index(meta) {
            continue;
        }
        let mut items: Items = HashMap::new();
        if let Some(local) = parsed_bibs.get(&relative.parent().join("refs.bib")) {
            items.extend(local.iter().cloned());
        }
        items.extend(snapshot.iter().cloned());
        let (transformed, keys) =
            transform_citations(source_file.text, &items, &relative.native())?;
        if keys.is_empty() {
            continue;
        }
        active = true;
        overrides.insert(relative.clone(), transformed.into_bytes());
        let hidden = |m: &Metadata| {
            truthy(m.get("draft")) || m.get("render") == Some(&toml::Value::Boolean(false))
        };
        let ancestor_hidden = metadata.iter().any(|(path, m)| {
            path.name() == "_index.md" && relative.has_ancestor(&path.parent()) && hidden(m)
        });
        if hidden(meta) || ancestor_hidden {
            continue;
        }
        let title = meta
            .get("title")
            .map(python_str)
            .unwrap_or_else(|| relative.stem());
        let link_source = format!("@/{}", relative.strip_first());
        for key in keys {
            let item = &items[&key];
            // Article-local keys can refer to different papers. Do not merge an
            // ambiguous key silently on the global bibliography page.
            match published.iter().find(|(k, _)| *k == key) {
                Some((_, existing)) if existing != item => {
                    bail!(
                        "{}: conflicting public reference @{key}; use distinct citekeys",
                        relative.native()
                    )
                }
                Some(_) => {}
                None => published.push((key.clone(), item.clone())),
            }
            backlinks
                .entry(key)
                .or_default()
                .push((title.clone(), link_source.clone()));
        }
    }
    if active {
        if let Some(meta) = metadata.get(&index_page) {
            if !reference_index(meta) {
                bail!("content/references.md conflicts with the generated bibliography; move it or mark extra.reference_index = true");
            }
        }
        let mut sorted: Vec<&(String, Item)> = published.iter().collect();
        sorted.sort_by_key(|(key, item)| {
            (
                casefold(item.get("title").and_then(Value::as_str).unwrap_or("")),
                key.clone(),
            )
        });
        let rows: Vec<String> = sorted
            .iter()
            .map(|(key, item)| {
                let related: Vec<String> = backlinks[key]
                    .iter()
                    .map(|(title, source)| article_link(title, source, key))
                    .collect();
                format!(
                    "<li id=\"{}\">{}<div class=\"reference-articles\">引用文章：{}</div></li>",
                    anchor(key),
                    format_item(item),
                    related.join(" · ")
                )
            })
            .collect();
        let mut content = String::from("+++\ntitle = \"文献\"\ndescription = \"已发表文章引用的文献与相关笔记。\"\npath = \"references\"\ntemplate = \"page.html\"\n[extra]\nreference_index = true\n+++\n\n");
        if rows.is_empty() {
            content.push_str(
                "<p class=\"reference-empty\">已发表文章中的文献引用将出现在这里。</p>\n",
            );
        } else {
            content.push_str(&format!(
                "<ol class=\"reference-library\">\n{}\n</ol>\n",
                rows.join("\n")
            ));
        }
        overrides.insert(index_page, content.into_bytes());
        // Even a template deliberately exposing this data sees only references
        // cited by published pages, never uncited or draft-only library entries.
        let items: Vec<Value> = published
            .into_iter()
            .map(|(_, item)| Value::Object(item))
            .collect();
        let data = crate::json::python_dumps(&serde_json::json!({"version": 1, "items": items}));
        overrides.insert(snapshot_path, data.into_bytes());
    }
    Ok(ReferencePreparation {
        overrides,
        excluded: bibliographies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Items {
        let item: Item = serde_json::from_str(r#"{"id":"a","citekey":"a","title":"T","author":[{"family":"F","given":"G"}],"issued":{"date-parts":[[2020]]}}"#).unwrap();
        HashMap::from([("a".to_string(), item)])
    }

    #[test]
    fn numbers_citations_and_appends_list() {
        let (text, keys) =
            transform_citations("See [@a, p. 2] and [@a].\n", &items(), "x").unwrap();
        assert_eq!(keys, ["a"]);
        assert!(text.contains(">1, p. 2</a>"));
        assert!(text.contains("参考文献</h2>"));
        assert!(text.contains("G F. 2020. <span class=\"reference-title\">T</span>."));
    }

    #[test]
    fn missing_key_reports_line() {
        let error = transform_citations("x\n[@b]", &items(), "post.md")
            .unwrap_err()
            .to_string();
        assert!(error.starts_with("post.md:2: missing reference @b"));
    }

    #[test]
    fn citations_in_code_and_links_are_literal() {
        let text = "`[@b]` [@b](x) [@b][y]\n```\n[@b]\n```\n";
        assert_eq!(transform_citations(text, &items(), "x").unwrap().0, text);
    }

    #[test]
    fn bibtex_names_accents_and_crossref() {
        let bib = "@proceedings{p, title={Proc}, booktitle={Proc}, year=2001}\n@inproceedings{k, title={G{\\\"o}del}, author={van Beethoven, Ludwig and {ACME Corp}}, crossref={p}}";
        let items = read_bibtex(bib, "refs.bib").unwrap();
        let k = &items.iter().find(|(key, _)| key == "k").unwrap().1;
        assert_eq!(k["title"], "Gödel");
        assert_eq!(k["container-title"], "Proc");
        assert_eq!(k["author"][0]["family"], "van Beethoven");
        assert_eq!(k["author"][1]["literal"], "ACME Corp");
    }

    #[test]
    fn urls_and_dois() {
        assert_eq!(doi_url("doi: 10.1000/x y").as_deref(), None);
        assert_eq!(
            doi_url("https://doi.org/10.1000/a<b").as_deref(),
            Some("https://doi.org/10.1000/a%3Cb")
        );
        assert!(public_url("https://example.org/a").is_some());
        assert!(public_url("http://localhost/a").is_none());
        assert!(public_url("https://u:p@example.org/").is_none());
        assert!(public_url("file:///etc").is_none());
    }
}
