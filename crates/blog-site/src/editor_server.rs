//! The local editing service behind the preview's drag-and-drop series
//! editor (site/static/series-editor.js). It mirrors the extension's
//! preview-server.cjs: same routes, checks, status codes and messages.

pub mod collections;
pub mod js;

use anyhow::{bail, Result};
use collections::{
    assign_article, catalog_revision, list_articles, read_catalog, Conflict, Expected,
};
use js::{JsObject, JsValue};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const BODY_LIMIT: usize = 16384;
const HEAD_LIMIT: usize = 16384;
const TIMEOUT: Duration = Duration::from_secs(5);

pub struct EditorServer {
    pub endpoint: String,
    pub token: String,
    stopping: Arc<AtomicBool>,
    port: u16,
    acceptor: JoinHandle<()>,
}

/// Only `http://127.0.0.1[:port]` and `http://localhost[:port]` in the exact
/// form a browser reports as `Origin`.
fn local_origin(origin: &str) -> bool {
    let Some(rest) = origin.strip_prefix("http://") else {
        return false;
    };
    let (host, port) = match rest.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (rest, None),
    };
    let port_ok = port.is_none_or(|port| {
        !port.is_empty()
            && !port.starts_with('0')
            && port.len() <= 5
            && port.bytes().all(|b| b.is_ascii_digit())
            && port.parse::<u32>().is_ok_and(|p| p <= 65535 && p != 80)
    });
    ["127.0.0.1", "localhost"].contains(&host) && port_ok
}

impl EditorServer {
    pub fn start(content_root: &Path, origin: &str) -> Result<Self> {
        if !local_origin(origin) {
            bail!("预览编辑仅支持本机 HTTP 地址。");
        }
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
        let token = hex::encode(bytes);
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let stopping = Arc::new(AtomicBool::new(false));
        let context = Arc::new(Context {
            content_root: content_root.to_path_buf(),
            origin: origin.to_string(),
            token: token.clone(),
            host: format!("127.0.0.1:{port}"),
        });
        let flag = stopping.clone();
        let acceptor = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if flag.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(stream) = stream else { continue };
                let context = context.clone();
                std::thread::spawn(move || {
                    let _ = serve(stream, &context);
                });
            }
        });
        Ok(EditorServer {
            endpoint: format!("http://127.0.0.1:{port}"),
            token,
            stopping,
            port,
            acceptor,
        })
    }

    /// What preview-server.cjs prints for the preview page.
    pub fn config_json(&self) -> String {
        let mut config = JsObject::new();
        config.insert("endpoint", JsValue::String(self.endpoint.clone()));
        config.insert("token", JsValue::String(self.token.clone()));
        js::stringify(&JsValue::Object(config), 0)
    }

    pub fn stop(self) {
        self.stopping.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        let _ = self.acceptor.join();
    }
}

struct Context {
    content_root: PathBuf,
    origin: String,
    token: String,
    host: String,
}

struct Request {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Node keeps the first value of these headers and joins the others with ", ".
const SINGLE_HEADERS: [&str; 18] = [
    "age",
    "authorization",
    "content-length",
    "content-type",
    "etag",
    "expires",
    "from",
    "host",
    "if-modified-since",
    "if-unmodified-since",
    "last-modified",
    "location",
    "max-forwards",
    "proxy-authorization",
    "referer",
    "retry-after",
    "server",
    "user-agent",
];

struct Connection {
    stream: TcpStream,
    buffer: Vec<u8>,
    deadline: Instant,
}

enum ReadError {
    Timeout,
    Closed,
    Invalid,
}

impl Connection {
    fn fill(&mut self) -> Result<(), ReadError> {
        let remaining = self
            .deadline
            .checked_duration_since(Instant::now())
            .ok_or(ReadError::Timeout)?;
        self.stream
            .set_read_timeout(Some(remaining.max(Duration::from_millis(1))))
            .map_err(|_| ReadError::Closed)?;
        let mut chunk = [0u8; 8192];
        match self.stream.read(&mut chunk) {
            Ok(0) => Err(ReadError::Closed),
            Ok(n) => {
                self.buffer.extend_from_slice(&chunk[..n]);
                Ok(())
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                Err(ReadError::Timeout)
            }
            Err(_) => Err(ReadError::Closed),
        }
    }

