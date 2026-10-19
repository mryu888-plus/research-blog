//! Series and groups in `site/data/collections.json`, as the VS Code
//! extension's collections-core.cjs manages them. Files written here are
//! byte-identical to the extension's, so both can edit the same catalog.

use super::js::{self, JsObject, JsValue};
use anyhow::{anyhow, bail, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

/// Raised when the catalog changed since the caller read its revision.
#[derive(Debug)]
pub struct Conflict;

impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("系列目录已被其他窗口修改，请刷新后重试。")
    }
}

impl std::error::Error for Conflict {}

/// Node's `path.resolve(base, ...parts)`: absolute and lexically normalized.
fn resolve(base: &Path, parts: &[&str]) -> Result<PathBuf> {
    let mut path = std::path::absolute(base)?;
    for part in parts {
        let piece = Path::new(part);
        if piece.is_absolute() {
            path = piece.to_path_buf();
        } else {
            path.push(piece);
        }
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    Ok(normalized)
}

fn fold(component: Component) -> String {
    let text = component.as_os_str().to_string_lossy();
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text.into_owned()
    }
}

/// Components of `target` below `root`, or `None` when it is outside.
fn below<'a>(root: &Path, target: &'a Path) -> Option<Vec<Component<'a>>> {
    let mut rest = target.components();
    for component in root.components() {
        if fold(component) != rest.next().map(fold)? {
            return None;
        }
    }
    Some(rest.collect())
}

pub fn inside(root: &Path, target: &Path) -> Result<bool> {
    Ok(below(&resolve(root, &[])?, &resolve(target, &[])?).is_some())
}

/// A path inside `root` that passes through no symlink or junction.
pub fn safe_path(root: &Path, parts: &[&str]) -> Result<PathBuf> {
    let root = resolve(root, &[])?;
    let target = resolve(&root, parts)?;
    let Some(components) = below(&root, &target) else {
        bail!("路径必须位于博客目录内。");
    };
    let mut current = root.clone();
    for step in std::iter::once(None).chain(components.into_iter().map(Some)) {
        if let Some(component) = step {
            current.push(component);
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!("博客写作目录不能经过符号链接或目录联接。")
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(target)
}

fn js_space(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}'
}

fn js_trim(value: &str) -> &str {
    value.trim_matches(js_space)
}

fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

pub fn title_text(value: Option<&JsValue>) -> Result<String> {
    let valid = value.and_then(JsValue::as_str).filter(|text| {
        let trimmed = js_trim(text);
        !trimmed.is_empty() && utf16_len(trimmed) <= 120 && !text.chars().any(|c| (c as u32) < 0x20)
    });
    match valid {
        Some(text) => Ok(js_trim(text).to_string()),
        None => bail!("请输入 1–120 个字符的标题。"),
    }
}

pub fn article_key(value: Option<&JsValue>) -> Result<&str> {
    let valid = value.and_then(JsValue::as_str).filter(|key| {
        let shape = key
            .strip_prefix("posts/")
            .and_then(|rest| rest.strip_suffix("/index.md"))
            .is_some_and(|name| !name.is_empty() && !name.contains(['/', '\\']));
        shape
            && !key.split('/').any(|part| part == "." || part == "..")
            && !key.chars().any(|c| c == ':' || (c as u32) < 0x20)
    });
    valid.ok_or_else(|| anyhow!("文章必须是 posts/文章目录/index.md。"))
}

fn valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    (2..=80).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes[1..]
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

pub fn validate_catalog(data: &JsValue) -> Result<()> {
    let series_list = data
        .prop("series")
        .and_then(JsValue::as_array)
        .filter(|_| data.truthy() && data.prop("version") == Some(&JsValue::Number(1.0)));
    let Some(series_list) = series_list else {
        bail!("系列目录格式错误：需要 version: 1 和 series 数组。");
    };
    let mut ids = Vec::new();
    let mut articles = Vec::new();
    let mut series_titles = Vec::new();
    for series in series_list {
        let groups = series.prop("groups").and_then(JsValue::as_array);
        let articles_ok = series
            .prop("articles")
            .is_none_or(|a| a.as_array().is_some());
        let (Some(groups), true, true) = (groups, series.truthy(), articles_ok) else {
            bail!("系列需要 groups 数组，articles 如有填写也必须是数组。");
        };
        let title = title_text(series.prop("title"))?;
        if series_titles.contains(&title) {
            bail!("系列标题不能重复。");
        }
        series_titles.push(title);
        let entities: Vec<&JsValue> = std::iter::once(series).chain(groups.iter()).collect();
        for entity in &entities {
            let id = entity.prop("id").and_then(JsValue::as_str);
            let Some(id) = id.filter(|id| entity.truthy() && valid_id(id) && !ids.contains(id))
            else {
                bail!("系列和组需要唯一的英文 ID。");
            };
            ids.push(id);
            title_text(entity.prop("title"))?;
            if let Some(description) = entity.prop("description") {
                if description.as_str().is_none_or(|d| utf16_len(d) > 2000) {
                    bail!("简介需为不超过 2000 字的文字。");
                }
            }
        }
        let mut group_titles = Vec::new();
        for group in groups {
            let title = js_trim(group.prop("title").and_then(JsValue::as_str).unwrap_or(""));
            if group_titles.contains(&title)
                || group.prop("articles").and_then(JsValue::as_array).is_none()
            {
                bail!("组标题不能重复，且需要 articles 数组。");
            }
            group_titles.push(title);
        }
        for container in &entities {
            let list = container.prop("articles").filter(|a| a.truthy());
            for key in list.and_then(JsValue::as_array).into_iter().flatten() {
                let key = article_key(Some(key))?;
                if articles.contains(&key) {
                    bail!("一篇文章只能属于一个系列或组。");
                }
                articles.push(key);
            }
        }
    }
    Ok(())
}

pub fn catalog_path(content_root: &Path) -> Result<PathBuf> {
    let parent = resolve(content_root, &[".."])?;
    safe_path(&parent, &["data", "collections.json"])
}

/// Node's `fs.readFileSync(file, 'utf8')`.
fn read_text(file: &Path) -> Result<String> {
    match fs::read(file) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        Err(error) if file.is_dir() => {
            let _ = error;
            bail!("EISDIR: illegal operation on a directory, read")
        }
        Err(error) => Err(error.into()),
    }
}

