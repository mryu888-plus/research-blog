//! `urllib.parse.urlsplit`, `urljoin` and `unquote` from Python 3.13.

use anyhow::{bail, Result};
use std::net::{IpAddr, Ipv6Addr};
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Split {
    pub scheme: String,
    pub netloc: String,
    pub path: String,
    pub query: String,
    pub fragment: String,
}

const USES_RELATIVE: &[&str] = &[
    "", "ftp", "http", "gopher", "nntp", "imap", "wais", "file", "https", "shttp", "mms",
    "prospero", "rtsp", "rtsps", "rtspu", "sftp", "svn", "svn+ssh", "ws", "wss",
];
const USES_NETLOC: &[&str] = &[
    "",
    "ftp",
    "http",
    "gopher",
    "nntp",
    "telnet",
    "imap",
    "wais",
    "file",
    "mms",
    "https",
    "shttp",
    "snews",
    "prospero",
    "rtsp",
    "rtsps",
    "rtspu",
    "rsync",
    "svn",
    "svn+ssh",
    "sftp",
    "nfs",
    "git",
    "git+ssh",
    "ws",
    "wss",
    "itms-services",
];
const USES_PARAMS: &[&str] = &[
    "", "ftp", "hdl", "prospero", "http", "imap", "https", "shttp", "rtsp", "rtsps", "rtspu",
    "sip", "sips", "mms", "sftp", "tel",
];

fn c0_or_space(c: char) -> bool {
    c <= ' '
}

fn check_bracketed_host(hostname: &str) -> Result<()> {
    if let Some(rest) = hostname.strip_prefix('v') {
        let valid = rest.split_once('.').is_some_and(|(hex, tail)| {
            !hex.is_empty()
                && hex.bytes().all(|b| b.is_ascii_hexdigit())
                && !tail.is_empty()
                && !tail.contains('\n')
        });
        if !valid {
            bail!("IPvFuture address is invalid");
        }
        return Ok(());
    }
    let address = hostname
        .split_once('%')
        .map_or(hostname, |(address, _)| address);
    let scope_ok = hostname
        .split_once('%')
        .is_none_or(|(_, scope)| !scope.is_empty() && !scope.contains('%'));
    if scope_ok && address.parse::<Ipv6Addr>().is_ok() {
        return Ok(());
    }
    if !hostname.contains('%') && hostname.parse::<IpAddr>().is_ok() {
        bail!("An IPv4 address cannot be in brackets");
    }
    bail!(
        "{} does not appear to be an IPv4 or IPv6 address",
        python_repr(hostname)
    )
}

/// Python `repr` of a string.
pub fn python_repr(value: &str) -> String {
    let quote = if value.contains('\'') && !value.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut result = String::new();
    result.push(quote);
    for c in value.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c == quote => {
                result.push('\\');
                result.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                result.push_str(&format!("\\x{:02x}", c as u32))
            }
            c => result.push(c),
        }
    }
    result.push(quote);
    result
}

fn check_netloc(netloc: &str) -> Result<()> {
    if netloc.is_ascii() {
        return Ok(());
    }
    let stripped: String = netloc.chars().filter(|c| !"@:#?".contains(*c)).collect();
    let normalized: String = stripped.nfkc().collect();
    if stripped == normalized {
        return Ok(());
    }
    if normalized.contains(['/', '?', '#', '@', ':']) {
        bail!("netloc '{netloc}' contains invalid characters under NFKC normalization");
    }
    Ok(())
}