    fn head(&mut self) -> Result<Request, ReadError> {
        let end = loop {
            if let Some(end) = self.buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                break end;
            }
            if self.buffer.len() > HEAD_LIMIT {
                return Err(ReadError::Invalid);
            }
            self.fill()?;
        };
        let head = String::from_utf8_lossy(&self.buffer[..end]).into_owned();
        self.buffer.drain(..end + 4);
        let mut lines = head.split("\r\n");
        let mut parts = lines.next().unwrap_or("").split(' ');
        let (Some(method), Some(target), Some(version), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(ReadError::Invalid);
        };
        if !version.starts_with("HTTP/1.") || method.is_empty() || target.is_empty() {
            return Err(ReadError::Invalid);
        }
        let mut headers: Vec<(String, String)> = Vec::new();
        for line in lines {
            let Some((name, value)) = line.split_once(':') else {
                return Err(ReadError::Invalid);
            };
            if name.is_empty() || name.contains([' ', '\t']) {
                return Err(ReadError::Invalid);
            }
            let name = name.to_ascii_lowercase();
            let value = value.trim_matches([' ', '\t']).to_string();
            match headers.iter_mut().find(|(key, _)| *key == name) {
                Some(_) if SINGLE_HEADERS.contains(&name.as_str()) => {}
                Some((_, existing)) => {
                    existing.push_str(", ");
                    existing.push_str(&value);
                }
                None => headers.push((name, value)),
            }
        }
        Ok(Request {
            method: method.to_string(),
            target: target.to_string(),
            headers,
        })
    }

    /// The request body, or `Ok(None)` once it exceeds the limit.
    fn body(&mut self, request: &Request) -> Result<Option<Vec<u8>>, ReadError> {
        let chunked = request
            .header("transfer-encoding")
            .is_some_and(|v| v.to_ascii_lowercase().ends_with("chunked"));
        if !chunked {
            let length = match request.header("content-length") {
                Some(value) => value.parse::<usize>().map_err(|_| ReadError::Invalid)?,
                None => 0,
            };
            while self.buffer.len() < length.min(BODY_LIMIT + 1) {
                self.fill()?;
            }
            if length > BODY_LIMIT {
                return Ok(None);
            }
            return Ok(Some(self.buffer.drain(..length).collect()));
        }
        let mut body = Vec::new();
        loop {
            let line_end = loop {
                if let Some(end) = self.buffer.windows(2).position(|w| w == b"\r\n") {
                    break end;
                }
                if self.buffer.len() > 1024 {
                    return Err(ReadError::Invalid);
                }
                self.fill()?;
            };
            let size_text = String::from_utf8_lossy(&self.buffer[..line_end]).into_owned();
            let size_text = size_text.split(';').next().unwrap_or("").trim();
            let size = usize::from_str_radix(size_text, 16).map_err(|_| ReadError::Invalid)?;
            self.buffer.drain(..line_end + 2);
            if size == 0 {
                return Ok(Some(body));
            }
            while self.buffer.len() < size + 2 {
                if body.len() + self.buffer.len() > BODY_LIMIT {
                    return Ok(None);
                }
                self.fill()?;
            }
            body.extend(self.buffer.drain(..size));
            self.buffer.drain(..2);
            if body.len() > BODY_LIMIT {
                return Ok(None);
            }
        }
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        408 => "Request Timeout",
        409 => "Conflict",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        _ => "Error",
    }
}

