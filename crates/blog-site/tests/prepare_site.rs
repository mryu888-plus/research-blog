//! Ported from tests/test_prepare_site.py: staging must protect formulas
//! without changing the author's source tree.
//!
//! The Python tests replaced protect_math with a marker; here the real
//! transform runs and the expected output is computed with it.

mod common;

use blog_site::math::protect_math;
use blog_site::paths::RelPath;
use blog_site::prepare::{PreparedSite, ORIGINAL_BODIES};
use common::{read, write, TempDir};
use std::fs;
use std::path::{Path, PathBuf};

struct Fixture {
    _dir: TempDir,
    root: PathBuf,
    source: PathBuf,
    target: PathBuf,
}

fn fixture() -> Fixture {
    let dir = TempDir::new("prepared");
    let root = dir.path().to_path_buf();
    let source = root.join("site");
    write(
        &source,
        "config.toml",
        "base_url = \"https://example.org\"\n",
    );
    Fixture {
        target: root.join("staged"),
        source,
        root,
        _dir: dir,
    }
}

fn paths(values: &[&str]) -> Vec<RelPath> {
    values.iter().map(|v| RelPath::new(v)).collect()
}

fn mtime(path: &Path) -> std::time::SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

#[test]
fn sources_untouched_and_only_content_markdown_transformed() {
    let f = fixture();
    let originals = [
        ("content/posts/a/index.md", "$x_i$\n"),
        ("content/posts/a/asset.svg", "<svg/>"),
        ("content/posts/a/data.txt", "$x_i$"),
        ("templates/example.md", "$x_i$"),
        ("static/example.md", "$x_i$"),
        ("sass/main.scss", "body {}"),
        ("data/profile.json", "{\"name\":\"User\"}"),
        ("themes/example/templates/index.html", "<main/>"),
    ];
    for (relative, text) in originals {
        write(&f.source, relative, text);
    }
    let mut stage = PreparedSite::new(&f.source, &f.target).unwrap();
    let changed = stage.sync().unwrap();
    assert!(changed.contains(&RelPath::new("data/profile.json")));
    for (relative, text) in originals {
        assert_eq!(
            read(&f.source.join(relative)),
            text,
            "source changed: {relative}"
        );
        let expected = if relative.ends_with("index.md") {
            protect_math(text)
        } else {
            text.to_string()
        };
        assert_ne!(
            relative.ends_with("index.md"),
            expected == text,
            "{relative}"
        );
        assert_eq!(read(&f.target.join(relative)), expected, "{relative}");
    }
}

#[test]
fn incremental_noop_edit_add_and_delete() {
    let f = fixture();
    let article = write(&f.source, "content/posts/a/index.md", "$x_i$");
    write(&f.source, "data/profile.json", "{\"name\":\"A\"}");
    let mut stage = PreparedSite::new(&f.source, &f.target).unwrap();
    stage.sync().unwrap();
    let output = f.target.join("content/posts/a/index.md");
    let before = mtime(&output);
    assert_eq!(stage.sync().unwrap(), Vec::<RelPath>::new());
    assert_eq!(mtime(&output), before);
    fs::write(&article, "$x_{updated}$").unwrap();
    write(&f.source, "static/new.txt", "added");
    write(&f.source, "data/profile.json", "{\"name\":\"Updated\"}");
    assert_eq!(
        stage.sync().unwrap(),
        paths(&[
            "content/posts/a/index.md",
            ORIGINAL_BODIES,
            "data/profile.json",
            "static/new.txt"
        ])
    );
    // Generated output is not owned by the mirror.
    let generated = write(&f.target, "public/keep.html", "generated");
    fs::remove_file(&article).unwrap();
    fs::remove_dir(article.parent().unwrap()).unwrap();
    assert_eq!(
        stage.sync().unwrap(),
        paths(&["content/posts/a/index.md", ORIGINAL_BODIES])
    );
    assert!(!output.exists());
    assert!(!output.parent().unwrap().exists());
    assert!(generated.exists());
}

