//! Live preview: Zola serve on the prepared copy, Typst watchers for figures,
//! the local series editor, and re-preparation whenever a source changes.

use crate::builder::{typst_figure, ScratchDir};
use crate::editor_server::EditorServer;
use crate::paths::absolute;
use crate::prepare::PreparedSite;
use crate::{profile, site, tools, zen};
use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

const POLL: Duration = Duration::from_millis(100);
const PROFILE_SETTLE: Duration = Duration::from_millis(400);

fn free_port() -> Result<u16> {
    for port in 1111..=1120 {
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return Ok(port);
        }
    }
    bail!("本地预览端口 1111–1120 均被占用，请停止之前的预览任务。")
}

fn modified(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

type Snapshot = BTreeMap<String, (SystemTime, u64)>;

/// Watch imports and assets as well as the main Typst source.
fn profile_snapshot(root: &Path) -> Snapshot {
    fn visit(folder: &Path, relative: &str, snapshot: &mut Snapshot) {
        let Ok(entries) = fs::read_dir(folder) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if [".git", "__pycache__", ".cache", "output", "build"].contains(&name.as_str()) {
                continue;
            }
            let child = if relative.is_empty() {
                name.clone()
            } else {
                format!("{relative}/{name}")
            };
            // Tinymist may emit this beside cv.typ; our outputs live outside profile/.
            if child == "cv.pdf" {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if metadata.is_dir() {
                visit(&entry.path(), &child, snapshot);
            } else if metadata.is_file() {
                if let Ok(time) = metadata.modified() {
                    snapshot.insert(child, (time, metadata.len()));
                }
            }
        }
    }
    let mut snapshot = Snapshot::new();
    visit(&root.join("profile"), "", &mut snapshot);
    snapshot
}

/// Prompt a Zola rebuild after load_data changes without creating a page.
fn trigger_content_reload(site: &Path) {
    let content = site.join("content");
    let mut entry = content.join("_index.md");
    if !entry.is_file() {
        match first_markdown(&content) {
            Some(found) => entry = found,
            None => return,
        }
    }
    if let Ok(file) = fs::OpenOptions::new().write(true).open(&entry) {
        let _ = file.set_modified(SystemTime::now());
    }
}

fn first_markdown(directory: &Path) -> Option<PathBuf> {
    for entry in fs::read_dir(directory).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = first_markdown(&path) {
                return Some(found);
            }
        } else if path.extension().is_some_and(|e| e == "md") {
            return Some(path);
        }
    }
    None
}

/// Every `figures/*.typ` below the posts, drafts and unreferenced ones too,
/// with the SVG beside the article.
fn figure_sources(posts: &Path) -> HashMap<PathBuf, PathBuf> {
    fn visit(directory: &Path, sources: &mut HashMap<PathBuf, PathBuf>) {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if entry.file_name() == "figures" {
                for figure in fs::read_dir(&path).into_iter().flatten().flatten() {
                    let source = figure.path();
                    if source.is_file() && source.extension().is_some_and(|e| e == "typ") {
                        let stem = source.file_stem().unwrap().to_string_lossy().into_owned();
                        sources.insert(source, directory.join(format!("{stem}.svg")));
                    }
                }
            }
            visit(&path, sources);
        }
    }
    let mut sources = HashMap::new();
    visit(posts, &mut sources);
    sources
}

struct Children {
    watchers: HashMap<PathBuf, Child>,
    server: Option<Child>,
    editor: Option<EditorServer>,
}

impl Children {
    fn stop(&mut self) {
        let mut children: Vec<Child> = self.watchers.drain().map(|(_, child)| child).collect();
        children.extend(self.server.take());
        for child in &mut children {
            let _ = child.kill();
        }
        for child in &mut children {
            let _ = child.wait();
        }
        if let Some(editor) = self.editor.take() {
            editor.stop();
        }
    }
}

impl Drop for Children {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn preview(root: &Path, open_browser: bool) -> Result<()> {
    let root = absolute(root)?;
    let site = root.join("site");
    let mut enabled = site::profile_enabled(&site::read_config(&site)?)?;
    if enabled {
        site::require_profile_source(&root)?;
    }
    zen::sync(&root, false)?;
    let mut previous_zen = modified(&root.join("ZEN.md"));
    let port = free_port()?;
    let typst = tools::tool_path("typst", &root)?;
    let zola = tools::tool_path("zola", &root)?;

