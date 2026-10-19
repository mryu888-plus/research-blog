//! JSON values with JavaScript semantics: property order, `JSON.parse`
//! numbers as doubles and `JSON.stringify` layout. The catalog revision is a
//! hash of `JSON.stringify`, so the bytes must match the extension exactly.

use std::fmt::Write;

#[derive(Clone, Debug, PartialEq)]
pub enum JsValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsValue>),
    Object(JsObject),
}

/// Object properties in JavaScript order: array-index keys ascending, then
/// other keys in insertion order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JsObject(Vec<(String, JsValue)>);

fn array_index(key: &str) -> Option<u32> {
    if key == "0" {
        return Some(0);
    }
    if key.is_empty() || key.starts_with('0') || !key.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    key.parse::<u64>()
        .ok()
        .filter(|v| *v < u32::MAX as u64)
        .map(|v| v as u32)
}

impl JsObject {
    pub fn new() -> Self {
        JsObject(Vec::new())
    }

    pub fn get(&self, key: &str) -> Option<&JsValue> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut JsValue> {
        self.0.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn insert(&mut self, key: &str, value: JsValue) {
        if let Some(existing) = self.get_mut(key) {
            *existing = value;
            return;
        }
        match array_index(key) {
            Some(index) => {
                let position = self
                    .0
                    .iter()
                    .position(|(k, _)| array_index(k).is_none_or(|other| other > index))
                    .unwrap_or(self.0.len());
                self.0.insert(position, (key.to_string(), value));
            }
            None => self.0.push((key.to_string(), value)),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &(String, JsValue)> {
        self.0.iter()
    }
}

impl JsValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsValue::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<JsValue>> {
        match self {
            JsValue::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&JsObject> {
        match self {
            JsValue::Object(o) => Some(o),
            _ => None,
        }
    }

    pub fn as_object_mut(&mut self) -> Option<&mut JsObject> {
        match self {
            JsValue::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Property access as `value.key` reads it: `undefined` is `None`.
    pub fn prop(&self, key: &str) -> Option<&JsValue> {
        match self {
            JsValue::Object(o) => o.get(key),
            JsValue::Array(a) if key == "length" => None,
            _ => None,
        }
    }

    /// JavaScript truthiness.
    pub fn truthy(&self) -> bool {
        match self {
            JsValue::Null => false,
            JsValue::Bool(b) => *b,
            JsValue::Number(n) => *n != 0.0 && !n.is_nan(),
            JsValue::String(s) => !s.is_empty(),
            JsValue::Array(_) | JsValue::Object(_) => true,
        }
    }

    /// `String(value)` for the values a request body can hold.
    pub fn to_js_string(&self) -> String {
        match self {
            JsValue::Null => "null".into(),
            JsValue::Bool(b) => b.to_string(),
            JsValue::Number(n) => number_to_string(*n),
            JsValue::String(s) => s.clone(),
            JsValue::Array(items) => items
                .iter()
                .map(|item| match item {
                    JsValue::Null => String::new(),
                    other => other.to_js_string(),
                })
                .collect::<Vec<_>>()
                .join(","),
            JsValue::Object(_) => "[object Object]".into(),
        }
    }
}

/// `Number.prototype.toString()` for finite doubles.
pub fn number_to_string(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value.is_infinite() {
        return if value > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    if value == 0.0 {
        return "0".into();
    }
    let sign = if value < 0.0 { "-" } else { "" };
    // Rust's `{:e}` prints the shortest round-trip digits, like JavaScript.
    let formatted = format!("{:e}", value.abs());
    let (mantissa, exponent) = formatted.split_once('e').unwrap();
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i64;
    let n = exponent.parse::<i64>().unwrap() + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let exponent = n - 1;
        let sign = if exponent >= 0 { "+" } else { "-" };
        let mantissa = if k == 1 {
            digits.clone()
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        format!("{mantissa}e{sign}{}", exponent.abs())
    };
    format!("{sign}{body}")
}

fn quote(value: &str, output: &mut String) {
    output.push('"');
    for c in value.chars() {
        match c {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{8}' => output.push_str("\\b"),
            '\u{c}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c if (c as u32) < 0x20 => write!(output, "\\u{:04x}", c as u32).unwrap(),
            c => output.push(c),
        }
    }
    output.push('"');
}

/// `JSON.stringify(value)` or `JSON.stringify(value, null, indent)`.
pub fn stringify(value: &JsValue, indent: usize) -> String {
    let mut output = String::new();
    write_value(value, indent, 0, &mut output);
    output
}

fn write_value(value: &JsValue, indent: usize, depth: usize, output: &mut String) {
    let newline = |output: &mut String, depth: usize| {
        if indent > 0 {
            output.push('\n');
            output.push_str(&" ".repeat(indent * depth));
        }
    };
    match value {
        JsValue::Null => output.push_str("null"),
        JsValue::Bool(b) => output.push_str(if *b { "true" } else { "false" }),
        JsValue::Number(n) if n.is_finite() => output.push_str(&number_to_string(*n)),
        JsValue::Number(_) => output.push_str("null"),
        JsValue::String(s) => quote(s, output),
        JsValue::Array(items) => {
            if items.is_empty() {
                output.push_str("[]");
                return;
            }
            output.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                newline(output, depth + 1);
                write_value(item, indent, depth + 1, output);
            }
            newline(output, depth);
            output.push(']');
        }
        JsValue::Object(object) => {
            if object.0.is_empty() {
                output.push_str("{}");
                return;
            }
            output.push('{');
            for (index, (key, item)) in object.0.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                newline(output, depth + 1);
                quote(key, output);
                output.push(':');
                if indent > 0 {
                    output.push(' ');
                }
                write_value(item, indent, depth + 1, output);
            }
            newline(output, depth);
            output.push('}');
        }
    }
}

/// `JSON.parse`, including V8's error messages.
pub fn parse(text: &str) -> Result<JsValue, String> {
    let mut parser = Parser {
        units: text.encode_utf16().collect(),
        at: 0,
    };
    parser.whitespace();
    let value = parser.value(0)?;
    parser.whitespace();
    if parser.at < parser.units.len() {
        return Err(parser.positioned("Unexpected non-whitespace character after JSON", parser.at));
    }
    Ok(value)
}

/// Works on UTF-16 units, like V8, so positions and context match.
struct Parser {
    units: Vec<u16>,
    at: usize,
}

fn unit_is(unit: Option<u16>, byte: u8) -> bool {
    unit == Some(byte as u16)
}

fn is_digit(unit: Option<u16>) -> bool {
    unit.is_some_and(|u| (b'0' as u16..=b'9' as u16).contains(&u))
}

impl Parser {
    fn peek(&self) -> Option<u16> {
        self.units.get(self.at).copied()
    }

    fn slice(&self, start: usize, end: usize) -> String {
        String::from_utf16_lossy(&self.units[start..end.min(self.units.len())])
    }

    /// `<message> in JSON at position N (line L column C)`.
    fn positioned(&self, message: &str, position: usize) -> String {
        let (mut line, mut line_start, mut index) = (1, 0, 0);
        while index < position.min(self.units.len()) {
            let unit = self.units[index];
            if unit == b'\r' as u16 && self.units.get(index + 1) == Some(&(b'\n' as u16)) {
                index += 1;
            }
            if unit == b'\n' as u16 || unit == b'\r' as u16 {
                line += 1;
                line_start = index + 1;
            }
            index += 1;
        }
        let place = if message.ends_with("after JSON") {
            ""
        } else {
            " in JSON"
        };
        format!(
            "{message}{place} at position {position} (line {line} column {})",
            position - line_start + 1
        )
    }

    /// V8's report for a token nothing more specific explains.
    fn unexpected(&self, position: usize) -> String {
        let Some(&unit) = self.units.get(position) else {
            return "Unexpected end of JSON input".into();
        };
        if unit == b'-' as u16 || is_digit(Some(unit)) {
            return self.positioned("Unexpected number", position);
        }
        if unit == b'"' as u16 {
            return self.positioned("Unexpected string", position);
        }
        let source = self.slice(0, self.units.len());
        if ["[object Object]", "undefined", "Infinity", "NaN"].contains(&source.as_str()) {
            return format!("\"{source}\" is not valid JSON");
        }
        let token = String::from_utf16_lossy(&[unit]);
        let length = self.units.len();
        const CONTEXT: usize = 10;
        if length < CONTEXT * 2 + 1 {
            return format!("Unexpected token '{token}', \"{source}\" is not valid JSON");
        }
        if position < CONTEXT {
            format!(
                "Unexpected token '{token}', \"{}\"... is not valid JSON",
                self.slice(0, position + CONTEXT)
            )
        } else if position < length - CONTEXT {
            let context = self.slice(position - CONTEXT, position + CONTEXT);
            format!("Unexpected token '{token}', ...\"{context}\"... is not valid JSON")
        } else {
            format!(
                "Unexpected token '{token}', ...\"{}\" is not valid JSON",
                self.slice(position - CONTEXT, length)
            )
        }
    }

    fn whitespace(&mut self) {
        while self
            .peek()
            .is_some_and(|u| matches!(u, 0x20 | 0x09 | 0x0a | 0x0d))
        {
            self.at += 1;
        }
    }

    fn literal(&mut self, word: &str, value: JsValue) -> Result<JsValue, String> {
        for expected in word.bytes().skip(1) {
            self.at += 1;
            match self.peek() {
                None => return Err(self.unexpected(self.at)),
                Some(unit) if unit != expected as u16 => return Err(self.unexpected(self.at)),
                _ => {}
            }
        }
        self.at += 1;
        Ok(value)
    }

    fn value(&mut self, depth: usize) -> Result<JsValue, String> {
        if depth > 5000 {
            return Err("Maximum call stack size exceeded".into());
        }
        let Some(unit) = self.peek() else {
            return Err(self.unexpected(self.at));
        };
        match u8::try_from(unit).unwrap_or(0) {
            b'{' => self.object(depth),
            b'[' => self.array(depth),
            b'"' => Ok(JsValue::String(self.string()?)),
            b't' => self.literal("true", JsValue::Bool(true)),
            b'f' => self.literal("false", JsValue::Bool(false)),
            b'n' => self.literal("null", JsValue::Null),
            b'-' | b'0'..=b'9' => self.number(),
            _ => Err(self.unexpected(self.at)),
        }
    }

    fn digits(&mut self) -> usize {
        let start = self.at;
        while is_digit(self.peek()) {
            self.at += 1;
        }
        self.at - start
    }

    fn number(&mut self) -> Result<JsValue, String> {
        let start = self.at;
        if unit_is(self.peek(), b'-') {
            self.at += 1;
            if !is_digit(self.peek()) {
                return Err(self.positioned("No number after minus sign", self.at));
            }
        }
        if unit_is(self.peek(), b'0') {
            self.at += 1;
            if is_digit(self.peek()) {
                return Err(self.unexpected(self.at));
            }
        } else {
            self.digits();
        }
        if unit_is(self.peek(), b'.') {
            self.at += 1;
            if self.digits() == 0 {
                return Err(self.positioned("Unterminated fractional number", self.at));
            }
        }
        if unit_is(self.peek(), b'e') || unit_is(self.peek(), b'E') {
            self.at += 1;
            if unit_is(self.peek(), b'+') || unit_is(self.peek(), b'-') {
                self.at += 1;
            }
            if self.digits() == 0 {
                return Err(self.positioned("Exponent part is missing a number", self.at));
            }
        }
        Ok(JsValue::Number(self.slice(start, self.at).parse().unwrap()))
    }

    fn string(&mut self) -> Result<String, String> {
        self.at += 1;
        let mut result: Vec<u16> = Vec::new();
        loop {
            let Some(unit) = self.peek() else {
                return Err(self.positioned("Unterminated string", self.units.len()));
            };
            match unit {
                0x22 => {
                    self.at += 1;
                    // Lone surrogates cannot live in Rust text.
                    return Ok(String::from_utf16_lossy(&result));
                }
                0x5c => {
                    self.at += 1;
                    let Some(escape) = self.peek() else {
                        return Err(self.unexpected(self.at));
                    };
                    let simple = match u8::try_from(escape).unwrap_or(0) {
                        b'"' => Some(0x22),
                        b'\\' => Some(0x5c),
                        b'/' => Some(0x2f),
                        b'b' => Some(0x08),
                        b'f' => Some(0x0c),
                        b'n' => Some(0x0a),
                        b'r' => Some(0x0d),
                        b't' => Some(0x09),
                        b'u' => None,
                        _ => return Err(self.positioned("Bad escaped character", self.at)),
                    };
                    self.at += 1;
                    match simple {
                        Some(unit) => result.push(unit),
                        None => {
                            let mut code = 0u16;
                            for _ in 0..4 {
                                let digit = self
                                    .peek()
                                    .and_then(|u| char::from_u32(u as u32))
                                    .and_then(|c| c.to_digit(16));
                                let Some(digit) = digit else {
                                    return Err(self.positioned("Bad Unicode escape", self.at));
                                };
                                code = code * 16 + digit as u16;
                                self.at += 1;
                            }
                            result.push(code);
                        }
                    }
                }
                0..=0x1f => {
                    return Err(self.positioned("Bad control character in string literal", self.at))
                }
                _ => {
                    result.push(unit);
                    self.at += 1;
                }
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<JsValue, String> {
        self.at += 1;
        let mut items = Vec::new();
        self.whitespace();
        if unit_is(self.peek(), b']') {
            self.at += 1;
            return Ok(JsValue::Array(items));
        }
        loop {
            self.whitespace();
            items.push(self.value(depth + 1)?);
            self.whitespace();
            if unit_is(self.peek(), b',') {
                self.at += 1;
            } else if unit_is(self.peek(), b']') {
                self.at += 1;
                return Ok(JsValue::Array(items));
            } else {
                return Err(self.positioned("Expected ',' or ']' after array element", self.at));
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<JsValue, String> {
        self.at += 1;
        let mut object = JsObject::new();
        self.whitespace();
        if unit_is(self.peek(), b'}') {
            self.at += 1;
            return Ok(JsValue::Object(object));
        }
        if !unit_is(self.peek(), b'"') {
            return Err(self.positioned("Expected property name or '}'", self.at));
        }
        loop {
            let key = self.string()?;
            self.whitespace();
            if !unit_is(self.peek(), b':') {
                return Err(self.positioned("Expected ':' after property name", self.at));
            }
            self.at += 1;
            self.whitespace();
            let value = self.value(depth + 1)?;
            object.insert(&key, value);
            self.whitespace();
            if unit_is(self.peek(), b',') {
                self.at += 1;
                self.whitespace();
                if !unit_is(self.peek(), b'"') {
                    return Err(self.positioned("Expected double-quoted property name", self.at));
                }
            } else if unit_is(self.peek(), b'}') {
                self.at += 1;
                return Ok(JsValue::Object(object));
            } else {
                return Err(self.positioned("Expected ',' or '}' after property value", self.at));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_print_like_javascript() {
        for (value, text) in [
            (1.0, "1"),
            (1.95, "1.95"),
            (-0.5, "-0.5"),
            (1e21, "1e+21"),
            (1e20, "100000000000000000000"),
            (1e-7, "1e-7"),
            (0.000001, "0.000001"),
            (123.456e5, "12345600"),
            (1.5e-10, "1.5e-10"),
        ] {
            assert_eq!(number_to_string(value), text);
        }
    }

    #[test]
    fn property_order_and_layout() {
        let value = parse(r#"{"b":1,"2":[],"a":{"x":"é\n"},"1":{},"b":2}"#).unwrap();
        assert_eq!(
            stringify(&value, 0),
            r#"{"1":{},"2":[],"b":2,"a":{"x":"é\n"}}"#
        );
        assert_eq!(
            stringify(&parse("[1,[2]]").unwrap(), 2),
            "[\n  1,\n  [\n    2\n  ]\n]"
        );
    }

    #[test]
    fn syntax_errors_match_v8() {
        for (source, message) in [
            ("", "Unexpected end of JSON input"),
            (
                "{broken",
                "Expected property name or '}' in JSON at position 1 (line 1 column 2)",
            ),
            ("bad", "Unexpected token 'b', \"bad\" is not valid JSON"),
            (
                "{\"a\":1}\n\nx",
                "Unexpected non-whitespace character after JSON at position 9 (line 3 column 1)",
            ),
            ("[1,]", "Unexpected token ']', \"[1,]\" is not valid JSON"),
            (
                "01",
                "Unexpected number in JSON at position 1 (line 1 column 2)",
            ),
            (
                "\"\\u12\"",
                "Bad Unicode escape in JSON at position 5 (line 1 column 6)",
            ),
            (
                "{\"a\":1,}",
                "Expected double-quoted property name in JSON at position 7 (line 1 column 8)",
            ),
            ("NaN", "\"NaN\" is not valid JSON"),
            (
                "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
                "Unexpected token 'x', \"xxxxxxxxxx\"... is not valid JSON",
            ),
            (
                "[1,2,3,4,5,6,7,8,9,10,11,x]",
                "Unexpected token 'x', ...\"8,9,10,11,x]\" is not valid JSON",
            ),
        ] {
            assert_eq!(parse(source).unwrap_err(), message, "{source:?}");
        }
    }
}