pub fn read_catalog(content_root: &Path) -> Result<JsValue> {
    let file = catalog_path(content_root)?;
    if !file.exists() {
        let mut empty = JsObject::new();
        empty.insert("version", JsValue::Number(1.0));
        empty.insert("series", JsValue::Array(Vec::new()));
        return Ok(JsValue::Object(empty));
    }
    if fs::metadata(&file)?.len() > 2 * 1024 * 1024 {
        bail!("系列目录文件过大。");
    }
    let parsed = (|| -> Result<JsValue> {
        let text = read_text(&file)?;
        let data =
            js::parse(text.strip_prefix('\u{feff}').unwrap_or(&text)).map_err(|e| anyhow!(e))?;
        validate_catalog(&data)?;
        Ok(data)
    })();
    parsed.map_err(|error| anyhow!("请检查 collections.json：{error}"))
}

pub fn catalog_revision(data: &JsValue) -> String {
    hex::encode(Sha256::digest(js::stringify(data, 0).as_bytes()))
}

fn random_hex(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    getrandom::getrandom(&mut buffer).expect("operating system random numbers");
    hex::encode(buffer)
}

/// The expected revision as the request supplied it; anything but a
/// matching string is a conflict, as with JavaScript's `!==`.
pub enum Expected<'a> {
    Any,
    Revision(&'a JsValue),
}

