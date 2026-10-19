//! Ported from tests/test_reading_time.py: match pinned Zola reading times
//! without exposing private original bodies.

mod common;

use blog_site::markdown::markdown_body;
use blog_site::paths::RelPath;
use blog_site::prepare::{PreparedSite, ORIGINAL_BODIES};
use common::{read, repo_root, write, zola_build, TempDir};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn frontmatter_is_removed_without_normalizing_body() {
    for delimiter in ["+++", "---"] {
        let text = format!("\u{feff}{delimiter}\r\nkey = \"value\"\r\n{delimiter}\r\n\r\n$x$\r\n");
        assert_eq!(markdown_body(&text), "\r\n$x$\r\n");
    }
    assert_eq!(markdown_body("$x$\n"), "$x$\n");
    assert_eq!(markdown_body("+++\nunclosed"), "+++\nunclosed");
}

struct Site {
    _dir: TempDir,
    source: PathBuf,
    prepared: PreparedSite,
}

fn site() -> Site {
    let dir = TempDir::new("reading");
    let source = dir.path().join("source");
    write(
        &source,
        "config.toml",
        "base_url = \"https://example.test/blog/\"\ndefault_language = \"zh\"\n",
    );
    write(&source, "content/_index.md", "+++\n+++\n");
    write(&source, "templates/index.html", "index");
    write(
        &source,
        "templates/page.html",
        "{% set reading_sources = load_data(path=\"data/original-bodies.json\") %}{% set reading_page = page %}minutes={% include \"reading-time.html\" %};native={{ page.reading_time }};prepared={{ page.relative_path in reading_sources }};",
    );
    fs::copy(
        repo_root().join("site/templates/reading-time.html"),
        source.join("templates/reading-time.html"),
    )
    .unwrap();
    write(&source, ORIGINAL_BODIES, "{}");
    let prepared = PreparedSite::new(&source, &dir.path().join("prepared")).unwrap();
    Site {
        source,
        prepared,
        _dir: dir,
    }
}

fn article(site: &Path, name: &str, body: &str) -> PathBuf {
    let title = "FrontMatter".repeat(100);
    write(
        site,
        &format!("content/{name}.md"),
        &format!("+++\ntitle = \"{title}\"\n+++\n{body}"),
    )
}

/// (reading minutes, native Zola minutes, prepared flag) per page directory.
fn build(site: &Path) -> HashMap<String, (u32, u32, bool)> {
    let public = zola_build(site, false);
    let pattern = Regex::new(r"minutes=(\d+);native=(\d+);prepared=(true|false);").unwrap();
    let mut values = HashMap::new();
    for entry in fs::read_dir(&public).unwrap() {
        let index = entry.unwrap().path().join("index.html");
        if let Ok(html) = fs::read_to_string(&index) {
            if let Some(found) = pattern.captures(&html) {
                let name = index
                    .parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                values.insert(
                    name,
                    (
                        found[1].parse().unwrap(),
                        found[2].parse().unwrap(),
                        &found[3] == "true",
                    ),
                );
            }
        }
    }
    values
}

#[test]
fn prepared_zh_matches_native_original_for_unicode_and_fences() {
    let mut s = site();
    let bodies = [
        ("empty", String::new()),
        ("boundary", "汉".repeat(439)),
        // U+0345 is Unicode Alphabetic but Python str.isalpha returns false.
        ("combining", "\u{345}".repeat(440)),
        (
            "math",
            "$$\\forall x_i \\in S: f(x_i) = y_i$$\n\n".repeat(30),
        ),
        (
            "fences",
            "汉".repeat(439) + "\n```python\n" + &"a".repeat(1000) + "\n```\n",
        ),
        ("unclosed", "汉".repeat(440) + "\n```" + &"a".repeat(1000)),
        (
            "four-backticks",
            "汉".repeat(438) + "\n````\n" + &"a".repeat(1000) + "\n````\nxy",
        ),
    ];
    let originals: Vec<Vec<u8>> = bodies
        .iter()
        .map(|(name, body)| fs::read(article(&s.source, name, body)).unwrap())
        .collect();
    let baseline = build(&s.source);
    s.prepared.sync().unwrap();
    let rendered = build(&s.prepared.target);
    for ((name, _), original) in bodies.iter().zip(&originals) {
        assert_eq!(rendered[*name].0, baseline[*name].1, "{name}");
        assert!(!baseline[*name].2, "{name}");
        assert!(rendered[*name].2, "{name}");
        assert_eq!(
            &fs::read(s.source.join(format!("content/{name}.md"))).unwrap(),
            original
        );
    }
    assert_eq!(rendered["combining"].0, 2);
    assert!(rendered["math"].1 > rendered["math"].0);
    assert!(!s
        .prepared
        .target
        .join("public")
        .join(ORIGINAL_BODIES)
        .exists());
    assert_eq!(read(&s.source.join(ORIGINAL_BODIES)), "{}");
    let sources: serde_json::Value =
        serde_json::from_str(&read(&s.prepared.target.join(ORIGINAL_BODIES))).unwrap();
    assert_eq!(sources["math.md"], bodies[3].1);
}

#[test]
fn private_source_data_updates_and_removes_deleted_pages() {
    let mut s = site();
    let path = article(&s.source, "math", "$x$");
    s.prepared.sync().unwrap();
    let data = s.prepared.target.join(ORIGINAL_BODIES);
    let before = fs::metadata(&data).unwrap().modified().unwrap();
    assert_eq!(s.prepared.sync().unwrap(), Vec::<RelPath>::new());
    assert_eq!(fs::metadata(&data).unwrap().modified().unwrap(), before);
    article(&s.source, "math", "$updated$");
    assert!(s
        .prepared
        .sync()
        .unwrap()
        .contains(&RelPath::new(ORIGINAL_BODIES)));
    let bodies = || serde_json::from_str::<serde_json::Value>(&read(&data)).unwrap();
    assert_eq!(bodies()["math.md"], "$updated$");
    fs::remove_file(path).unwrap();
    assert!(s
        .prepared
        .sync()
        .unwrap()
        .contains(&RelPath::new(ORIGINAL_BODIES)));
    assert!(bodies().get("math.md").is_none());
}

#[test]
fn fresh_site_without_reading_placeholder_renders_real_zola() {
    let mut s = site();
    fs::remove_file(s.source.join(ORIGINAL_BODIES)).unwrap();
    fs::remove_dir(s.source.join("data")).unwrap();
    let path = article(&s.source, "fresh", &("汉".repeat(440) + "\n\n$x_i$"));
    let original = fs::read(&path).unwrap();
    s.prepared.sync().unwrap();
    let rendered = build(&s.prepared.target);
    assert_eq!(rendered["fresh"].0, 2);
    assert!(rendered["fresh"].2);
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(!s.source.join("data").exists());
    assert!(!find_named(
        &s.prepared.target.join("public"),
        "original-bodies.json"
    ));
}

fn find_named(directory: &Path, name: &str) -> bool {
    fs::read_dir(directory).unwrap().flatten().any(|entry| {
        let path = entry.path();
        if path.is_dir() {
            find_named(&path, name)
        } else {
            entry.file_name() == name
        }
    })
}

#[test]
fn other_languages_keep_native_fallback() {
    let mut s = site();
    write(
        &s.source,
        "config.toml",
        "base_url = \"https://example.test\"\ndefault_language = \"en\"\n",
    );
    article(&s.source, "math", &"$x$ ".repeat(50));
    s.prepared.sync().unwrap();
    let (reading, native, prepared) = build(&s.prepared.target)["math"];
    assert_eq!(reading, native);
    assert!(prepared);
}
