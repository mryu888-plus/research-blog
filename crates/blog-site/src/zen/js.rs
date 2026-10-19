//! JSON values with JavaScript semantics: property order, number formatting,
//! `JSON.stringify` layout and the string conversions template literals use.

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::cmp::Ordering;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Js {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Js>),
    /// Properties in creation order; see [`Js::entries`] for enumeration order.
    Obj(Vec<(String, Js)>),
}

impl Js {
    pub fn obj() -> Js {
        Js::Obj(Vec::new())
    }

    /// Property access on an object; `undefined` (`None`) for anything else.
    pub fn get(&self, key: &str) -> Option<&Js> {
        match self {
            Js::Obj(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Assignment: an existing property keeps its position.
    pub fn set(&mut self, key: &str, value: Js) {
        if let Js::Obj(entries) = self {
            match entries.iter_mut().find(|(k, _)| k == key) {
                Some(entry) => entry.1 = value,
                None => entries.push((key.to_string(), value)),
            }
        }
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Js> {
        match self {
            Js::Obj(entries) => entries.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// `Object.entries` order: array-index keys ascending, then creation order.
    pub fn entries(&self) -> Vec<(&String, &Js)> {
        let Js::Obj(entries) = self else {
            return Vec::new();
        };
        let mut indexed: Vec<(u32, &String, &Js)> = entries
            .iter()
            .filter_map(|(k, v)| array_index(k).map(|i| (i, k, v)))
            .collect();
        indexed.sort_by_key(|(i, _, _)| *i);
        indexed
            .into_iter()
            .map(|(_, k, v)| (k, v))
            .chain(
                entries
                    .iter()
                    .filter(|(k, _)| array_index(k).is_none())
                    .map(|(k, v)| (k, v)),
            )
            .collect()
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Js::Str(s) => Some(s),
            _ => None,
        }
    }
}

fn array_index(key: &str) -> Option<u32> {
    if key.is_empty()
        || (key.len() > 1 && key.starts_with('0'))
        || !key.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    key.parse::<u32>().ok().filter(|i| *i != u32::MAX)
}

/// JavaScript truthiness of a possibly undefined value.
pub fn truthy(value: Option<&Js>) -> bool {
    match value {
        None | Some(Js::Null) => false,
        Some(Js::Bool(b)) => *b,
        Some(Js::Num(n)) => *n != 0.0 && !n.is_nan(),
        Some(Js::Str(s)) => !s.is_empty(),
        Some(Js::Arr(_) | Js::Obj(_)) => true,
    }
}

/// ECMAScript WhiteSpace and LineTerminator, as `String.prototype.trim` and `\s` use them.
pub fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\x0b' | '\x0c' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// The same set as a regex character class.
pub const JS_SPACE_CLASS: &str = r"[\t\n\x0B\x0C\r \xA0\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}]";

pub fn trim(value: &str) -> &str {
    value.trim_matches(is_js_space)
}

/// `Number.prototype.toString()`.
pub fn number(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value == 0.0 {
        return "0".into();
    }
    if value.is_infinite() {
        return if value > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    if value < 0.0 {
        return format!("-{}", number(-value));
    }
    // Rust's `{:e}` prints the shortest round-trip digits, as ECMAScript requires.
    let scientific = format!("{value:e}");
    let (mantissa, exponent) = scientific.split_once('e').unwrap();
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i64;
    let n = exponent.parse::<i64>().unwrap() + 1;
    if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let sign = if e >= 0 { '+' } else { '-' };
        if k == 1 {
            format!("{digits}e{sign}{}", e.abs())
        } else {
            format!("{}.{}e{sign}{}", &digits[..1], &digits[1..], e.abs())
        }
    }
}

/// `String(value)` as a template literal interpolates it.
pub fn to_string(value: Option<&Js>) -> String {
    match value {
        None => "undefined".into(),
        Some(Js::Null) => "null".into(),
        Some(Js::Bool(b)) => b.to_string(),
        Some(Js::Num(n)) => number(*n),
        Some(Js::Str(s)) => s.clone(),
        Some(Js::Arr(items)) => items
            .iter()
            .map(|item| match item {
                Js::Null => String::new(),
                other => to_string(Some(other)),
            })
            .collect::<Vec<_>>()
            .join(","),
        Some(Js::Obj(_)) => "[object Object]".into(),
    }
}

/// Order strings as JavaScript's default `sort` does (UTF-16 code units).
pub fn utf16_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

pub fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `JSON.stringify(value, null, indent)`; `indent` 0 is the compact form.
pub fn stringify(value: &Js, indent: usize) -> String {
    let mut out = String::new();
    write(value, &" ".repeat(indent), "", &mut out);
    out
}

fn write(value: &Js, gap: &str, current: &str, out: &mut String) {
    match value {
        Js::Null => out.push_str("null"),
        Js::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Js::Num(n) if n.is_finite() => out.push_str(&number(*n)),
        Js::Num(_) => out.push_str("null"),
        Js::Str(s) => out.push_str(&quote(s)),
        Js::Arr(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            let inner = format!("{current}{gap}");
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                if !gap.is_empty() {
                    out.push('\n');
                    out.push_str(&inner);
                }
                write(item, gap, &inner, out);
            }
            if !gap.is_empty() {
                out.push('\n');
                out.push_str(current);
            }
            out.push(']');
        }
        Js::Obj(_) => {
            let entries = value.entries();
            if entries.is_empty() {
                out.push_str("{}");
                return;
            }
            let inner = format!("{current}{gap}");
            out.push('{');
            for (index, (key, item)) in entries.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                if !gap.is_empty() {
                    out.push('\n');
                    out.push_str(&inner);
                }
                out.push_str(&quote(key));
                out.push_str(if gap.is_empty() { ":" } else { ": " });
                write(item, gap, &inner, out);
            }
            if !gap.is_empty() {
                out.push('\n');
                out.push_str(current);
            }
            out.push('}');
        }
    }
}

pub fn parse(text: &str) -> anyhow::Result<Js> {
    Ok(serde_json::from_str(text)?)
}

impl<'de> Deserialize<'de> for Js {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(JsVisitor)
    }
}

struct JsVisitor;

impl<'de> Visitor<'de> for JsVisitor {
    type Value = Js;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Js, E> {
        Ok(Js::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Js, E> {
        Ok(Js::Num(value as f64))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Js, E> {
        Ok(Js::Num(value as f64))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Js, E> {
        Ok(Js::Num(value))
    }

    fn visit_str<E>(self, value: &str) -> Result<Js, E> {
        Ok(Js::Str(value.to_string()))
    }

    fn visit_string<E>(self, value: String) -> Result<Js, E> {
        Ok(Js::Str(value))
    }

    fn visit_unit<E>(self) -> Result<Js, E> {
        Ok(Js::Null)
    }

    fn visit_none<E>(self) -> Result<Js, E> {
        Ok(Js::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Js, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Js::Arr(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Js, A::Error> {
        // JSON.parse keeps the first position of a duplicate key and the last value.
        let mut object = Js::obj();
        while let Some((key, value)) = map.next_entry::<String, Js>()? {
            object.set(&key, value);
        }
        Ok(object)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_numbers_like_javascript() {
        let cases = [
            (1.95, "1.95"),
            (2.0, "2"),
            (700.0, "700"),
            (0.5, "0.5"),
            (0.000001, "0.000001"),
            (0.0000001, "1e-7"),
            (1e21, "1e+21"),
            (1.5e22, "1.5e+22"),
            (123456789012345680000.0, "123456789012345680000"),
            (-0.0, "0"),
            (-3.25, "-3.25"),
            (0.1 + 0.2, "0.30000000000000004"),
        ];
        for (value, expected) in cases {
            assert_eq!(number(value), expected, "{value}");
        }
    }

    #[test]
    fn stringify_matches_json_stringify_layout() {
        let value =
            parse(r#"{"b": [1, {"x": "é\n\u0001"}], "a": {}, "2": true, "10": [], "01": null}"#)
                .unwrap();
        assert_eq!(
            stringify(&value, 0),
            r#"{"2":true,"10":[],"b":[1,{"x":"é\n\u0001"}],"a":{},"01":null}"#
        );
        assert_eq!(
            stringify(&parse("[1,[2]]").unwrap(), 2),
            "[\n  1,\n  [\n    2\n  ]\n]"
        );
    }

    #[test]
    fn duplicate_keys_keep_first_position_and_last_value() {
        let value = parse(r#"{"a": 1, "b": 2, "a": 3}"#).unwrap();
        assert_eq!(stringify(&value, 0), r#"{"a":3,"b":2}"#);
    }
}