pub fn urlsplit(url: &str, default_scheme: &str) -> Result<Split> {
    let url: String = url
        .trim_start_matches(c0_or_space)
        .chars()
        .filter(|c| !"\t\r\n".contains(*c))
        .collect();
    let mut scheme: String = default_scheme
        .trim_matches(c0_or_space)
        .chars()
        .filter(|c| !"\t\r\n".contains(*c))
        .collect();
    let mut rest = url.as_str();
    if let Some(colon) = rest.find(':') {
        let head = &rest[..colon];
        if colon > 0
            && head.as_bytes()[0].is_ascii_alphabetic()
            && head
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
        {
            scheme = head.to_ascii_lowercase();
            rest = &rest[colon + 1..];
        }
    }
    let mut netloc = "";
    if let Some(after) = rest.strip_prefix("//") {
        let end = after.find(['/', '?', '#']).unwrap_or(after.len());
        netloc = &after[..end];
        rest = &after[end..];
        if netloc.contains('[') != netloc.contains(']') {
            bail!("Invalid IPv6 URL");
        }
        if netloc.contains('[') && netloc.contains(']') {
            let host_port = netloc.rsplit_once('@').map_or(netloc, |(_, host)| host);
            let hostname = match host_port.split_once('[') {
                Some((before, bracketed)) => {
                    if !before.is_empty() {
                        bail!("Invalid IPv6 URL");
                    }
                    let (hostname, port) = bracketed.split_once(']').unwrap_or((bracketed, ""));
                    if !port.is_empty() && !port.starts_with(':') {
                        bail!("Invalid IPv6 URL");
                    }
                    hostname
                }
                None => host_port.split(':').next().unwrap_or(""),
            };
            check_bracketed_host(hostname)?;
        }
    }
    let (rest, fragment) = rest.split_once('#').unwrap_or((rest, ""));
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    check_netloc(netloc)?;
    Ok(Split {
        scheme,
        netloc: netloc.to_string(),
        path: path.to_string(),
        query: query.to_string(),
        fragment: fragment.to_string(),
    })
}

fn urlunsplit(scheme: &str, netloc: &str, path: &str, query: &str, fragment: &str) -> String {
    let mut url = path.to_string();
    if !netloc.is_empty() {
        if !url.is_empty() && !url.starts_with('/') {
            url = format!("/{url}");
        }
        url = format!("//{netloc}{url}");
    } else if url.starts_with("//")
        || (!scheme.is_empty()
            && USES_NETLOC.contains(&scheme)
            && (url.is_empty() || url.starts_with('/')))
    {
        url = format!("//{url}");
    }
    if !scheme.is_empty() {
        url = format!("{scheme}:{url}");
    }
    if !query.is_empty() {
        url = format!("{url}?{query}");
    }
    if !fragment.is_empty() {
        url = format!("{url}#{fragment}");
    }
    url
}

struct Parsed {
    split: Split,
    params: String,
}

fn urlparse(url: &str, default_scheme: &str) -> Result<Parsed> {
    let mut split = urlsplit(url, default_scheme)?;
    let mut params = String::new();
    if USES_PARAMS.contains(&split.scheme.as_str()) && split.path.contains(';') {
        let path = split.path.clone();
        let index = match path.rfind('/') {
            Some(slash) => path[slash..].find(';').map(|i| slash + i),
            None => path.find(';'),
        };
        if let Some(index) = index {
            split.path = path[..index].to_string();
            params = path[index + 1..].to_string();
        }
    }
    Ok(Parsed { split, params })
}

fn urlunparse(parsed: &Parsed) -> String {
    let s = &parsed.split;
    let path = if parsed.params.is_empty() {
        s.path.clone()
    } else {
        format!("{};{}", s.path, parsed.params)
    };
    urlunsplit(&s.scheme, &s.netloc, &path, &s.query, &s.fragment)
}

