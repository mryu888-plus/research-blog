//! Ported from tests/test_init_site.py: independent starter creation must not
//! copy or overwrite private state. The starter is now embedded in blogctl,
//! so it carries no scripts/, crates/ or extensions/.

mod common;

use blog_site::init::{initialize, normalize_url, starter_files, InitSummary};
use common::{read, write, TempDir};
use std::fs;
use std::path::{Path, PathBuf};

struct Fixture {
    dir: TempDir,
    destination: PathBuf,
}

fn fixture() -> Fixture {
    let dir = TempDir::new("starter");
    let destination = dir.path().join("workbench");
    Fixture { dir, destination }
}

fn init(destination: &Path, name: &str, url: &str, author: &str) -> anyhow::Result<InitSummary> {
    initialize(destination, name, url, author, false)
}

fn config(destination: &Path) -> toml::Table {
    read(&destination.join("site/config.toml")).parse().unwrap()
}

fn frontmatter(path: &Path) -> toml::Table {
    let text = read(path);
    text.split("+++").nth(1).unwrap().parse().unwrap()
}

fn any_file(directory: &Path, predicate: &dyn Fn(&str) -> bool) -> bool {
    fs::read_dir(directory).unwrap().flatten().any(|entry| {
        let path = entry.path();
        if path.is_dir() {
            any_file(&path, predicate)
        } else {
            predicate(&entry.file_name().to_string_lossy())
        }
    })
}

#[test]
fn complete_starter_contains_shared_sources_and_only_anonymous_content() {
    let f = fixture();
    let result = init(
        &f.destination,
        "My Research Notes",
        "https://example.org/notes/",
        "An Author",
    )
    .unwrap();
    let config = config(&f.destination);
    assert_eq!(config["title"].as_str(), Some("My Research Notes"));
    assert_eq!(config["extra"]["author"].as_str(), Some("An Author"));
    assert_eq!(config["extra"]["profile_enabled"].as_bool(), Some(false));
    assert_eq!(
        config["base_url"].as_str(),
        Some("https://example.org/notes/")
    );
    assert_eq!(result.files, starter_files().len());
    assert!(result.files > 40, "only {} files", result.files);
    assert!(any_file(&f.destination, &|name| name == "ZEN.md"));
    for relative in [
        "ZEN.md",
        "AGENTS.md",
        "CLAUDE.md",
        "README.md",
        "LICENSE",
        ".github/workflows/pages.yml",
        ".vscode/tasks.json",
        "design/visual-system/typst/blog-diagram.typ",
        "site/templates/base.html",
        "site/static/zen.css",
        "site/content/posts/welcome/figures/style-example.typ",
    ] {
        assert!(f.destination.join(relative).is_file(), "{relative}");
    }
    for forbidden in [
        "profile",
        ".git",
        ".tools",
        "output",
        "target",
        "node_modules",
        "site/public",
        "site/data/profile.json",
        "site/static/resume.pdf",
        ".env",
        "site/content/posts/reusable-skills",
        "scripts",
        "crates",
        "extensions",
        "Cargo.toml",
    ] {
        assert!(!f.destination.join(forbidden).exists(), "{forbidden}");
    }
    assert!(!any_file(&f.destination, &|name| name.ends_with(".vsix")
        || name.ends_with(".pyc")));
    // The embedded allowlist never carries private or environment files.
    for (name, _) in starter_files() {
        assert!(
            !name.split('/').any(|part| part.starts_with(".env")),
            "{name}"
        );
        assert!(
            !name.starts_with("profile/") && !name.contains("resume"),
            "{name}"
        );
    }
    let references: serde_json::Value =
        serde_json::from_str(&read(&f.destination.join("site/data/references.json"))).unwrap();
    assert_eq!(references["items"][0]["citekey"], "zotero-guide");
    assert!(
        !read(&f.destination.join("site/content/about.md")).contains("template = \"about.html\"")
    );
}

#[test]
fn dry_run_validates_without_creating_destination() {
    let f = fixture();
    let result = initialize(
        &f.destination,
        "My Research Notes",
        "https://example.org",
        "Researcher",
        true,
    )
    .unwrap();
    assert_eq!(result.url, "https://example.org/");
    assert!(result.dry_run);
    assert!(!f.destination.exists());
}

/// Replaces test_generated_pages_workflow_uses_only_copied_build_entrypoints:
/// the workflow downloads the matching blogctl release instead.
#[test]
fn generated_pages_workflow_downloads_matching_blogctl() {
    let f = fixture();
    init(
        &f.destination,
        "Notes",
        "https://example.org/notes/",
        "Researcher",
    )
    .unwrap();
    let workflow = read(&f.destination.join(".github/workflows/pages.yml"));
    for expected in [
        format!("BLOGCTL_VERSION: '{}'", env!("CARGO_PKG_VERSION")).as_str(),
        "gh release download \"blogctl-v$BLOGCTL_VERSION\"",
        "sha256sum -c",
        "run: blogctl check",
        "fonts-noto-cjk",
        "actions/upload-pages-artifact@v3",
        "actions/deploy-pages@v4",
        "if: github.event_name != 'pull_request'",
    ] {
        assert!(workflow.contains(expected), "missing {expected}");
    }
    for forbidden in [
        "__BLOGCTL_VERSION__",
        "python",
        "setup-node",
        "node ",
        "npm ",
        "scripts/",
        "tests/",
    ] {
        assert!(!workflow.contains(forbidden), "unexpected {forbidden}");
    }
}

