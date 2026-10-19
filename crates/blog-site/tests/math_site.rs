//! Ported from tests/test_math_site.py: protected equations through the real
//! Zola build and live server.

mod common;

use blog_site::prepare::PreparedSite;
use common::{unescape, write, zola, zola_build, TempDir};
use regex::Regex;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const HEADER: &str = "+++\ntitle = \"Math\"\n+++\n\n";

/// What tests/test_math_site.py's RenderedPage collects from a page.
struct Rendered {
    formulas: Vec<String>,
    code: String,
    cells: usize,
}

fn rendered(html: &str) -> Rendered {
    let span = Regex::new(r#"<span\b[^>]*?\bdata-math-tex="([^"]*)""#).unwrap();
    let code = Regex::new(r"(?s)<code\b[^>]*>(.*?)</code>").unwrap();
    let tag = Regex::new(r"(?s)<[^>]*>").unwrap();
    Rendered {
        formulas: span.captures_iter(html).map(|c| unescape(&c[1])).collect(),
        code: code
            .captures_iter(html)
            .map(|c| unescape(&tag.replace_all(&c[1], "")))
            .collect(),
        cells: Regex::new(r"<td\b").unwrap().find_iter(html).count(),
    }
}

struct Site {
    _dir: TempDir,
    dir: PathBuf,
    article: PathBuf,
    prepared: PreparedSite,
}

fn site() -> Site {
    let dir = TempDir::new("math");
    let source = dir.path().join("source");
    write(
        &source,
        "config.toml",
        "base_url = \"https://example.test/blog/\"\n",
    );
    write(&source, "content/_index.md", "+++\n+++\n");
    write(
        &source,
        "templates/index.html",
        "{{ section.content | safe }}",
    );
    write(&source, "templates/page.html", "{{ page.content | safe }}");
    let prepared = PreparedSite::new(&source, &dir.path().join("prepared")).unwrap();
    Site {
        dir: dir.path().to_path_buf(),
        article: source.join("content/math.md"),
        prepared,
        _dir: dir,
    }
}

#[test]
fn real_markdown_preserves_tex_code_and_table_columns() {
    let mut s = site();
    let equation = r"\forall s \in S: Q^*_{\mathcal{M}}(s, \tau_1) = Q^*_{\mathcal{M}}(s, \tau_2)";
    let matrix =
        String::from(r"\begin{aligned}x_1 &= \{a,b\} \\") + "\n" + r"x_2 &= {{c}}\end{aligned}";
    let body = format!(
        "$f(x)$\n\n$$\n{equation}\n$$\n\n$$\n{matrix}\n$$\n\n| math | label |\n| --- | --- |\n| $a|b$ | ordinary |\n\n```latex\n$$ Q^*_{{x}} $$\n```\n\n`$inline_code$`\n\n- > ~~~latex\n  > $nested_code$\n  > ~~~\n\n\\(a_b\\)\n\n\\[\\{{x\\}}\\]\n"
    );
    let original = format!("{HEADER}{body}");
    fs::write(&s.article, &original).unwrap();
    s.prepared.sync().unwrap();
    let public = zola_build(&s.prepared.target, false);
    let page = rendered(&fs::read_to_string(public.join("math/index.html")).unwrap());
    assert_eq!(
        page.formulas,
        [
            "f(x)".to_string(),
            format!("\n{equation}\n"),
            format!("\n{matrix}\n"),
            "a|b".into(),
            "a_b".into(),
            r"\{x\}".into()
        ]
    );
    assert_eq!(page.cells, 2);
    assert!(page.code.contains("$$ Q^*_{x} $$"));
    assert!(page.code.contains("$inline_code$"));
    assert!(page.code.contains("$nested_code$"));
    assert!(!page.code.contains("math-source"));
    assert_eq!(fs::read_to_string(&s.article).unwrap(), original);
}

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn get(port: u16, path: &str) -> Option<(u16, String)> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(1))).ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\n\r\n"
    )
    .ok()?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).ok()?;
    let response = String::from_utf8_lossy(&response).into_owned();
    let status = response.split(' ').nth(1)?.parse().ok()?;
    let body = response.split_once("\r\n\r\n")?.1.to_string();
    Some((status, body))
}

#[test]
fn prepared_preview_updates_and_recovers_incomplete_math() {
    let mut s = site();
    fs::write(&s.article, format!("{HEADER}$x_1$")).unwrap();
    s.prepared.sync().unwrap();
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let log = s.dir.join("zola-preview.log");
    let child = Command::new(zola())
        .arg("--root")
        .arg(&s.prepared.target)
        .args([
            "serve",
            "--port",
            &port.to_string(),
            "--interface",
            "127.0.0.1",
        ])
        .args([
            "--base-url",
            "http://127.0.0.1",
            "--debounce",
            "1",
            "--force",
        ])
        .stdout(Stdio::from(fs::File::create(&log).unwrap()))
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut server = Server(child);
    let mut await_page = |predicate: &dyn Fn(&str) -> bool| {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut last = String::new();
        while Instant::now() < deadline {
            assert!(
                server.0.try_wait().unwrap().is_none(),
                "Zola preview stopped unexpectedly"
            );
            if let Some((status, html)) = get(port, "/math/") {
                if status == 200 && predicate(&html) {
                    return;
                }
                last = html;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!(
            "timed out waiting for updated math\nlast response: {last:?}\n{}",
            fs::read_to_string(&log).unwrap()
        );
    };
    await_page(&|html| rendered(html).formulas == ["x_1"]);
    fs::write(&s.article, format!("{HEADER}$unfinished")).unwrap();
    s.prepared.sync().unwrap();
    await_page(&|html| html.contains("$unfinished") && rendered(html).formulas.is_empty());
    // The staging layer deliberately preserves the original TeX line endings.
    for (newline, variable) in [("\n", "x"), ("\r\n", "y")] {
        let equation = format!("Q^*_{{{variable}}} = \\{{1\\}}");
        let tex = format!("{newline}{equation}{newline}");
        let source = HEADER.replace('\n', newline) + "$$" + &tex + "$$";
        fs::write(&s.article, source).unwrap();
        s.prepared.sync().unwrap();
        await_page(&|html| rendered(html).formulas == [tex.clone()]);
    }
}