fn update_catalog<T>(
    content_root: &Path,
    expected: Expected,
    change: impl FnOnce(&mut JsValue) -> Result<T>,
) -> Result<T> {
    let file = catalog_path(content_root)?;
    let directory = file.parent().unwrap().to_path_buf();
    fs::create_dir_all(&directory)?;
    let lock = safe_path(&directory, &["collections.json.lock"])?;
    let handle = match fs::OpenOptions::new().write(true).create_new(true).open(&lock) {
        Ok(handle) => handle,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => bail!(
            "另一个窗口正在修改系列目录，请稍后重试。若上次异常退出，请检查 data/collections.json.lock。"
        ),
        Err(error) => return Err(error.into()),
    };
    let temporary = safe_path(
        &directory,
        &[&format!("collections-{}.tmp", random_hex(16))],
    );
    let result = (|| {
        let temporary = temporary.as_ref().map_err(|e| anyhow!("{e}"))?;
        let mut data = read_catalog(content_root)?;
        if let Expected::Revision(expected) = expected {
            if expected.as_str() != Some(catalog_revision(&data).as_str()) {
                return Err(Conflict.into());
            }
        }
        let result = change(&mut data)?;
        validate_catalog(&data)?;
        let text = js::stringify(&data, 2) + "\n";
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temporary)?;
        fs::write(temporary, text)?;
        fs::rename(temporary, &file)?;
        Ok(result)
    })();
    if let Ok(temporary) = &temporary {
        if temporary.exists() {
            let _ = fs::remove_file(temporary);
        }
    }
    drop(handle);
    fs::remove_file(&lock)?;
    result
}

fn object_mut(value: &mut JsValue) -> &mut JsObject {
    value.as_object_mut().expect("validated catalog entry")
}

/// (series, group) positions of every series and group, in catalog order.
fn containers(data: &JsValue) -> Vec<(usize, Option<usize>)> {
    let mut result = Vec::new();
    for (s, series) in data
        .prop("series")
        .and_then(JsValue::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        result.push((s, None));
        for g in 0..series
            .prop("groups")
            .and_then(JsValue::as_array)
            .map_or(0, Vec::len)
        {
            result.push((s, Some(g)));
        }
    }
    result
}

fn container_mut(data: &mut JsValue, (s, g): (usize, Option<usize>)) -> &mut JsObject {
    let series = &mut object_mut(data)
        .get_mut("series")
        .and_then(|v| match v {
            JsValue::Array(a) => Some(a),
            _ => None,
        })
        .unwrap()[s];
    match g {
        None => object_mut(series),
        Some(g) => match object_mut(series).get_mut("groups").unwrap() {
            JsValue::Array(groups) => object_mut(&mut groups[g]),
            _ => unreachable!(),
        },
    }
}

fn includes(list: Option<&JsValue>, key: &str) -> bool {
    list.and_then(JsValue::as_array)
        .is_some_and(|items| items.iter().any(|item| item.as_str() == Some(key)))
}

pub fn create_series(content_root: &Path, title: &str, description: &str) -> Result<JsValue> {
    let title = title_text(Some(&JsValue::String(title.into())))?;
    update_catalog(content_root, Expected::Any, |data| {
        let mut series = JsObject::new();
        series.insert("id", JsValue::String(format!("s-{}", random_hex(4))));
        series.insert("title", JsValue::String(title));
        series.insert("description", JsValue::String(description.into()));
        series.insert("articles", JsValue::Array(Vec::new()));
        series.insert("groups", JsValue::Array(Vec::new()));
        let series = JsValue::Object(series);
        if let Some(JsValue::Array(list)) = object_mut(data).get_mut("series") {
            list.push(series.clone());
        }
        Ok(series)
    })
}

pub fn create_group(
    content_root: &Path,
    series_id: &str,
    title: &str,
    description: &str,
) -> Result<JsValue> {
    let title = title_text(Some(&JsValue::String(title.into())))?;
    update_catalog(content_root, Expected::Any, |data| {
        let position = containers(data).into_iter().find(|&(s, g)| {
            g.is_none()
                && data.prop("series").unwrap().as_array().unwrap()[s]
                    .prop("id")
                    .and_then(JsValue::as_str)
                    == Some(series_id)
        });
        let Some(position) = position else {
            bail!("系列已不存在，请重新选择。");
        };
        let mut group = JsObject::new();
        group.insert("id", JsValue::String(format!("g-{}", random_hex(4))));
        group.insert("title", JsValue::String(title));
        group.insert("description", JsValue::String(description.into()));
        group.insert("articles", JsValue::Array(Vec::new()));
        let mut result = group.clone();
        if let Some(JsValue::Array(groups)) = container_mut(data, position).get_mut("groups") {
            groups.push(JsValue::Object(group));
        }
        result.insert("seriesId", JsValue::String(series_id.into()));
        Ok(JsValue::Object(result))
    })
}

