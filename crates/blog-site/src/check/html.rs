//! The start-tag side of Python 3.13's `html.parser.HTMLParser`, fed once and
//! never closed, plus `html.unescape`. Only `href`, `src` and `id` matter to
//! the site check, but tokenizing must follow Python exactly: where Python
//! stops at an unfinished construct, every later tag is ignored too.

use super::entities::{HTML5, INVALID_CHARREFS, INVALID_CODEPOINTS};
use crate::markdown::is_space;
use std::collections::HashSet;

/// Links and anchors collected from one page.
#[derive(Debug, Default, PartialEq)]
pub struct Page {
    /// `href` then `src` of every start tag, when non-empty.
    pub links: Vec<String>,
    /// `id` values; an `id` attribute without a value is never a target.
    pub ids: HashSet<String>,
}

const WS: &[u8] = b"\t\n\r\x0c ";

fn in_set(byte: u8, set: &[u8]) -> bool {
    set.contains(&byte)
}

fn entity(name: &str) -> Option<&'static str> {
    HTML5
        .binary_search_by(|(key, _)| key.as_bytes().cmp(name.as_bytes()))
        .ok()
        .map(|i| HTML5[i].1)
}

/// Python's `str.splitlines` newline handling for text-mode reads: `\r\n` and
/// a lone `\r` become `\n`.
pub fn universal_newlines(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_string();
    }
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn numeric_reference(digits: &str, radix: u32) -> String {
    let mut value: u64 = 0;
    for c in digits.chars() {
        value = value
            .saturating_mul(radix as u64)
            .saturating_add(c.to_digit(radix).unwrap() as u64);
    }
    if let Ok(i) = INVALID_CHARREFS.binary_search_by_key(&value, |(k, _)| *k as u64) {
        return INVALID_CHARREFS[i].1.to_string();
    }
    if (0xD800..=0xDFFF).contains(&value) || value > 0x10FFFF {
        return '\u{fffd}'.to_string();
    }
    if INVALID_CODEPOINTS.binary_search(&(value as u32)).is_ok() {
        return String::new();
    }
    char::from_u32(value as u32)
        .map(String::from)
        .unwrap_or_default()
}

/// Python `html.unescape`.
pub fn unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let bytes = text.as_bytes();
    let mut result = String::with_capacity(text.len());
    let mut cursor = 0;
    while let Some(offset) = text[cursor..].find('&') {
        let amp = cursor + offset;
        result.push_str(&text[cursor..amp]);
        let start = amp + 1;
        // &(#[0-9]+;?|#[xX][0-9a-fA-F]+;?|[^\t\n\f <&#;]{1,32};?)
        if bytes.get(start) == Some(&b'#') {
            let (hex, digits_start) = match bytes.get(start + 1) {
                Some(b'x' | b'X') => (true, start + 2),
                _ => (false, start + 1),
            };
            let digit = |b: &u8| {
                if hex {
                    b.is_ascii_hexdigit()
                } else {
                    b.is_ascii_digit()
                }
            };
            let mut end = digits_start;
            while end < bytes.len() && digit(&bytes[end]) {
                end += 1;
            }
            if end > digits_start {
                let replacement =
                    numeric_reference(&text[digits_start..end], if hex { 16 } else { 10 });
                if bytes.get(end) == Some(&b';') {
                    end += 1;
                }
                result.push_str(&replacement);
                cursor = end;
                continue;
            }
            result.push('&');
            cursor = start;
            continue;
        }
        let mut end = start;
        let mut count = 0;
        for c in text[start..].chars() {
            if count == 32 || matches!(c, '\t' | '\n' | '\x0c' | ' ' | '<' | '&' | '#' | ';') {
                break;
            }
            end += c.len_utf8();
            count += 1;
        }
        if count == 0 {
            result.push('&');
            cursor = start;
            continue;
        }
        if bytes.get(end) == Some(&b';') {
            end += 1;
        }
        let name = &text[start..end];
        if let Some(value) = entity(name) {
            result.push_str(value);
        } else {
            // The longest matching prefix, at least two characters long.
            let boundaries: Vec<usize> = name.char_indices().map(|(i, _)| i).collect();
            let found = (2..boundaries.len())
                .rev()
                .find_map(|x| entity(&name[..boundaries[x]]).map(|value| (value, boundaries[x])));
            match found {
                Some((value, split)) => {
                    result.push_str(value);
                    result.push_str(&name[split..]);
                }
                None => {
                    result.push('&');
                    result.push_str(name);
                }
            }
        }
        cursor = end;
    }
    result.push_str(&text[cursor..]);
    result
}

