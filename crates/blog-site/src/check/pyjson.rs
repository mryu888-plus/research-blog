//! `json.loads` as CPython's C scanner does it, so a broken generated file
//! yields the same message, line, column and character offset.

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number,
    Str(String),
    Array(Vec<Json>),
    /// Pairs in document order; a repeated key resolves to its last value.
    Object(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(pairs) => pairs.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct DecodeError(String);

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DecodeError {}

struct Decoder {
    s: Vec<char>,
}

/// Stop scanning: a JSONDecodeError message and offset, or "no value here".
enum Stop {
    Error(&'static str, usize),
    NoValue(usize),
}

type Scan<T> = Result<(T, usize), Stop>;

fn is_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

impl Decoder {
    fn at(&self, i: usize) -> Option<char> {
        self.s.get(i).copied()
    }

    fn skip_ws(&self, mut i: usize) -> usize {
        while self.at(i).is_some_and(is_ws) {
            i += 1;
        }
        i
    }

    fn starts(&self, i: usize, word: &str) -> bool {
        word.chars()
            .enumerate()
            .all(|(k, c)| self.at(i + k) == Some(c))
    }

    fn hex4(&self, from: usize) -> Option<u32> {
        let mut value = 0;
        for k in 0..4 {
            value = value * 16 + self.at(from + k)?.to_digit(16)?;
        }
        Some(value)
    }

    fn scan_string(&self, end: usize) -> Scan<String> {
        let begin = end - 1;
        let len = self.s.len();
        let mut end = end;
        let mut out = String::new();
        loop {
            let mut next = end;
            let mut c = '\0';
            while next < len {
                c = self.s[next];
                if c == '"' || c == '\\' {
                    break;
                }
                if (c as u32) <= 0x1f {
                    return Err(Stop::Error("Invalid control character at", next));
                }
                next += 1;
            }
            if next >= len || !(c == '"' || c == '\\') {
                return Err(Stop::Error("Unterminated string starting at", begin));
            }
            out.extend(&self.s[end..next]);
            next += 1;
            if c == '"' {
                return Ok((out, next));
            }
            if next == len {
                return Err(Stop::Error("Unterminated string starting at", begin));
            }
            let escape = self.s[next];
            if escape != 'u' {
                end = next + 1;
                let decoded = match escape {
                    '"' => '"',
                    '\\' => '\\',
                    '/' => '/',
                    'b' => '\u{8}',
                    'f' => '\u{c}',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    _ => return Err(Stop::Error("Invalid \\escape", end - 2)),
                };
                out.push(decoded);
                continue;
            }
            next += 1;
            end = next + 4;
            if end >= len {
                return Err(Stop::Error("Invalid \\uXXXX escape", next - 1));
            }
            let Some(mut code) = self.hex4(next) else {
                return Err(Stop::Error("Invalid \\uXXXX escape", end - 5));
            };
            if (0xd800..=0xdbff).contains(&code)
                && end + 6 < len
                && self.s[end] == '\\'
                && self.s[end + 1] == 'u'
            {
                let Some(low) = self.hex4(end + 2) else {
                    return Err(Stop::Error("Invalid \\uXXXX escape", end + 1));
                };
                if (0xdc00..=0xdfff).contains(&low) {
                    code = 0x10000 + (((code - 0xd800) << 10) | (low - 0xdc00));
                    end += 6;
                }
            }
            // Lone surrogates have no Rust `char`; they never appear in our checks.
            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
        }
    }

    fn scan_number(&self, i: usize) -> Option<usize> {
        let mut p = i;
        if self.at(p) == Some('-') {
            p += 1;
        }
        match self.at(p) {
            Some('0') => p += 1,
            Some('1'..='9') => {
                while self.at(p).is_some_and(|c| c.is_ascii_digit()) {
                    p += 1;
                }
            }
            _ => return None,
        }
        if self.at(p) == Some('.') && self.at(p + 1).is_some_and(|c| c.is_ascii_digit()) {
            p += 1;
            while self.at(p).is_some_and(|c| c.is_ascii_digit()) {
                p += 1;
            }
        }
        if matches!(self.at(p), Some('e' | 'E')) {
            let mut q = p + 1;
            if matches!(self.at(q), Some('+' | '-')) {
                q += 1;
            }
            if self.at(q).is_some_and(|c| c.is_ascii_digit()) {
                while self.at(q).is_some_and(|c| c.is_ascii_digit()) {
                    q += 1;
                }
                p = q;
            }
        }
        Some(p)
    }

    fn scan_once(&self, i: usize) -> Scan<Json> {
        let Some(c) = self.at(i) else {
            return Err(Stop::NoValue(i));
        };
        match c {
            '"' => self.scan_string(i + 1).map(|(s, e)| (Json::Str(s), e)),
            '{' => self.scan_object(i + 1),
            '[' => self.scan_array(i + 1),
            'n' if self.starts(i, "null") => Ok((Json::Null, i + 4)),
            't' if self.starts(i, "true") => Ok((Json::Bool(true), i + 4)),
            'f' if self.starts(i, "false") => Ok((Json::Bool(false), i + 5)),
            'N' if self.starts(i, "NaN") => Ok((Json::Number, i + 3)),
            'I' if self.starts(i, "Infinity") => Ok((Json::Number, i + 8)),
            '-' if self.starts(i, "-Infinity") => Ok((Json::Number, i + 9)),
            _ => match self.scan_number(i) {
                Some(end) => Ok((Json::Number, end)),
                None => Err(Stop::NoValue(i)),
            },
        }
    }

    fn value(&self, i: usize) -> Scan<Json> {
        self.scan_once(i).map_err(|stop| match stop {
            Stop::NoValue(at) => Stop::Error("Expecting value", at),
            other => other,
        })
    }

    fn scan_object(&self, start: usize) -> Scan<Json> {
        let mut pairs = Vec::new();
        let mut i = self.skip_ws(start);
        if self.at(i) == Some('}') {
            return Ok((Json::Object(pairs), i + 1));
        }
        if self.at(i) != Some('"') {
            return Err(Stop::Error(
                "Expecting property name enclosed in double quotes",
                i,
            ));
        }
        loop {
            let (key, after) = self.scan_string(i + 1)?;
            i = self.skip_ws(after);
            if self.at(i) != Some(':') {
                return Err(Stop::Error("Expecting ':' delimiter", i));
            }
            i = self.skip_ws(i + 1);
            let (value, after) = self.value(i)?;
            pairs.push((key, value));
            i = self.skip_ws(after);
            match self.at(i) {
                Some('}') => return Ok((Json::Object(pairs), i + 1)),
                Some(',') => {}
                _ => return Err(Stop::Error("Expecting ',' delimiter", i)),
            }
            let comma = i;
            i = self.skip_ws(i + 1);
            match self.at(i) {
                Some('"') => {}
                Some('}') => {
                    return Err(Stop::Error(
                        "Illegal trailing comma before end of object",
                        comma,
                    ))
                }
                _ => {
                    return Err(Stop::Error(
                        "Expecting property name enclosed in double quotes",
                        i,
                    ))
                }
            }
        }
    }

    fn scan_array(&self, start: usize) -> Scan<Json> {
        let mut values = Vec::new();
        let mut i = self.skip_ws(start);
        if self.at(i) == Some(']') {
            return Ok((Json::Array(values), i + 1));
        }
        loop {
            let (value, after) = self.value(i)?;
            values.push(value);
            i = self.skip_ws(after);
            match self.at(i) {
                Some(']') => return Ok((Json::Array(values), i + 1)),
                Some(',') => {}
                _ => return Err(Stop::Error("Expecting ',' delimiter", i)),
            }
            let comma = i;
            i = self.skip_ws(i + 1);
            if self.at(i) == Some(']') {
                return Err(Stop::Error(
                    "Illegal trailing comma before end of array",
                    comma,
                ));
            }
        }
    }

    fn error(&self, message: &str, pos: usize) -> DecodeError {
        let before = &self.s[..pos.min(self.s.len())];
        let line = before.iter().filter(|c| **c == '\n').count() + 1;
        let column = match before.iter().rposition(|c| *c == '\n') {
            Some(newline) => pos - newline,
            None => pos + 1,
        };
        DecodeError(format!(
            "{message}: line {line} column {column} (char {pos})"
        ))
    }
}

/// Python `json.loads` on text already decoded with `utf-8-sig`.
pub fn loads(text: &str) -> Result<Json, DecodeError> {
    let decoder = Decoder {
        s: text.chars().collect(),
    };
    if text.starts_with('\u{feff}') {
        return Err(decoder.error("Unexpected UTF-8 BOM (decode using utf-8-sig)", 0));
    }
    let start = decoder.skip_ws(0);
    let (value, end) = decoder.value(start).map_err(|stop| match stop {
        Stop::Error(message, pos) => decoder.error(message, pos),
        Stop::NoValue(pos) => decoder.error("Expecting value", pos),
    })?;
    let end = decoder.skip_ws(end);
    if end != decoder.s.len() {
        return Err(decoder.error("Extra data", end));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_match_python() {
        let message = |text: &str| loads(text).unwrap_err().to_string();
        assert_eq!(message(""), "Expecting value: line 1 column 1 (char 0)");
        assert_eq!(
            message("[1,]"),
            "Illegal trailing comma before end of array: line 1 column 3 (char 2)"
        );
        assert_eq!(
            message("{\"a\" 1}"),
            "Expecting ':' delimiter: line 1 column 6 (char 5)"
        );
        assert_eq!(message("[]\n x"), "Extra data: line 2 column 2 (char 4)");
    }

    #[test]
    fn last_duplicate_key_wins() {
        let value = loads("{\"a\": 1, \"a\": false}").unwrap();
        assert_eq!(value.get("a"), Some(&Json::Bool(false)));
    }
}