fn respond(stream: &mut TcpStream, status: u16, extra: &[(&str, &str)], body: Option<&JsValue>) {
    let mut head = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: application/json; charset=utf-8\r\nCache-Control: no-store\r\n",
        reason(status)
    );
    for (name, value) in extra {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    let body = if status == 204 {
        String::new()
    } else {
        body.map(|b| js::stringify(b, 0)).unwrap_or_default()
    };
    if status != 204 {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    head.push_str("Connection: close\r\n\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
}

fn error_body(message: &str) -> JsValue {
    let mut object = JsObject::new();
    object.insert("error", JsValue::String(message.to_string()));
    JsValue::Object(object)
}

fn snapshot(context: &Context) -> Result<JsValue> {
    let catalog = read_catalog(&context.content_root)?;
    let revision = catalog_revision(&catalog);
    let articles = list_articles(&context.content_root)?
        .into_iter()
        .map(|article| {
            let mut object = JsObject::new();
            object.insert("key", JsValue::String(article.key));
            object.insert("title", article.title);
            object.insert("draft", JsValue::Bool(article.draft));
            JsValue::Object(object)
        })
        .collect();
    let mut object = JsObject::new();
    object.insert("catalog", catalog);
    object.insert("revision", JsValue::String(revision));
    object.insert("articles", JsValue::Array(articles));
    Ok(JsValue::Object(object))
}

fn serve(stream: TcpStream, context: &Context) -> std::io::Result<()> {
    let mut connection = Connection {
        stream: stream.try_clone()?,
        buffer: Vec::new(),
        deadline: Instant::now() + TIMEOUT,
    };
    let mut stream = stream;
    let finish = |stream: &mut TcpStream, connection: &mut Connection| {
        // Read what the client is still sending so closing does not reset
        // the connection before it reads the response.
        let _ = stream.shutdown(Shutdown::Write);
        connection.deadline = Instant::now() + Duration::from_millis(300);
        let mut drained = 0;
        while drained < 1 << 20 && connection.fill().is_ok() {
            drained += connection.buffer.len();
            connection.buffer.clear();
        }
    };
    let request = match connection.head() {
        Ok(request) => request,
        Err(ReadError::Timeout) => {
            respond(&mut stream, 408, &[], None);
            return Ok(());
        }
        Err(ReadError::Invalid) => {
            let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n");
            return Ok(());
        }
        Err(ReadError::Closed) => return Ok(()),
    };
    if request
        .header("expect")
        .is_some_and(|v| v.eq_ignore_ascii_case("100-continue"))
    {
        stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n")?;
    }
    handle(&mut stream, &mut connection, &request, context);
    finish(&mut stream, &mut connection);
    Ok(())
}

fn handle(
    stream: &mut TcpStream,
    connection: &mut Connection,
    request: &Request,
    context: &Context,
) {
    // Even possession of the token never authorizes a different web origin.
    if request.header("host") != Some(&context.host)
        || request.header("origin") != Some(&context.origin)
    {
        return respond(
            stream,
            403,
            &[],
            Some(&error_body("请从本地博客预览中编辑。")),
        );
    }
    let cors = [
        ("Access-Control-Allow-Origin", context.origin.as_str()),
        ("Vary", "Origin"),
    ];
    if request.method == "OPTIONS" {
        let headers = [
            cors[0],
            cors[1],
            ("Access-Control-Allow-Methods", "GET, POST"),
            (
                "Access-Control-Allow-Headers",
                "Content-Type, X-Preview-Token",
            ),
        ];
        return respond(stream, 204, &headers, None);
    }
    if request.header("x-preview-token") != Some(&context.token) {
        return respond(
            stream,
            403,
            &cors,
            Some(&error_body("预览会话已失效，请重新打开预览。")),
        );
    }
    let result = (|| -> Result<(u16, JsValue)> {
        if request.method == "GET" && request.target == "/library" {
            return Ok((200, snapshot(context)?));
        }
        if request.method != "POST" || request.target != "/move" {
            return Ok((404, error_body("没有这个编辑操作。")));
        }
        let content_type = request
            .header("content-type")
            .unwrap_or("")
            .to_ascii_lowercase();
        let json_type = content_type
            .strip_prefix("application/json")
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(';'));
        if !json_type {
            return Ok((415, error_body("需要 JSON 请求。")));
        }
        // Number(header) > 16384; a non-numeric header is NaN and passes.
        if request
            .header("content-length")
            .and_then(|v| v.trim().parse::<f64>().ok())
            .is_some_and(|n| n > 16384.0)
        {
            return Ok((413, error_body("请求过大。")));
        }
        let body = match connection.body(request) {
            Ok(Some(body)) => body,
            Ok(None) => return Ok((413, error_body("请求过大。"))),
            Err(ReadError::Timeout) => return Ok((408, JsValue::Null)),
            Err(_) => bail!("请求不完整。"),
        };
        let data =
            js::parse(&String::from_utf8_lossy(&body)).map_err(|error| anyhow::anyhow!(error))?;
        let key = data.prop("key").and_then(JsValue::as_str);
        let nullable = |name: &str| match data.prop(name) {
            Some(JsValue::Null) => Some(None),
            Some(JsValue::String(text)) => Some(Some(text.as_str())),
            _ => None,
        };
        let revision = data.prop("revision").filter(|r| r.truthy());
        let revision_text = revision.map(JsValue::to_js_string).unwrap_or_default();
        let revision_ok = revision_text.len() == 64
            && revision_text
                .bytes()
                .all(|b| matches!(b, b'a'..=b'f' | b'0'..=b'9'));
        let (Some(key), Some(target_id), Some(before_key), true, true) = (
            key,
            nullable("targetId"),
            nullable("beforeKey"),
            data.truthy(),
            revision_ok,
        ) else {
            return Ok((400, error_body("移动请求不完整，请刷新后重试。")));
        };
        assign_article(
            &context.content_root,
            key,
            target_id,
            Some(before_key),
            Expected::Revision(revision.unwrap()),
        )?;
        Ok((200, snapshot(context)?))
    })();
    match result {
        Ok((408, _)) => respond(stream, 408, &[], None),
        Ok((status, body)) => respond(stream, status, &cors, Some(&body)),
        Err(error) => {
            let status = if error.downcast_ref::<Conflict>().is_some() {
                409
            } else {
                400
            };
            respond(stream, status, &cors, Some(&error_body(&error.to_string())));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::collections::{catalog_path, create_article, create_series};
    use super::*;

    fn send(endpoint: &str, raw: String) -> (u16, String, String) {
        let address = endpoint.trim_start_matches("http://");
        let mut stream = TcpStream::connect(address).unwrap();
        stream.write_all(raw.as_bytes()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let status = response[9..12].parse().unwrap();
        let (head, body) = response.split_once("\r\n\r\n").unwrap();
        (status, head.to_string(), body.to_string())
    }

    fn request(
        server: &EditorServer,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &str,
    ) -> (u16, String, String) {
        let host = server.endpoint.trim_start_matches("http://").to_string();
        let mut raw = format!(
            "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Length: {}\r\n",
            body.len()
        );
        for (name, value) in headers {
            raw.push_str(&format!("{name}: {value}\r\n"));
        }
        raw.push_str("\r\n");
        raw.push_str(body);
        send(&server.endpoint, raw)
    }

    #[test]
    fn persists_only_authenticated_same_origin_moves() {
        let root = std::env::temp_dir().join(format!("blog-preview-editor-{}", std::process::id()));
        let content = root.join("site/content");
        std::fs::create_dir_all(&content).unwrap();
        let series = create_series(&content, "系列", "").unwrap();
        let article = create_article(&content, "可拖动草稿", None).unwrap();
        let server = EditorServer::start(&content, "http://127.0.0.1:1111").unwrap();
        let config = js::parse(&server.config_json()).unwrap();
        assert_eq!(
            config.prop("token").and_then(JsValue::as_str),
            Some(server.token.as_str())
        );
        let origin = ("Origin", "http://127.0.0.1:1111");
        let token = ("X-Preview-Token", server.token.as_str());
        let json = ("Content-Type", "application/json");
        let saved = std::fs::read(catalog_path(&content).unwrap()).unwrap();
        for rejected in [
            vec![],
            vec![("Origin", "https://example.test"), token, json],
            vec![origin, ("X-Preview-Token", "wrong"), json],
        ] {
            assert_eq!(request(&server, "POST", "/move", &rejected, "{}").0, 403);
        }
        let raw = format!("GET /library HTTP/1.1\r\nHost: attacker.test\r\nOrigin: {}\r\nX-Preview-Token: {}\r\n\r\n", origin.1, token.1);
        assert_eq!(send(&server.endpoint, raw).0, 403);
        assert_eq!(
            std::fs::read(catalog_path(&content).unwrap()).unwrap(),
            saved
        );
        let (status, head, _) = request(&server, "OPTIONS", "/move", &[origin], "");
        assert_eq!(status, 204);
        assert!(head.contains("Access-Control-Allow-Origin: http://127.0.0.1:1111"));
        let state =
            js::parse(&request(&server, "GET", "/library", &[origin, token, json], "").2).unwrap();
        let first = &state.prop("articles").unwrap().as_array().unwrap()[0];
        assert_eq!(
            first.prop("key").and_then(JsValue::as_str),
            Some(article.key.as_str())
        );
        assert!(first.prop("file").is_none());
        let revision = state
            .prop("revision")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();
        let series_id = series.prop("id").unwrap().as_str().unwrap();
        let body = format!(
            r#"{{"key":"{}","targetId":"{series_id}","beforeKey":null,"revision":"{revision}"}}"#,
            article.key
        );
        let (status, _, moved) = request(&server, "POST", "/move", &[origin, token, json], &body);
        assert_eq!(status, 200);
        let next = js::parse(&moved).unwrap();
        assert_ne!(next.prop("revision"), state.prop("revision"));
        let stale = body.replace(&format!("\"{series_id}\""), "null");
        assert_eq!(
            request(&server, "POST", "/move", &[origin, token, json], &stale).0,
            409
        );
        let next_revision = next.prop("revision").unwrap().as_str().unwrap();
        let bad = format!(
            r#"{{"key":"../outside","targetId":null,"beforeKey":null,"revision":"{next_revision}"}}"#
        );
        assert_eq!(
            request(&server, "POST", "/move", &[origin, token, json], &bad).0,
            400
        );
        let missing = format!(
            r#"{{"key":"{}","targetId":null,"beforeKey":null}}"#,
            article.key
        );
        assert_eq!(
            request(&server, "POST", "/move", &[origin, token, json], &missing).0,
            400
        );
        let big = "x".repeat(BODY_LIMIT + 1);
        assert_eq!(
            request(&server, "POST", "/move", &[origin, token, json], &big).0,
            413
        );
        assert_eq!(
            request(&server, "POST", "/move", &[origin, token], "{}").0,
            415
        );
        assert_eq!(
            request(&server, "GET", "/other", &[origin, token], "").0,
            404
        );
        let catalog = read_catalog(&content).unwrap();
        let articles = catalog.prop("series").unwrap().as_array().unwrap()[0]
            .prop("articles")
            .unwrap()
            .clone();
        assert_eq!(
            articles,
            JsValue::Array(vec![JsValue::String(article.key.clone())])
        );
        server.stop();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_non_local_origins() {
        let content = std::env::temp_dir();
        for origin in [
            "https://127.0.0.1:1111",
            "http://example.test",
            "http://127.0.0.1:1111/",
            "http://127.0.0.1:80",
        ] {
            assert!(EditorServer::start(&content, origin).is_err(), "{origin}");
        }
        EditorServer::start(&content, "http://localhost:1112")
            .unwrap()
            .stop();
    }
}