/// `beforeKey`: `None` is undefined (keep an existing position), `Some(None)`
/// appends, `Some(Some(key))` inserts before that article.
pub fn assign_article(
    content_root: &Path,
    key: &str,
    target_id: Option<&str>,
    before_key: Option<Option<&str>>,
    expected: Expected,
) -> Result<()> {
    article_key(Some(&JsValue::String(key.into())))?;
    let file = safe_path(content_root, &[key])?;
    match fs::metadata(&file) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => bail!("文章文件不存在。"),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            bail!(
                "ENOENT: no such file or directory, stat '{}'",
                file.display()
            )
        }
        Err(error) => return Err(error.into()),
    }
    let target_id = target_id.filter(|id| !id.is_empty());
    update_catalog(content_root, expected, |data| {
        let lists = containers(data);
        let id_of = |data: &JsValue, (s, g): (usize, Option<usize>)| {
            let series = &data.prop("series").unwrap().as_array().unwrap()[s];
            let entity = match g {
                None => series,
                Some(g) => &series.prop("groups").unwrap().as_array().unwrap()[g],
            };
            entity
                .prop("id")
                .and_then(JsValue::as_str)
                .map(str::to_string)
        };
        let target = target_id.and_then(|id| {
            lists
                .iter()
                .copied()
                .find(|&l| id_of(data, l).as_deref() == Some(id))
        });
        if target_id.is_some() && target.is_none() {
            bail!("系列或组已不存在，请重新选择。");
        }
        let target_articles =
            |data: &mut JsValue| target.map(|t| container_mut(data, t).get("articles").cloned());
        if let Some(Some(before)) = before_key {
            if !target_articles(data)
                .flatten()
                .is_some_and(|a| includes(Some(&a), before))
            {
                bail!("目标文章已移动，请刷新后重试。");
            }
        }
        // Choosing the existing destination preserves order; an explicit beforeKey
        // requests an insertion (null means append), including within the same list.
        if includes(target_articles(data).flatten().as_ref(), key)
            && (before_key.is_none() || before_key == Some(Some(key)))
        {
            return Ok(());
        }
        for &list in &lists {
            let container = container_mut(data, list);
            if let Some(JsValue::Array(items)) = container.get_mut("articles") {
                items.retain(|item| item.as_str() != Some(key));
            }
        }
        if let Some(target) = target {
            let container = container_mut(data, target);
            if !container.get("articles").is_some_and(JsValue::truthy) {
                container.insert("articles", JsValue::Array(Vec::new()));
            }
            if let Some(JsValue::Array(items)) = container.get_mut("articles") {
                let index = match before_key.flatten() {
                    None => items.len(),
                    Some(before) => items
                        .iter()
                        .position(|item| item.as_str() == Some(before))
                        .unwrap(),
                };
                items.insert(index, JsValue::String(key.into()));
            }
        }
        Ok(())
    })
}

pub fn move_article(content_root: &Path, key: &str, delta: i64) -> Result<()> {
    update_catalog(content_root, Expected::Any, |data| {
        let found = containers(data)
            .into_iter()
            .find(|&l| includes(container_mut(data, l).get("articles"), key));
        let Some(found) = found else { return Ok(()) };
        if let Some(JsValue::Array(items)) = container_mut(data, found).get_mut("articles") {
            let from = items
                .iter()
                .position(|item| item.as_str() == Some(key))
                .unwrap() as i64;
            let to = from + delta.signum();
            if to >= 0 && (to as usize) < items.len() {
                items.swap(from as usize, to as usize);
            }
        }
        Ok(())
    })
}

#[derive(Debug)]
pub struct CreatedArticle {
    pub key: String,
    pub file: PathBuf,
}