/// Character references in attribute values, decoded as Python's parser does.
fn unescape_attribute(value: &str) -> String {
    if !value.contains('&') {
        return value.to_string();
    }
    let bytes = value.as_bytes();
    let mut result = String::with_capacity(value.len());
    let mut cursor = 0;
    while let Some(offset) = value[cursor..].find('&') {
        let amp = cursor + offset;
        result.push_str(&value[cursor..amp]);
        // &(#[0-9]+|#[xX][0-9a-fA-F]+|[a-zA-Z][a-zA-Z0-9]*)[;=]?
        let start = amp + 1;
        let mut end = start;
        let numeric = bytes.get(start) == Some(&b'#');
        if numeric {
            let hex = matches!(bytes.get(start + 1), Some(b'x' | b'X'));
            let digits = if hex { start + 2 } else { start + 1 };
            end = digits;
            while end < bytes.len()
                && (if hex {
                    bytes[end].is_ascii_hexdigit()
                } else {
                    bytes[end].is_ascii_digit()
                })
            {
                end += 1;
            }
            if end == digits {
                end = start;
            }
        } else if bytes.get(start).is_some_and(u8::is_ascii_alphabetic) {
            end = start + 1;
            while end < bytes.len() && bytes[end].is_ascii_alphanumeric() {
                end += 1;
            }
        }
        if end == start {
            result.push('&');
            cursor = start;
            continue;
        }
        if matches!(bytes.get(end), Some(b';' | b'=')) {
            end += 1;
        }
        let reference = &value[amp..end];
        if numeric || (!reference.ends_with('=') && entity(&reference[1..]).is_some()) {
            result.push_str(&unescape(reference));
        } else {
            result.push_str(reference);
        }
        cursor = end;
    }
    result.push_str(&value[cursor..]);
    result
}

struct Parser<'a> {
    raw: &'a [u8],
    text: &'a str,
    /// Lower-case name of the open `script`/`style`/`textarea`/`title` element.
    cdata: Option<String>,
    page: Page,
}