#[test]
fn output_manifests_and_unapproved_root_files_are_excluded() {
    let f = fixture();
    for relative in [
        "public/index.html",
        ".git/config",
        "secret.env",
        "article-manifest.json",
        "content/posts/a/article-manifest.json",
        "static/article-manifest.json",
        "themes/example/.git/config",
        "content/public/old.html",
    ] {
        write(&f.source, relative, "local secret");
    }
    let mut stage = PreparedSite::new(&f.source, &f.target).unwrap();
    assert_eq!(
        stage.sync().unwrap(),
        paths(&["config.toml", ORIGINAL_BODIES])
    );
    let mut files = Vec::new();
    collect_files(&f.target, &f.target, &mut files);
    files.sort();
    assert_eq!(files, ["config.toml", ORIGINAL_BODIES]);
    let bodies: serde_json::Value =
        serde_json::from_str(&read(&f.target.join(ORIGINAL_BODIES))).unwrap();
    assert_eq!(bodies, serde_json::json!({}));
}

fn collect_files(base: &Path, directory: &Path, files: &mut Vec<String>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_files(base, &path, files);
        } else {
            files.push(
                path.strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

#[test]
fn reading_data_generated_without_placeholder_and_tracks_edits_and_deletes() {
    let f = fixture();
    let original = "+++\ntitle = \"Fresh note\"\n+++\n\nRaw $x_i$ and 汉字.\n";
    let article = write(&f.source, "content/posts/fresh/index.md", original);
    assert!(!f.source.join("data").exists());
    let mut stage = PreparedSite::new(&f.source, &f.target).unwrap();
    assert!(stage
        .sync()
        .unwrap()
        .contains(&RelPath::new(ORIGINAL_BODIES)));
    let data = f.target.join(ORIGINAL_BODIES);
    let bodies = || serde_json::from_str::<serde_json::Value>(&read(&data)).unwrap();
    assert_eq!(
        bodies(),
        serde_json::json!({"posts/fresh/index.md": "\nRaw $x_i$ and 汉字.\n"})
    );
    assert_eq!(read(&article), original);
    assert!(!f.source.join("data").exists());
    let before = mtime(&data);
    assert_eq!(stage.sync().unwrap(), Vec::<RelPath>::new());
    assert_eq!(mtime(&data), before);
    fs::write(&article, "+++\ntitle = \"Fresh note\"\n+++\nChanged $y$\n").unwrap();
    assert!(stage
        .sync()
        .unwrap()
        .contains(&RelPath::new(ORIGINAL_BODIES)));
    assert_eq!(
        bodies(),
        serde_json::json!({"posts/fresh/index.md": "Changed $y$\n"})
    );
    fs::remove_file(&article).unwrap();
    assert!(stage
        .sync()
        .unwrap()
        .contains(&RelPath::new(ORIGINAL_BODIES)));
    assert_eq!(bodies(), serde_json::json!({}));
    assert!(f.target.join("data").is_dir());
    assert_eq!(stage.sync().unwrap(), Vec::<RelPath>::new());
}

#[test]
fn overlapping_and_nonempty_targets_are_rejected() {
    let f = fixture();
    for target in [f.source.clone(), f.source.join("stage"), f.root.clone()] {
        assert!(
            PreparedSite::new(&f.source, &target).is_err(),
            "accepted {}",
            target.display()
        );
    }
    let sentinel = write(&f.target, "keep.txt", "keep");
    assert!(PreparedSite::new(&f.source, &f.target).is_err());
    assert_eq!(read(&sentinel), "keep");
}

#[test]
fn file_directory_transitions() {
    let f = fixture();
    let path = write(&f.source, "static/item", "first file");
    let mut stage = PreparedSite::new(&f.source, &f.target).unwrap();
    stage.sync().unwrap();
    fs::remove_file(&path).unwrap();
    let child = write(&f.source, "static/item/child.txt", "child");
    stage.sync().unwrap();
    assert!(f.target.join("static/item/child.txt").is_file());
    fs::remove_file(&child).unwrap();
    fs::remove_dir(&path).unwrap();
    write(&f.source, "static/item", "file again");
    stage.sync().unwrap();
    assert_eq!(read(&f.target.join("static/item")), "file again");
}

#[test]
fn unowned_staging_collision_is_not_overwritten() {
    let f = fixture();
    let mut stage = PreparedSite::new(&f.source, &f.target).unwrap();
    stage.sync().unwrap();
    let unowned = write(&f.target, "static/new.txt", "keep");
    write(&f.source, "static/new.txt", "replacement");
    assert!(stage.sync().is_err());
    assert_eq!(read(&unowned), "keep");
}

/// Python made protect_math raise. The Rust transform cannot fail, so an
/// undecodable article stands in for the bad input: the sync fails, and
/// after the fix the next sync stages the protected article.
#[test]
fn sync_recovers_after_a_transform_error() {
    let f = fixture();
    let article = f.source.join("content/article.md");
    fs::create_dir_all(article.parent().unwrap()).unwrap();
    fs::write(&article, b"\xff$x_i$").unwrap();
    let mut stage = PreparedSite::new(&f.source, &f.target).unwrap();
    assert!(stage.sync().is_err());
    fs::write(&article, "$x_i$").unwrap();
    assert!(stage
        .sync()
        .unwrap()
        .contains(&RelPath::new("content/article.md")));
    assert_eq!(
        read(&f.target.join("content/article.md")),
        protect_math("$x_i$")
    );
    assert_eq!(stage.sync().unwrap(), Vec::<RelPath>::new());
}

/// A directory link: a junction on Windows (no privilege needed), a symlink elsewhere.
fn link_directory(target: &Path, link: &Path) -> bool {
    #[cfg(windows)]
    {
        std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .is_ok_and(|o| o.status.success())
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
}

#[test]
fn windows_reparse_points_are_rejected() {
    let f = fixture();
    let outside = f.root.join("outside");
    write(&outside, "secret.md", "secret");
    if !link_directory(&outside, &f.source.join("content")) {
        eprintln!("skipped: directory links are not available here");
        return;
    }
    assert!(blog_site::paths::reject_link(&f.source.join("content")).is_err());
    assert!(
        blog_site::paths::reject_link_parents(&f.source.join("content").join("secret.md")).is_err()
    );
}

#[test]
fn source_symlink_is_rejected_before_copying() {
    let f = fixture();
    let outside = f.root.join("outside");
    write(&outside, "secret.md", "secret");
    if !link_directory(&outside, &f.source.join("content")) {
        eprintln!("skipped: directory links are not available here");
        return;
    }
    let mut stage = PreparedSite::new(&f.source, &f.target).unwrap();
    assert!(stage.sync().is_err());
    assert!(fs::read_dir(&f.target).unwrap().next().is_none());
}

/// Python linked the staged file itself. File symlinks need extra privilege
/// on Windows, so there the staged directory becomes a junction instead.
#[test]
fn target_symlink_is_rejected_before_deletion() {
    let f = fixture();
    let article = write(&f.source, "content/article.md", "article");
    let mut stage = PreparedSite::new(&f.source, &f.target).unwrap();
    stage.sync().unwrap();
    let outside = f.root.join("outside");
    let kept = write(&outside, "article.md", "keep");
    let staged = f.target.join("content");
    fs::remove_dir_all(&staged).unwrap();
    if !link_directory(&outside, &staged) {
        eprintln!("skipped: directory links are not available here");
        return;
    }
    fs::remove_file(&article).unwrap();
    assert!(stage.sync().is_err());
    assert_eq!(read(&kept), "keep");
    #[cfg(windows)]
    {
        let _ = fs::remove_dir(&staged); // remove the junction, not its target
    }
}