pub fn create_article(
    content_root: &Path,
    title: &str,
    target_id: Option<&str>,
) -> Result<CreatedArticle> {
    let title = title_text(Some(&JsValue::String(title.into())))?;
    let target_id = target_id.filter(|id| !id.is_empty());
    if let Some(id) = target_id {
        let data = read_catalog(content_root)?;
        let exists = containers(&data).into_iter().any(|(s, g)| {
            let series = &data.prop("series").unwrap().as_array().unwrap()[s];
            let entity = g.map_or(series, |g| {
                &series.prop("groups").unwrap().as_array().unwrap()[g]
            });
            entity.prop("id").and_then(JsValue::as_str) == Some(id)
        });
        if !exists {
            bail!("系列或组已不存在，请重新选择。");
        }
    }
    let replaced: String = title
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || (c as u32) < 0x20 {
                '-'
            } else {
                c
            }
        })
        .collect();
    let trimmed = replaced.trim_matches([' ', '.', '-']);
    let slug: String = trimmed.chars().take(80).collect();
    let mut slug = slug.trim_end_matches([' ', '.']).to_string();
    if slug.is_empty() {
        bail!("标题需要包含可用于目录名的文字或数字。");
    }
    let base = slug.split('.').next().unwrap().to_lowercase();
    let reserved = ["con", "prn", "aux", "nul"].contains(&base.as_str())
        || ["com", "lpt"].iter().any(|prefix| {
            base.strip_prefix(prefix).is_some_and(|rest| {
                let mut chars = rest.chars();
                matches!(
                    (chars.next(), chars.next()),
                    (Some('1'..='9' | '¹' | '²' | '³'), None)
                )
            })
        });
    if reserved {
        slug = format!("文章-{slug}");
    }
    let folder = safe_path(content_root, &["posts", &slug])?;
    fs::create_dir_all(safe_path(content_root, &["posts"])?)?;
    match fs::create_dir(&folder) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            bail!("同名文章目录已经存在，请换一个标题。")
        }
        Err(error) => return Err(error.into()),
    }
    let key = format!("posts/{slug}/index.md");
    let file = folder.join("index.md");
    let date = chrono::Local::now().format("%Y-%m-%d");
    let quoted = js::stringify(&JsValue::String(title), 0).replace('+', "\\u002b");
    let text = format!(
        "+++\ntitle = {quoted}\ndate = {date}\ndescription = \"\"\ndraft = true\n\n[taxonomies]\ntags = []\n+++\n\n## 问题与背景\n\n在这里开始写作。\n\n## 我的思考\n\n"
    );
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&file)?;
    fs::write(&file, text)?;
    fs::create_dir(folder.join("figures"))?;
    if let Some(id) = target_id {
        if let Err(error) = assign_article(content_root, &key, Some(id), None, Expected::Any) {
            bail!(
                "草稿已保存到 {}，但加入系列或组失败：{error}。可稍后使用“移动文章到系列或组”。",
                file.display()
            );
        }
    }
    Ok(CreatedArticle { key, file })
}

fn line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// The TOML header the extension's regular expression finds:
/// `/^﻿?\+\+\+\s*\r?\n([\s\S]*?)^\+\+\+\s*$/m`.
fn toml_header(text: &str) -> Option<&str> {
    let starts = std::iter::once(0).chain(
        text.char_indices()
            .filter(|(_, c)| line_terminator(*c))
            .map(|(i, c)| i + c.len_utf8()),
    );
    for start in starts {
        let open = start
            + if text[start..].starts_with('\u{feff}') {
                3
            } else {
                0
            };
        if !text[open..].starts_with("+++") {
            continue;
        }
        let after = open + 3;
        let run = text[after..]
            .find(|c: char| !js_space(c))
            .map_or(text.len(), |i| after + i);
        let Some(newline) = text[after..run].rfind('\n') else {
            continue;
        };
        let body = after + newline + 1;
        // The closing fence starts with '+', so it lies after the whitespace run.
        let mut search = run;
        while let Some(found) = text[search..].find("+++") {
            let fence = search + found;
            let line_start = fence == 0
                || text[..fence]
                    .chars()
                    .next_back()
                    .is_some_and(line_terminator);
            let tail = fence + 3;
            let tail_end = text[tail..]
                .find(|c: char| !js_space(c))
                .map_or(text.len(), |i| tail + i);
            if line_start
                && (tail_end == text.len() || text[tail..tail_end].contains(line_terminator))
            {
                return Some(&text[body..fence]);
            }
            search = fence + 1;
        }
        return None;
    }
    None
}

