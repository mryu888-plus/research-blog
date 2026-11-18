//! Embed the files a new site starts from, so `blogctl init` needs no checkout.
//!
//! Text files are stored with LF line endings whatever the checkout uses.

use std::fs;
use std::path::PathBuf;

/// Shared components copied from the repository.
const SHARED_FILES: &[&str] = &[
    "ZEN.md",
    "design/visual-system/typst/blog-diagram.typ",
    "LICENSE",
    "CONTENT_LICENSE.md",
    "THIRD_PARTY_NOTICES.md",
];
const SHARED_DIRECTORIES: &[&str] = &[
    "site/templates",
    "site/templates/tags",
    "site/templates/shortcodes",
];
const STATIC_FILES: &[&str] = &[
    "zen.css",
    "style.css",
    "apollo.css",
    "main.js",
    "preview.js",
    "graph.css",
    "graph.js",
    "knowledge-core.js",
    "knowledge.js",
    "topics.css",
    "topics.js",
    "series.css",
    "series-editor.js",
    "references.css",
    "references-page.js",
    "about.css",
    "about.js",
    "favicon.svg",
];
/// The public starter: content and settings for a new site.
const STARTER_FILES: &[&str] = &[
    "README.md",
    "AGENTS.md",
    "CLAUDE.md",
    ".gitignore",
    ".vscode/settings.json",
    ".vscode/extensions.json",
    ".vscode/tasks.json",
    ".github/workflows/pages.yml",
    "site/config.toml",
    "site/content/_index.md",
    "site/content/posts/_index.md",
    "site/content/about.md",
    "site/content/graph.md",
    "site/content/graph-data.md",
    "site/content/search.md",
    "site/content/series.md",
    "site/content/references.md",
    "site/data/collections.json",
    "site/data/references.json",
    "site/content/posts/welcome/index.md",
    "site/content/posts/welcome/figures/style-example.typ",
    "site/content/posts/graph-and-math/index.md",
    "site/content/posts/graph-and-math/figures/workflow-pipeline.typ",
    "site/content/posts/style-notes/index.md",
    "site/content/posts/draft-note/index.md",
];

fn main() {
    let repo = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let mut entries: Vec<(String, PathBuf)> = Vec::new();
    for file in SHARED_FILES {
        entries.push((file.to_string(), repo.join(file)));
    }
    for directory in SHARED_DIRECTORIES {
        let path = repo.join(directory);
        println!("cargo:rerun-if-changed={}", path.display());
        // Git does not keep empty directories, so a missing one is empty.
        let Ok(listing) = fs::read_dir(&path) else {
            continue;
        };
        let mut names: Vec<String> = listing
            .flatten()
            .filter(|e| e.path().is_file() && e.path().extension().is_some_and(|x| x == "html"))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        for name in names {
            entries.push((format!("{directory}/{name}"), path.join(&name)));
        }
    }
    for name in STATIC_FILES {
        entries.push((
            format!("site/static/{name}"),
            repo.join("site/static").join(name),
        ));
    }
    for file in STARTER_FILES {
        entries.push((file.to_string(), repo.join("examples/starter").join(file)));
    }
    let mut source = String::from("pub static FILES: &[(&str, &[u8])] = &[\n");
    for (index, (published, path)) in entries.iter().enumerate() {
        println!("cargo:rerun-if-changed={}", path.display());
        let data =
            fs::read(path).unwrap_or_else(|_| panic!("missing starter file {}", path.display()));
        let data = normalize(data);
        let stored = out.join(format!("asset-{index}"));
        fs::write(&stored, data).unwrap();
        source.push_str(&format!(
            "    ({published:?}, include_bytes!({:?})),\n",
            stored.display().to_string()
        ));
    }
    source.push_str("];\n");
    fs::write(out.join("starter_files.rs"), source).unwrap();
}

fn normalize(data: Vec<u8>) -> Vec<u8> {
    if data.contains(&0) || std::str::from_utf8(&data).is_err() {
        return data;
    }
    String::from_utf8(data)
        .unwrap()
        .replace("\r\n", "\n")
        .into_bytes()
}