    crate::lifetime::kill_children_on_exit();
    let interrupted = Arc::new(AtomicBool::new(false));
    {
        let flag = interrupted.clone();
        // Ctrl+C also reaches Zola and Typst; we only need to stop waiting.
        let _ = ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst));
    }

    let refresh_profile = |root: &Path| -> bool {
        let result =
            site::require_profile_source(root).and_then(|_| profile::build_profile(root, true));
        match result {
            Ok(_) => {
                // Zola watches content, but may not watch files read by load_data.
                // Updating only mtime triggers a rebuild without rewriting the page.
                trigger_content_reload(&root.join("site"));
                println!("个人介绍与简历已更新。");
                true
            }
            Err(error) => {
                eprintln!("个人资料编译失败，保留上一版产物：{error:#}");
                eprintln!("修正 profile/ 中的文件并保存后会自动重试。");
                false
            }
        }
    };

    let mut children = Children {
        watchers: HashMap::new(),
        server: None,
        editor: None,
    };
    let sync_figures = |children: &mut Children| -> Result<()> {
        let sources = figure_sources(&site.join("content").join("posts"));
        let stale: Vec<PathBuf> = children
            .watchers
            .keys()
            .filter(|s| !sources.contains_key(*s))
            .cloned()
            .collect();
        for source in stale {
            if let Some(mut child) = children.watchers.remove(&source) {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        for (source, output) in sources {
            if children.watchers.contains_key(&source) {
                continue;
            }
            // Finish the first SVG before Zola renders its corresponding page.
            let _ = typst_figure(&typst, &root, &source, &output, false).status();
            let mut watch = typst_figure(&typst, &root, &source, &output, true);
            let child = crate::lifetime::tie_to_parent(&mut watch)
                .spawn()
                .with_context(|| format!("无法启动 Typst 监视：{}", source.display()))?;
            children.watchers.insert(source, child);
        }
        Ok(())
    };

    let initial_profile = if enabled {
        refresh_profile(&root)
    } else {
        true
    };
    if !initial_profile && !site.join("data/profile.json").exists() {
        bail!("首次生成个人资料失败，请修正上面的错误后重新运行预览。");
    }
    let mut previous_profile = profile_snapshot(&root);
    let mut profile_changed_at: Option<Instant> = None;
    sync_figures(&mut children)?;
    println!("网页实时预览：http://127.0.0.1:{port}/ （含草稿）");
    println!("保存 Markdown 或 Typst 后自动编译、刷新网页。Ctrl+C 停止。");
    let preview_output = root.join(".tools").join("site-preview");
    let scratch = ScratchDir::new(&root.join(".tools"), "math-preview-")?;
    let mut prepared = PreparedSite::new(&site, &scratch.path().join("site"))?;
    prepared.sync()?;
    let editor = EditorServer::start(&site.join("content"), &format!("http://127.0.0.1:{port}"))?;
    // Session credentials exist only in the private serve input, never in
    // site/static or the public build. PreparedSite leaves unmanaged files alone.
    fs::create_dir_all(prepared.target.join("static"))?;
    fs::write(
        prepared.target.join("static").join("preview-editor.json"),
        editor.config_json(),
    )?;
    children.editor = Some(editor);
    let mut command = Command::new(&zola);
    command
        .arg("--root")
        .arg(&prepared.target)
        .arg("serve")
        .arg("--drafts")
        .arg("--output-dir")
        .arg(&preview_output)
        .arg("--force")
        .arg("--interface")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .arg("--debounce")
        .arg("1")
        .current_dir(&root);
    if open_browser {
        command.arg("--open");
    }
    crate::lifetime::tie_to_parent(&mut command);
    children.server = Some(command.spawn().context("无法启动 Zola 预览")?);
    let mut last_sync_error: Option<String> = None;
    let status = loop {
        if interrupted.load(Ordering::SeqCst) {
            break None;
        }
        if let Some(status) = children.server.as_mut().unwrap().try_wait()? {
            break Some(status);
        }
        let current_zen = modified(&root.join("ZEN.md"));
        if current_zen != previous_zen {
            previous_zen = current_zen;
            if let Err(error) = zen::sync(&root, false) {
                eprintln!("ZEN 更新失败，请修正 ZEN.md 后保存重试：{error:#}");
            }
        }
        if let Err(error) = sync_figures(&mut children) {
            eprintln!("图形监视更新失败：{error:#}");
        }
        // A config edit may briefly be incomplete; PreparedSite reports
        // the render error while we retain the previous feature state.
        let current_enabled = site::read_config(&site)
            .and_then(|c| site::profile_enabled(&c))
            .unwrap_or(enabled);
        if current_enabled && !enabled {
            refresh_profile(&root);
        }
        enabled = current_enabled;
        let current_profile = profile_snapshot(&root);
        if current_profile != previous_profile {
            previous_profile = current_profile;
            profile_changed_at = Some(Instant::now());
        } else if profile_changed_at.is_some_and(|at| at.elapsed() >= PROFILE_SETTLE) {
            if enabled {
                refresh_profile(&root);
            }
            profile_changed_at = None;
        }
        match prepared.sync() {
            Ok(changed) => {
                // load_data inputs may not be watched by Zola itself.
                if changed.iter().any(|path| path.first() == "data") {
                    trigger_content_reload(&prepared.target);
                }
                last_sync_error = None;
            }
            Err(error) => {
                // An editor can atomically replace a file during our scan.
                // Retry next tick, retaining the last valid preview meanwhile.
                let message = format!("{error:#}");
                if last_sync_error.as_deref() != Some(&message) {
                    eprintln!("预览更新暂缓：{message}");
                    last_sync_error = Some(message);
                }
            }
        }
        std::thread::sleep(POLL);
    };
    children.stop();
    drop(prepared);
    drop(scratch);
    if let Some(status) = status {
        if !status.success() && !interrupted.load(Ordering::SeqCst) {
            bail!(
                "网页预览退出，错误码 {}，请查看上面的编译错误。",
                status.code().unwrap_or(-1)
            );
        }
    }
    Ok(())
}