#[test]
fn special_pages_are_not_filtered_by_article_date_sorting() {
    let f = fixture();
    init(
        &f.destination,
        "Notes",
        "https://example.org/notes/",
        "Researcher",
    )
    .unwrap();
    let content = f.destination.join("site/content");
    assert_eq!(
        frontmatter(&content.join("_index.md"))["sort_by"].as_str(),
        Some("none"),
        "A date-sorted root ignores undated route pages in Zola"
    );
    assert_eq!(
        frontmatter(&content.join("posts/_index.md"))["sort_by"].as_str(),
        Some("date")
    );
    for (relative, expected) in [
        ("about.md", "about"),
        ("graph.md", "graph"),
        ("graph-data.md", "graph-data"),
        ("search.md", "search-index"),
        ("series.md", "series"),
        ("references.md", "references"),
    ] {
        let page = frontmatter(&content.join(relative));
        assert!(!page.contains_key("date"), "{relative}");
        let stem = relative.trim_end_matches(".md");
        assert_eq!(
            page.get("path").and_then(|p| p.as_str()).unwrap_or(stem),
            expected,
            "{relative}"
        );
    }
    for relative in ["search.md", "graph-data.md"] {
        let page = frontmatter(&content.join(relative));
        assert_eq!(page["in_search_index"].as_bool(), Some(false), "{relative}");
        assert_eq!(
            page["include_in_feeds"].as_bool(),
            Some(false),
            "{relative}"
        );
    }
}

#[test]
fn existing_empty_directory_can_be_initialized() {
    let f = fixture();
    fs::create_dir(&f.destination).unwrap();
    init(
        &f.destination,
        "Notes",
        "https://example.org/notes/",
        "Researcher",
    )
    .unwrap();
    assert!(f.destination.join("site/config.toml").is_file());
}

#[test]
fn existing_data_is_preserved() {
    let f = fixture();
    let existing = write(&f.destination, "notes.txt", "Keep this");
    let error = init(
        &f.destination,
        "Notes",
        "https://example.org/notes/",
        "Researcher",
    )
    .err()
    .expect("init should fail");
    assert!(error.to_string().contains("不会覆盖"), "{error}");
    assert_eq!(read(&existing), "Keep this");
    assert_eq!(fs::read_dir(&f.destination).unwrap().count(), 1);
}

#[test]
fn existing_file_is_not_replaced() {
    let f = fixture();
    fs::write(&f.destination, "Keep file").unwrap();
    assert!(init(
        &f.destination,
        "Notes",
        "https://example.org/notes/",
        "Researcher"
    )
    .is_err());
    assert_eq!(read(&f.destination), "Keep file");
}

#[test]
fn invalid_urls_are_rejected_without_writes() {
    let f = fixture();
    for url in [
        "javascript:alert(1)",
        "file:///tmp/site",
        "https://user:pass@example.org/",
        "https://example.org/?token=secret",
        "https://example.org/#fragment",
        "https://example.org\\path",
        "https://example.org/../private/",
        "https://example.org/%2e%2e/private/",
        "https://example.org/%252e%252e/private/",
        "https://example.org/%00",
        "https://example.org:99999/",
        "https://example .org/",
        "https://-example.org/",
        "https://example.org/\n",
    ] {
        assert!(
            init(&f.destination, "Notes", url, "Researcher").is_err(),
            "accepted {url:?}"
        );
    }
    assert!(!f.destination.exists());
}

#[test]
fn safe_urls_include_local_and_github_pages_roots() {
    for (value, expected) in [
        ("http://127.0.0.1:1111", "http://127.0.0.1:1111/"),
        ("http://[::1]:1111/notes", "http://[::1]:1111/notes/"),
        (
            "https://USERNAME.github.io/REPO",
            "https://USERNAME.github.io/REPO/",
        ),
    ] {
        assert_eq!(normalize_url(value).unwrap(), expected);
    }
}

#[test]
fn toml_values_are_escaped_without_reinterpolation() {
    let f = fixture();
    let name = "Notes \"quoted\" __SITE_URL__ \\ path";
    let author = "Name __SITE_TITLE__";
    init(&f.destination, name, "https://example.org/notes/", author).unwrap();
    let config = config(&f.destination);
    assert_eq!(config["title"].as_str(), Some(name));
    assert_eq!(config["extra"]["author"].as_str(), Some(author));
}

/// Covers test_windows_junction_attribute_is_detected and
/// test_linked_destination_parent_is_rejected with a real link: a junction
/// on Windows (no privilege needed), a symlink elsewhere.
#[test]
fn linked_destination_parent_is_rejected() {
    let f = fixture();
    let target = f.dir.path().join("real");
    fs::create_dir(&target).unwrap();
    let link = f.dir.path().join("linked");
    #[cfg(windows)]
    let linked = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&target)
        .output()
        .is_ok_and(|o| o.status.success());
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&target, &link).is_ok();
    if !linked {
        eprintln!("skipped: directory links are not available here");
        return;
    }
    let error = init(
        &link.join("new"),
        "Notes",
        "https://example.org/notes/",
        "Researcher",
    )
    .err()
    .expect("init should fail");
    assert!(error.to_string().contains("junction"), "{error}");
    assert!(!target.join("new").exists());
    #[cfg(windows)]
    {
        let _ = fs::remove_dir(&link);
    }
    let _ = f.destination;
}