pub fn urljoin(base: &str, url: &str) -> Result<String> {
    if base.is_empty() {
        return Ok(url.to_string());
    }
    if url.is_empty() {
        return Ok(base.to_string());
    }
    let b = urlparse(base, "")?;
    let mut u = urlparse(url, &b.split.scheme)?;
    let scheme = u.split.scheme.clone();
    if scheme != b.split.scheme || !USES_RELATIVE.contains(&scheme.as_str()) {
        return Ok(url.to_string());
    }
    if USES_NETLOC.contains(&scheme.as_str()) {
        if !u.split.netloc.is_empty() {
            return Ok(urlunparse(&u));
        }
        u.split.netloc = b.split.netloc.clone();
    }
    if u.split.path.is_empty() && u.params.is_empty() {
        u.split.path = b.split.path.clone();
        u.params = b.params.clone();
        if u.split.query.is_empty() {
            u.split.query = b.split.query.clone();
        }
        return Ok(urlunparse(&u));
    }
    let mut base_parts: Vec<&str> = b.split.path.split('/').collect();
    if base_parts.last() != Some(&"") {
        base_parts.pop();
    }
    let path = u.split.path.clone();
    let segments: Vec<&str> = if path.starts_with('/') {
        path.split('/').collect()
    } else {
        let mut segments: Vec<&str> = base_parts.into_iter().chain(path.split('/')).collect();
        if segments.len() > 2 {
            let last = segments.len() - 1;
            let middle: Vec<&str> = segments[1..last]
                .iter()
                .copied()
                .filter(|s| !s.is_empty())
                .collect();
            segments = std::iter::once(segments[0])
                .chain(middle)
                .chain(std::iter::once(segments[last]))
                .collect();
        }
        segments
    };
    let mut resolved: Vec<&str> = Vec::new();
    for segment in &segments {
        match *segment {
            ".." => {
                resolved.pop();
            }
            "." => {}
            other => resolved.push(other),
        }
    }
    if matches!(segments.last(), Some(&".") | Some(&"..")) {
        resolved.push("");
    }
    let joined = resolved.join("/");
    u.split.path = if joined.is_empty() {
        "/".to_string()
    } else {
        joined
    };
    Ok(urlunparse(&u))
}

fn hex_value(byte: u8) -> Option<u8> {
    (byte as char).to_digit(16).map(|d| d as u8)
}

/// Python `urllib.parse.unquote` (UTF-8, errors replaced).
pub fn unquote(text: &str) -> String {
    if !text.contains('%') {
        return text.to_string();
    }
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        let ascii_len = rest.bytes().take_while(u8::is_ascii).count();
        if ascii_len == 0 {
            let non_ascii = rest
                .char_indices()
                .find(|(_, c)| c.is_ascii())
                .map_or(rest.len(), |(i, _)| i);
            result.push_str(&rest[..non_ascii]);
            rest = &rest[non_ascii..];
            continue;
        }
        let run = &rest.as_bytes()[..ascii_len];
        let mut bytes = Vec::with_capacity(run.len());
        let mut i = 0;
        while i < run.len() {
            if run[i] == b'%' {
                if let (Some(high), Some(low)) = (
                    run.get(i + 1).and_then(|b| hex_value(*b)),
                    run.get(i + 2).and_then(|b| hex_value(*b)),
                ) {
                    bytes.push(high * 16 + low);
                    i += 3;
                    continue;
                }
            }
            bytes.push(run[i]);
            i += 1;
        }
        result.push_str(&String::from_utf8_lossy(&bytes));
        rest = &rest[ascii_len..];
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_like_python() {
        let base = "https://example.org/research/posts/a/";
        assert_eq!(
            urljoin(base, "../b/#x").unwrap(),
            "https://example.org/research/posts/b/#x"
        );
        assert_eq!(urljoin(base, "/x").unwrap(), "https://example.org/x");
        assert_eq!(
            urljoin(base, "#top").unwrap(),
            "https://example.org/research/posts/a/#top"
        );
        assert_eq!(
            urljoin(base, "//cdn.example/x").unwrap(),
            "https://cdn.example/x"
        );
        assert_eq!(urljoin(base, "mailto:a@b").unwrap(), "mailto:a@b");
        assert_eq!(urljoin("https://h/a/b", "..").unwrap(), "https://h/");
    }

    #[test]
    fn unquotes_utf8_and_keeps_bad_escapes() {
        assert_eq!(unquote("%E4%B8%AD%2e%zz%"), "中.%zz%");
        assert_eq!(unquote("%ff"), "\u{fffd}");
    }

    #[test]
    fn rejects_bad_ipv6() {
        assert!(urlsplit("http://[::1/x", "").is_err());
        assert!(urlsplit("http://[1.2.3.4]/", "").is_err());
        assert_eq!(urlsplit("http://[::1]:80/p?q#f", "").unwrap().fragment, "f");
    }
}