fn toml_to_js(value: &toml::Value) -> JsValue {
    match value {
        toml::Value::String(s) => JsValue::String(s.clone()),
        toml::Value::Integer(i) => JsValue::Number(*i as f64),
        toml::Value::Float(f) => JsValue::Number(*f),
        toml::Value::Boolean(b) => JsValue::Bool(*b),
        toml::Value::Datetime(d) => JsValue::String(d.to_string()),
        toml::Value::Array(items) => JsValue::Array(items.iter().map(toml_to_js).collect()),
        toml::Value::Table(table) => {
            let mut object = JsObject::new();
            for (key, value) in table {
                object.insert(key, toml_to_js(value));
            }
            JsValue::Object(object)
        }
    }
}

pub struct Article {
    pub key: String,
    pub file: PathBuf,
    /// `meta.title || directory name`, which need not be a string.
    pub title: JsValue,
    pub draft: bool,
}

pub fn list_articles(content_root: &Path) -> Result<Vec<Article>> {
    let posts = safe_path(content_root, &["posts"])?;
    if !posts.exists() {
        return Ok(Vec::new());
    }
    let mut entries: Vec<(String, fs::FileType)> = fs::read_dir(&posts)?
        .map(|entry| {
            let entry = entry?;
            Ok((
                entry.file_name().to_string_lossy().into_owned(),
                entry.file_type()?,
            ))
        })
        .collect::<Result<_>>()?;
    // libuv sorts directory listings on Unix; Windows keeps the NTFS order.
    if !cfg!(windows) {
        entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    }
    let mut result = Vec::new();
    for (name, kind) in entries {
        if !kind.is_dir() || kind.is_symlink() {
            continue;
        }
        let key = format!("posts/{name}/index.md");
        let file = safe_path(content_root, &[&key])?;
        if !file.exists() {
            continue;
        }
        let text = read_text(&file)?;
        // Keep partially edited articles visible.
        let meta = toml_header(&text)
            .and_then(|header| header.parse::<toml::Table>().ok())
            .unwrap_or_default();
        let title = meta
            .get("title")
            .map(toml_to_js)
            .filter(JsValue::truthy)
            .unwrap_or(JsValue::String(name));
        let draft = meta.get("draft") == Some(&toml::Value::Boolean(true));
        result.push(Article {
            key,
            file,
            title,
            draft,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("blog-library-{}", random_hex(6)));
        let content = root.join("site/content");
        fs::create_dir_all(&content).unwrap();
        (root, content)
    }

    fn catalog(content: &Path) -> JsValue {
        read_catalog(content).unwrap()
    }

    fn list(data: &JsValue, path: &[(&str, usize)]) -> Vec<String> {
        let mut value = data;
        for (key, index) in path {
            value = &value.prop(key).unwrap().as_array().unwrap()[*index];
        }
        value
            .prop("articles")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect()
    }

    fn id(value: &JsValue) -> String {
        value.prop("id").unwrap().as_str().unwrap().to_string()
    }

    #[test]
    fn series_groups_reassignment_and_order_persist() {
        let (root, content) = fixture();
        let s1 = create_series(&content, "检索研究", "从词到向量").unwrap();
        let s2 = create_series(&content, "其他笔记", "").unwrap();
        let g1 = create_group(&content, &id(&s1), "基础", "").unwrap();
        let g2 = create_group(&content, &id(&s1), "实践", "").unwrap();
        let g3 = create_group(&content, &id(&s2), "基础", "").unwrap();
        let a = create_article(&content, "从文字到向量", Some(&id(&g1))).unwrap();
        let b = create_article(&content, "检索的边界", Some(&id(&g1))).unwrap();
        let original = fs::read(&a.file).unwrap();
        assert!(
            list_articles(&content)
                .unwrap()
                .iter()
                .find(|x| x.key == a.key)
                .unwrap()
                .draft
        );
        move_article(&content, &b.key, -1).unwrap();
        assert_eq!(
            list(&catalog(&content), &[("series", 0), ("groups", 0)]),
            [b.key.clone(), a.key.clone()]
        );
        assign_article(&content, &b.key, Some(&id(&g1)), None, Expected::Any).unwrap();
        assert_eq!(
            list(&catalog(&content), &[("series", 0), ("groups", 0)]),
            [b.key.clone(), a.key.clone()]
        );
        assign_article(&content, &a.key, Some(&id(&g2)), None, Expected::Any).unwrap();
        assign_article(&content, &b.key, Some(&id(&g3)), None, Expected::Any).unwrap();
        assign_article(&content, &a.key, None, None, Expected::Any).unwrap();
        let data = catalog(&content);
        assert!(list(&data, &[("series", 0), ("groups", 0)]).is_empty());
        assert!(list(&data, &[("series", 0), ("groups", 1)]).is_empty());
        assert_eq!(
            list(&data, &[("series", 1), ("groups", 0)]),
            std::slice::from_ref(&b.key)
        );
        assert_eq!(fs::read(&a.file).unwrap(), original);
        assert_eq!(list_articles(&content).unwrap().len(), 2);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_operations_preserve_catalog() {
        let (root, content) = fixture();
        let series = create_series(&content, "同名", "").unwrap();
        let group = create_group(&content, &id(&series), "第一组", "").unwrap();
        let article = create_article(&content, "现有文章", Some(&id(&group))).unwrap();
        let before = fs::read(catalog_path(&content).unwrap()).unwrap();
        assert!(create_series(&content, " 同名 ", "")
            .unwrap_err()
            .to_string()
            .contains("重复"));
        assert!(create_group(&content, &id(&series), "第一组", "")
            .unwrap_err()
            .to_string()
            .contains("重复"));
        assert!(create_article(&content, "现有文章", None)
            .unwrap_err()
            .to_string()
            .contains("已经存在"));
        let missing = assign_article(
            &content,
            &article.key,
            Some("not-found"),
            None,
            Expected::Any,
        );
        assert!(missing.unwrap_err().to_string().contains("不存在"));
        assert_eq!(fs::read(catalog_path(&content).unwrap()).unwrap(), before);
        assert!(!catalog_path(&content)
            .unwrap()
            .with_extension("json.lock")
            .exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bad_catalogs_traversal_duplicates_and_locks() {
        let (root, content) = fixture();
        assert!(safe_path(&content, &["../escape"])
            .unwrap_err()
            .to_string()
            .contains("目录内"));
        assert!(article_key(Some(&JsValue::String("posts/../index.md".into()))).is_err());
        assert!(article_key(Some(&JsValue::String("posts/a/index.md:secret".into()))).is_err());
        let series = create_series(&content, "研究", "").unwrap();
        let group = create_group(&content, &id(&series), "小组", "").unwrap();
        let article = create_article(&content, "草稿", Some(&id(&group))).unwrap();
        let text = js::stringify(&catalog(&content), 0)
            .replace("]}]}]", &format!(",\"{}\"]}}]}}]", article.key));
        let duplicated = js::parse(&text).unwrap();
        assert!(validate_catalog(&duplicated)
            .unwrap_err()
            .to_string()
            .contains("一个系列或组"));
        let path = catalog_path(&content).unwrap();
        let lock = path.with_file_name("collections.json.lock");
        fs::write(&lock, "").unwrap();
        assert!(create_series(&content, "并发", "")
            .unwrap_err()
            .to_string()
            .contains("另一个窗口"));
        fs::remove_file(&lock).unwrap();
        fs::write(&path, "{broken").unwrap();
        assert!(create_series(&content, "不覆盖坏文件", "")
            .unwrap_err()
            .to_string()
            .contains("检查 collections"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "{broken");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn direct_series_articles_support_insertion_and_moves() {
        let (root, content) = fixture();
        let first = create_series(&content, "直接阅读", "").unwrap();
        let second = create_series(&content, "其他系列", "").unwrap();
        let group = create_group(&content, &id(&second), "可选分组", "").unwrap();
        let a = create_article(&content, "甲", Some(&id(&first))).unwrap();
        let b = create_article(&content, "乙", Some(&id(&first))).unwrap();
        let c = create_article(&content, "丙", Some(&id(&first))).unwrap();
        assign_article(
            &content,
            &c.key,
            Some(&id(&first)),
            Some(Some(&a.key)),
            Expected::Any,
        )
        .unwrap();
        assert_eq!(
            list(&catalog(&content), &[("series", 0)]),
            [c.key.clone(), a.key.clone(), b.key.clone()]
        );
        assign_article(&content, &c.key, Some(&id(&first)), None, Expected::Any).unwrap();
        assert_eq!(
            list(&catalog(&content), &[("series", 0)]),
            [c.key.clone(), a.key.clone(), b.key.clone()]
        );
        assign_article(
            &content,
            &c.key,
            Some(&id(&first)),
            Some(None),
            Expected::Any,
        )
        .unwrap();
        move_article(&content, &b.key, -1).unwrap();
        assert_eq!(
            list(&catalog(&content), &[("series", 0)]),
            [b.key.clone(), a.key.clone(), c.key.clone()]
        );
        assign_article(&content, &a.key, Some(&id(&group)), None, Expected::Any).unwrap();
        assign_article(&content, &a.key, Some(&id(&second)), None, Expected::Any).unwrap();
        assign_article(&content, &c.key, None, None, Expected::Any).unwrap();
        let data = catalog(&content);
        assert_eq!(list(&data, &[("series", 0)]), std::slice::from_ref(&b.key));
        assert_eq!(list(&data, &[("series", 1)]), std::slice::from_ref(&a.key));
        assert!(list(&data, &[("series", 1), ("groups", 0)]).is_empty());
        assert_eq!(list_articles(&content).unwrap().len(), 3);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_catalogs_and_stale_revisions() {
        let (root, content) = fixture();
        let series = create_series(&content, "旧目录", "").unwrap();
        let group = create_group(&content, &id(&series), "旧组", "").unwrap();
        let a = create_article(&content, "旧文", Some(&id(&group))).unwrap();
        let b = create_article(&content, "新文", None).unwrap();
        let path = catalog_path(&content).unwrap();
        let legacy = js::stringify(&catalog(&content), 0)
            .replace("\"articles\":[],\"groups\"", "\"groups\"");
        fs::write(&path, legacy).unwrap();
        let revision = JsValue::String(catalog_revision(&catalog(&content)));
        assign_article(
            &content,
            &a.key,
            Some(&id(&series)),
            Some(None),
            Expected::Revision(&revision),
        )
        .unwrap();
        let saved = fs::read(&path).unwrap();
        let stale = assign_article(
            &content,
            &b.key,
            Some(&id(&series)),
            Some(None),
            Expected::Revision(&revision),
        );
        assert!(stale.unwrap_err().downcast_ref::<Conflict>().is_some());
        let moved = assign_article(
            &content,
            &b.key,
            Some(&id(&series)),
            Some(Some("posts/missing/index.md")),
            Expected::Any,
        );
        assert!(moved.unwrap_err().to_string().contains("目标文章"));
        assert_eq!(fs::read(&path).unwrap(), saved);
        assert_eq!(
            list(&catalog(&content), &[("series", 0)]),
            std::slice::from_ref(&a.key)
        );
        // A series that gained `articles` lists it after `groups`, as JavaScript does.
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            text.find("\"groups\"").unwrap()
                < text.find("\"articles\": [\n        \"posts").unwrap()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn titles_are_escaped_and_partial_sources_stay_listed() {
        let (root, content) = fixture();
        let a = create_article(&content, "研究 \"A\" +++ / B", None).unwrap();
        let text = fs::read_to_string(&a.file).unwrap();
        let header: toml::Table = text.split("+++").nth(1).unwrap().parse().unwrap();
        assert_eq!(header["title"].as_str(), Some("研究 \"A\" +++ / B"));
        assert!(inside(&content, &a.file).unwrap());
        fs::write(&a.file, "+++\ntitle = \"half typed\n+++\n").unwrap();
        assert_eq!(list_articles(&content).unwrap().len(), 1);
        let reserved = create_article(&content, "CON", None).unwrap();
        assert!(reserved.key.contains("文章-CON"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn header_regex_semantics() {
        assert_eq!(toml_header("+++\ntitle = 1\n+++\n"), Some("title = 1\n"));
        assert_eq!(
            toml_header("\u{feff}+++  \r\n\na=1\r\n+++  "),
            Some("a=1\r\n")
        );
        assert_eq!(
            toml_header("x\n+++\nb=2\n+++ x\n+++\n"),
            Some("b=2\n+++ x\n")
        );
        assert_eq!(toml_header("+++ a\n+++\n"), None);
        assert_eq!(toml_header("+++\na\n"), None);
    }
}
