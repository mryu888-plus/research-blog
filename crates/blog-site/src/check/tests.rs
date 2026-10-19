//! Cases from tests/test_site_features.py, plus a differential driver.

use super::*;
use std::path::PathBuf;

struct Fixture {
    _directory: TempDir,
    site: PathBuf,
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temp_dir() -> TempDir {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let path = std::env::temp_dir().join(format!(
        "generic-site-test-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&path).unwrap();
    TempDir(path)
}

impl Fixture {
    fn new() -> Self {
        let directory = temp_dir();
        let site = directory.0.join("site");
        fs::create_dir(&site).unwrap();
        let fixture = Fixture {
            _directory: directory,
            site,
        };
        fixture.config(false);
        fixture
    }

    fn config(&self, enabled: bool) {
        fs::write(
            self.site.join("config.toml"),
            format!(
                "base_url = \"https://example.org/research/\"\n[extra]\nprofile_enabled = {enabled}\nagent_endpoint = \"\"\n"
            ),
        )
        .unwrap();
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.site.join("public").join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn output_with(&self, base: &str) {
        self.write(
            "index.html",
            &format!("<a href=\"{base}posts/new/\">Read</a>"),
        );
        self.write(
            "posts/new/index.html",
            "<h1 id=\"result\">Result</h1><a href=\"#result\">Section</a>",
        );
        for relative in ["404.html", "graph/index.html", "references/index.html"] {
            self.write(relative, "<main>Research</main>");
        }
        self.write("atom.xml", "<feed/>");
        self.write("sitemap.xml", "<urlset/>");
        self.write(
            "search-index/index.html",
            &format!(
                "[{{\"title\": \"A completely different paper\", \"url\": \"{base}posts/new/\"}}]"
            ),
        );
        self.graph(base, false);
    }

    fn graph(&self, base: &str, draft: bool) {
        self.write(
            "graph-data/index.html",
            &format!(
                "[{{\"id\": \"posts/new/index.md\", \"url\": \"{base}posts/new/\", \"draft\": {draft}, \"html\": \"<h1>Result</h1>\", \"headings\": []}}]"
            ),
        );
    }

    fn output(&self) {
        self.output_with("https://example.org/research/");
    }

    fn validate(&self) -> Result<CheckSummary> {
        validate_site(&self.site, None, None)
    }
}

fn failure(result: Result<CheckSummary>) -> String {
    match result {
        Ok(_) => panic!("validation passed"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn generic_article_and_missing_about_profile_pass() {
    let fixture = Fixture::new();
    fixture.output();
    let result = fixture.validate().unwrap();
    assert_eq!((result.articles, result.pages, result.links), (1, 5, 2));
    assert!(!fixture.site.join("public/about").exists());
}

#[test]
fn empty_library_is_valid() {
    let fixture = Fixture::new();
    fixture.output();
    fixture.write("search-index/index.html", "[]");
    fixture.write("graph-data/index.html", "[]");
    assert_eq!(fixture.validate().unwrap().articles, 0);
}

#[test]
fn prefix_build_can_be_checked_with_explicit_base_url() {
    let fixture = Fixture::new();
    let base = "https://preview.example.org/another-prefix/";
    fixture.output_with(base);
    assert!(failure(fixture.validate()).contains("must belong to the site"));
    assert_eq!(
        validate_site(&fixture.site, None, Some(base))
            .unwrap()
            .articles,
        1
    );
}

#[test]
fn missing_link_anchor_and_graph_draft_are_reported() {
    let fixture = Fixture::new();
    fixture.output();
    fixture.write(
        "index.html",
        "<a href=\"missing/\">Gone</a><a href=\"posts/new/#wrong\">Anchor</a>",
    );
    fixture.graph("https://example.org/research/", true);
    let message = failure(fixture.validate());
    assert!(message.contains("missing target"));
    assert!(message.contains("missing anchor"));
    assert!(message.contains("Draft leaked"));
}

#[test]
fn encoded_link_cannot_escape_output() {
    let fixture = Fixture::new();
    fixture.output();
    fixture.write("index.html", "<a href=\"%2e%2e/secret.txt\">Secret</a>");
    fs::write(fixture.site.join("secret.txt"), "outside").unwrap();
    assert!(failure(fixture.validate()).contains("escapes output directory"));
}

#[test]
fn private_credentials_are_rejected_without_profile_enabled() {
    let fixture = Fixture::new();
    fixture.output();
    fixture.write("nested/preview-editor.json", "{\"token\":\"private\"}");
    fixture.write(
        "index.html",
        "<script id=\"series-editor-script\"></script>",
    );
    let message = failure(fixture.validate());
    assert!(message.contains("Private build input leaked"));
    assert!(message.contains("local editing is enabled"));
}

#[test]
fn enabled_profile_checks_generated_outputs() {
    let fixture = Fixture::new();
    fixture.output();
    fixture.config(true);
    let message = failure(fixture.validate());
    assert!(message.contains("generated metadata is missing/invalid"));
    assert!(message.contains("/about/ is missing"));
    assert!(message.contains("public resume is missing/invalid"));
}

/// Differential driver for `.tools/differential_check.py`: reads JSON
/// requests from CHECK_DIFF_IN and writes one JSON answer per line.
#[test]
#[ignore]
fn differential_driver() {
    let Ok(input) = std::env::var("CHECK_DIFF_IN") else {
        return;
    };
    let output = std::env::var("CHECK_DIFF_OUT").unwrap();
    let mut answers = Vec::new();
    for line in fs::read_to_string(input).unwrap().lines() {
        let request: serde_json::Value = serde_json::from_str(line).unwrap();
        let text = request["text"].as_str().unwrap_or("");
        let answer = match request["op"].as_str().unwrap() {
            "page" => {
                let page = parse_page(text);
                let mut ids: Vec<&String> = page.ids.iter().collect();
                ids.sort();
                serde_json::json!({"links": page.links, "ids": ids})
            }
            "unescape" => serde_json::json!({"text": unescape(text)}),
            "json" => match pyjson::loads(text) {
                Ok(_) => serde_json::json!({"ok": true}),
                Err(error) => serde_json::json!({"error": error.to_string()}),
            },
            "join" => match urljoin(request["base"].as_str().unwrap(), text) {
                Ok(joined) => match urlsplit(&joined, "") {
                    Ok(parts) => {
                        serde_json::json!({"url": joined, "parts": [parts.scheme, parts.netloc, parts.path, parts.query, parts.fragment], "unquoted": unquote(&parts.path)})
                    }
                    Err(error) => serde_json::json!({"error": error.to_string()}),
                },
                Err(error) => serde_json::json!({"error": error.to_string()}),
            },
            "validate" => {
                let site = PathBuf::from(request["site"].as_str().unwrap());
                let output = request["output"].as_str().map(PathBuf::from);
                match validate_site(&site, output.as_deref(), request["base"].as_str()) {
                    Ok(summary) => {
                        serde_json::json!({"pages": summary.pages, "links": summary.links, "articles": summary.articles})
                    }
                    Err(error) => serde_json::json!({"error": error.to_string()}),
                }
            }
            other => panic!("unknown op {other}"),
        };
        answers.push(answer.to_string());
    }
    fs::write(output, answers.join("\n") + "\n").unwrap();
}