impl Parser<'_> {
    fn n(&self) -> usize {
        self.raw.len()
    }

    fn find(&self, needle: &str, from: usize) -> Option<usize> {
        if from > self.n() {
            return None;
        }
        self.text[from..].find(needle).map(|i| from + i)
    }

    fn find_byte(&self, needle: u8, from: usize) -> Option<usize> {
        self.raw
            .get(from..)?
            .iter()
            .position(|b| *b == needle)
            .map(|i| from + i)
    }

    /// `[\t\n\r\f ]*=[\t\n\r\f ]*` and a value, after an attribute name ending
    /// at `start`. Returns the end and the raw value when present.
    fn value_group(&self, start: usize) -> (usize, Option<(usize, usize)>) {
        let raw = self.raw;
        let n = self.n();
        let mut equals = start;
        while equals < n && in_set(raw[equals], WS) {
            equals += 1;
        }
        if equals >= n || raw[equals] != b'=' {
            return (start, None);
        }
        let mut value = equals + 1;
        while value < n && in_set(raw[value], WS) {
            value += 1;
        }
        let quoted = |quote: u8| self.find_byte(quote, value + 1).map(|close| close + 1);
        let end = match raw.get(value) {
            Some(b'\'') => quoted(b'\''),
            Some(b'"') => quoted(b'"'),
            _ => {
                let mut end = value;
                while end < n && !in_set(raw[end], b">\t\n\r\x0c ") {
                    end += 1;
                }
                Some(end)
            }
        };
        match end {
            Some(end) => (end, Some((value, end))),
            // Giving back one space leaves an empty bare value.
            None if value > equals + 1 => (value - 1, Some((value - 1, value - 1))),
            None => (start, None),
        }
    }

    fn lookbehind_ok(&self, position: usize) -> bool {
        position > 0 && in_set(self.raw[position - 1], b"'\"\t\n\r\x0c /")
    }

    /// The `locatetagend` pattern from a tag name at `position`.
    fn locate_tag_end(&self, position: usize) -> usize {
        let raw = self.raw;
        let n = self.n();
        let mut p = position + 1;
        while p < n && !in_set(raw[p], b"\t\n\r\x0c />") {
            p += 1;
        }
        while p < n && in_set(raw[p], b"\t\n\r\x0c /") {
            p += 1;
        }
        while self.lookbehind_ok(p) && p < n && !in_set(raw[p], b"\t\n\r\x0c />") {
            let mut q = p + 1;
            while q < n && !in_set(raw[q], b"\t\n\r\x0c /=>") {
                q += 1;
            }
            q = self.value_group(q).0;
            while q < n && in_set(raw[q], b"\t\n\r\x0c /") {
                q += 1;
            }
            p = q;
        }
        if p < n && raw[p] == b'>' {
            p += 1;
        }
        p
    }

    fn skip_tag_tail(&self, mut p: usize) -> usize {
        // (?:[\t\n\r\f ]|/(?!>))*
        while p < self.n() {
            if in_set(self.raw[p], WS)
                || (self.raw[p] == b'/' && self.raw.get(p + 1) != Some(&b'>'))
            {
                p += 1;
            } else {
                break;
            }
        }
        p
    }

    fn parse_starttag(&mut self, i: usize) -> Option<usize> {
        let endpos = self.locate_tag_end(i + 1);
        if self.raw[endpos - 1] != b'>' {
            return None;
        }
        let raw = self.raw;
        let n = self.n();
        let mut k = i + 2;
        while k < n && !in_set(raw[k], b"\t\n\r\x0c />") {
            k += 1;
        }
        let tag = self.text[i + 1..k].to_lowercase();
        k = self.skip_tag_tail(k);
        let mut attrs: Vec<(String, Option<String>)> = Vec::new();
        while k < endpos {
            if !self.lookbehind_ok(k) || k >= n || in_set(raw[k], b"\t\n\r\x0c />") {
                break;
            }
            let mut name_end = k + 1;
            while name_end < n && !in_set(raw[name_end], b"\t\n\r\x0c /=>") {
                name_end += 1;
            }
            let (end, value) = self.value_group(name_end);
            let value = value.map(|(start, stop)| {
                let mut value = &self.text[start..stop];
                if value.len() >= 2
                    && ((value.starts_with('\'') && value.ends_with('\''))
                        || (value.starts_with('"') && value.ends_with('"')))
                {
                    value = &value[1..value.len() - 1];
                }
                unescape_attribute(value)
            });
            attrs.push((self.text[k..name_end].to_lowercase(), value));
            k = self.skip_tag_tail(end);
        }
        let end = self.text[k.min(endpos)..endpos].trim_matches(is_space);
        if end != ">" && end != "/>" {
            return Some(endpos);
        }
        self.start_tag(&attrs);
        if !end.ends_with("/>") && matches!(tag.as_str(), "script" | "style" | "textarea" | "title")
        {
            self.cdata = Some(tag);
        }
        Some(endpos)
    }

    fn start_tag(&mut self, attrs: &[(String, Option<String>)]) {
        let last = |name: &str| {
            attrs
                .iter()
                .rev()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value)
        };
        if let Some(Some(id)) = last("id") {
            self.page.ids.insert(id.clone());
        }
        for attribute in ["href", "src"] {
            if let Some(Some(value)) = last(attribute) {
                if !value.is_empty() {
                    self.page.links.push(value.clone());
                }
            }
        }
    }

    fn parse_bogus_comment(&self, i: usize) -> Option<usize> {
        self.find_byte(b'>', i + 2).map(|p| p + 1)
    }

    fn parse_endtag(&mut self, i: usize) -> Option<usize> {
        self.find_byte(b'>', i + 2)?;
        if !self.raw.get(i + 2).is_some_and(u8::is_ascii_alphabetic) {
            if self.raw.get(i + 2) == Some(&b'>') {
                return Some(i + 3);
            }
            return self.parse_bogus_comment(i);
        }
        let j = self.locate_tag_end(i + 2);
        if self.raw[j - 1] != b'>' {
            return None;
        }
        self.cdata = None;
        Some(j)
    }

    fn parse_comment(&self, i: usize) -> Option<usize> {
        // --!?> searched, else -?> matched right after "<!--".
        let mut from = i + 4;
        while let Some(p) = self.find("--", from) {
            if self.raw.get(p + 2) == Some(&b'>') {
                return Some(p + 3);
            }
            if self.raw.get(p + 2) == Some(&b'!') && self.raw.get(p + 3) == Some(&b'>') {
                return Some(p + 4);
            }
            from = p + 1;
        }
        match (self.raw.get(i + 4), self.raw.get(i + 5)) {
            (Some(b'>'), _) => Some(i + 5),
            (Some(b'-'), Some(b'>')) => Some(i + 6),
            _ => None,
        }
    }

    fn parse_marked_section(&self, i: usize) -> Option<usize> {
        // Only "<![CDATA[" reaches here; its name ends before "[".
        let text = &self.text[i + 3..];
        for (p, c) in text.char_indices() {
            if c != ']' {
                continue;
            }
            let rest = &text[p + 1..];
            let rest = rest.trim_start_matches(is_space);
            if let Some(after) = rest.strip_prefix(']') {
                let after = after.trim_start_matches(is_space);
                if after.starts_with('>') {
                    return Some(self.n() - after.len() + 1);
                }
            }
        }
        None
    }

    fn parse_html_declaration(&self, i: usize) -> Option<usize> {
        let head = &self.raw[i..self.n().min(i + 9)];
        if head == b"<![CDATA[" {
            return self.parse_marked_section(i);
        }
        if head.eq_ignore_ascii_case(b"<!doctype") {
            return self.find_byte(b'>', i + 9).map(|p| p + 1);
        }
        self.parse_bogus_comment(i)
    }

    /// The next `</name` followed by a tag-ending character, ignoring ASCII case.
    fn find_cdata_end(&self, name: &str, from: usize) -> Option<usize> {
        let mut search = from;
        while let Some(p) = self.find("</", search) {
            let after = p + 2 + name.len();
            if self
                .raw
                .get(p + 2..after)
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name.as_bytes()))
                && self
                    .raw
                    .get(after)
                    .is_some_and(|b| in_set(*b, b"\t\n\r\x0c />"))
            {
                return Some(p);
            }
            search = p + 1;
        }
        None
    }

    fn run(&mut self) {
        let n = self.n();
        let mut i = 0;
        while i < n {
            let next = match &self.cdata {
                None => self.find_byte(b'<', i),
                Some(name) => self.find_cdata_end(&name.clone(), i),
            };
            let Some(j) = next else { break };
            i = j;
            let raw = self.raw;
            let k = if raw.get(i + 1).is_some_and(u8::is_ascii_alphabetic) {
                self.parse_starttag(i)
            } else if raw[i..].starts_with(b"</") {
                self.parse_endtag(i)
            } else if raw[i..].starts_with(b"<!--") {
                self.parse_comment(i)
            } else if raw[i..].starts_with(b"<?") {
                self.find_byte(b'>', i + 2).map(|p| p + 1)
            } else if raw[i..].starts_with(b"<!") {
                self.parse_html_declaration(i)
            } else if i + 1 < n {
                Some(i + 1)
            } else {
                None
            };
            match k {
                Some(k) => i = k,
                None => break,
            }
        }
    }
}

/// Parse a page the way `check_site.Page` does.
pub fn parse_page(text: &str) -> Page {
    let mut parser = Parser {
        raw: text.as_bytes(),
        text,
        cdata: None,
        page: Page::default(),
    };
    parser.run();
    parser.page
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_links_and_ids() {
        let page =
            parse_page("<A HREF='/x?a=1&amp;b=2' ID=top><img src=\"p.png\"/><a href=\"\">e</a>");
        assert_eq!(page.links, ["/x?a=1&b=2", "p.png"]);
        assert!(page.ids.contains("top"));
    }

    #[test]
    fn script_content_and_comments_hide_tags() {
        let page = parse_page("<script>'<a href=x>'</script ><!-- <a href=y> --><a href=z>");
        assert_eq!(page.links, ["z"]);
    }

    #[test]
    fn unfinished_tag_stops_parsing() {
        assert_eq!(parse_page("<a href=a><a href=\"b><a href=c>").links, ["a"]);
    }

    #[test]
    fn unescape_matches_python() {
        assert_eq!(unescape("&lt;&#x41;&#0;&notit;&amp"), "<A\u{fffd}¬it;&");
        assert_eq!(unescape_attribute("&amp=&ampx&#65="), "&amp=&ampxA=");
    }
}
