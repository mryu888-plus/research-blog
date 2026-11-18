//! Find, install and check the pinned build tools: Zola 0.23.4 and Typst 0.15.1.
//!
//! Each tool is looked up in this order: the ZOLA or TYPST environment
//! variable, `.tools/<tool>/` inside the project, the per-user cache
//! (`<local data>/research-blog/tools/<tool>-<version>/`, overridable with
//! RESEARCH_BLOG_TOOLS_DIR), then PATH when it reports the pinned version.
//! Missing tools are downloaded from the official GitHub releases into the
//! per-user cache, checked against the SHA-256 hashes pinned below, and
//! unpacked only inside that cache.

use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

pub const ZOLA_VERSION: &str = "0.23.4";
pub const TYPST_VERSION: &str = "0.15.1";
const CHUNK: usize = 1024 * 1024;
const VERSION_MARKER: &str = ".research-blog-version";

/// (system, arch) and the pinned (asset, sha256) for it.
pub type Asset = ((&'static str, &'static str), (&'static str, &'static str));

pub struct Release {
    pub name: &'static str,
    pub version: &'static str,
    pub base: &'static str,
    pub assets: &'static [Asset],
}

// Official release assets, with SHA-256 hashes computed from them on 2026-10-13.
pub const RELEASES: [Release; 2] = [
    Release {
        name: "zola",
        version: ZOLA_VERSION,
        base: "https://github.com/getzola/zola/releases/download/v0.23.4/",
        assets: &[
            (
                ("windows", "x86_64"),
                (
                    "zola-v0.23.4-x86_64-pc-windows-msvc.zip",
                    "dc839498e27b631f60fd5e1527935198b7f002eb09b8446a1e2913a8fa386925",
                ),
            ),
            // Zola has no Windows arm64 build; the x86_64 binary runs under emulation.
            (
                ("windows", "aarch64"),
                (
                    "zola-v0.23.4-x86_64-pc-windows-msvc.zip",
                    "dc839498e27b631f60fd5e1527935198b7f002eb09b8446a1e2913a8fa386925",
                ),
            ),
            (
                ("linux", "x86_64"),
                (
                    "zola-v0.23.4-x86_64-unknown-linux-gnu.tar.gz",
                    "54d1a347781b2f32330914fcc02def81c7e3ddb6111b36d1cc89c06557aed1de",
                ),
            ),
            (
                ("linux-musl", "x86_64"),
                (
                    "zola-v0.23.4-x86_64-unknown-linux-musl.tar.gz",
                    "d99c51302ebbf909a0d83d4319d4d745b56a93dc49c4a69878c0f0dcaa4c8531",
                ),
            ),
            (
                ("linux", "aarch64"),
                (
                    "zola-v0.23.4-aarch64-unknown-linux-gnu.tar.gz",
                    "21bb37a4f3bbac663cf8f04df9b51ac6bc154acfe2cf2c3e9ea162b4951487b6",
                ),
            ),
            (
                ("darwin", "x86_64"),
                (
                    "zola-v0.23.4-x86_64-apple-darwin.tar.gz",
                    "e79edcba2e8d03d22065c9cb8fa2e3abf07b823ef17f00abdc060188dceabba7",
                ),
            ),
            (
                ("darwin", "aarch64"),
                (
                    "zola-v0.23.4-aarch64-apple-darwin.tar.gz",
                    "303b8e1f3251a6250e47f811eda143316f653c22201faa66777d48ac499c0ee3",
                ),
            ),
        ],
    },
    Release {
        name: "typst",
        version: TYPST_VERSION,
        base: "https://github.com/typst/typst/releases/download/v0.15.1/",
        assets: &[
            (
                ("windows", "x86_64"),
                (
                    "typst-x86_64-pc-windows-msvc.zip",
                    "19ce3551153c2fe7ee9fa2f95208310c8f4d3209fedb699e0333faf8913f6736",
                ),
            ),
            (
                ("windows", "aarch64"),
                (
                    "typst-aarch64-pc-windows-msvc.zip",
                    "4ab28e1b71ec3184d38d580ab797f499b6770d952b6b19167be5cea5c2662e14",
                ),
            ),
            // Typst ships static musl binaries for Linux; they run on glibc systems too.
            (
                ("linux", "x86_64"),
                (
                    "typst-x86_64-unknown-linux-musl.tar.xz",
                    "a6d077d0a95eed5a2eba715b2dae06be954f624ccbf85758a03f389ded33118c",
                ),
            ),
            (
                ("linux-musl", "x86_64"),
                (
                    "typst-x86_64-unknown-linux-musl.tar.xz",
                    "a6d077d0a95eed5a2eba715b2dae06be954f624ccbf85758a03f389ded33118c",
                ),
            ),
            (
                ("linux", "aarch64"),
                (
                    "typst-aarch64-unknown-linux-musl.tar.xz",
                    "5aa8d74a3d906e60ea12a66ac2f37f8eef1b14cbad7182a745e393a10c23dcee",
                ),
            ),
            (
                ("linux-musl", "aarch64"),
                (
                    "typst-aarch64-unknown-linux-musl.tar.xz",
                    "5aa8d74a3d906e60ea12a66ac2f37f8eef1b14cbad7182a745e393a10c23dcee",
                ),
            ),
            (
                ("darwin", "x86_64"),
                (
                    "typst-x86_64-apple-darwin.tar.xz",
                    "7f9fdd9584866245de9a79e0add8f9236fae6f40a8a45e2c4771ccc14db4e0fa",
                ),
            ),
            (
                ("darwin", "aarch64"),
                (
                    "typst-aarch64-apple-darwin.tar.xz",
                    "48f62ed034aa3a7978309579ac6ca00045e2ef0da73114e8af27cfd8e74dc05a",
                ),
            ),
        ],
    },
];

const CJK_FONT_FILES: [&str; 8] = [
    "msyh.ttc",
    "msyh.ttf",
    "msyhbd.ttc",
    "simhei.ttf",
    "simsun.ttc",
    "NotoSansCJK-Regular.ttc",
    "NotoSansSC-Regular.otf",
    "SourceHanSansSC-Regular.otf",
];

pub fn release(name: &str) -> Result<&'static Release> {
    RELEASES
        .iter()
        .find(|release| release.name == name)
        .ok_or_else(|| anyhow!("未知的工具：{name}"))
}

/// ("windows" | "linux" | "linux-musl" | "darwin", "x86_64" | "aarch64").
pub fn platform_key() -> Result<(&'static str, &'static str)> {
    let mut system = match std::env::consts::OS {
        "windows" => "windows",
        "linux" => "linux",
        "macos" => "darwin",
        other => other,
    };
    let arch = std::env::consts::ARCH;
    if system == "linux" && is_musl() {
        system = "linux-musl";
    }
    if !["windows", "linux", "linux-musl", "darwin"].contains(&system)
        || !["x86_64", "aarch64"].contains(&arch)
    {
        bail!(
            "没有 {} {} 的预编译版本，请自己安装 Zola 和 Typst 并放进 PATH。",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
    }
    Ok((system, arch))
}

fn is_musl() -> bool {
    if cfg!(target_env = "musl") {
        return true;
    }
    fs::read_dir("/lib").is_ok_and(|entries| {
        entries
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().starts_with("ld-musl-"))
    })
}

/// The release asset and hash for a tool on a platform.
///
/// A musl Linux falls back to the glibc build when no musl build is pinned.
pub fn asset_for(name: &str, key: (&str, &str)) -> Result<(&'static str, &'static str)> {
    let assets = release(name)?.assets;
    let find = |key: (&str, &str)| {
        assets
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, asset)| *asset)
    };
    let found = find(key).or_else(|| match key {
        ("linux-musl", arch) => find(("linux", arch)),
        _ => None,
    });
    found.ok_or_else(|| anyhow!("没有 {name} 在 {} {} 上的预编译版本。", key.0, key.1))
}

static VERSION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+\.\d+\.\d+)").unwrap());

pub fn parse_version(output: &str) -> Option<String> {
    VERSION
        .captures(output)
        .map(|captures| captures[1].to_string())
}

/// `<command> --version`, giving up after 20 seconds.
pub fn run_version(command: &Path) -> Option<String> {
    let mut child = Command::new(command)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let mut stderr = child.stderr.take()?;
    let readers = (
        std::thread::spawn(move || {
            let mut text = String::new();
            stdout.read_to_string(&mut text).ok();
            text
        }),
        std::thread::spawn(move || {
            let mut text = String::new();
            stderr.read_to_string(&mut text).ok();
            text
        }),
    );
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < Duration::from_secs(20) => {
                std::thread::sleep(Duration::from_millis(10))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let output = readers.0.join().unwrap_or_default() + &readers.1.join().unwrap_or_default();
    parse_version(&output)
}

pub fn binary_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// The environment that decides where tools are found.
#[derive(Clone, Debug, Default)]
pub struct Environment {
    /// ZOLA / TYPST.
    pub overrides: Vec<(String, OsString)>,
    /// Root of the per-user tool cache.
    pub cache: Option<PathBuf>,
    pub path: Option<OsString>,
}

impl Environment {
    pub fn current() -> Self {
        let overrides = ["ZOLA", "TYPST"]
            .iter()
            .filter_map(|name| {
                std::env::var_os(name)
                    .filter(|value| !value.is_empty())
                    .map(|value| (name.to_string(), value))
            })
            .collect();
        Environment {
            overrides,
            cache: cache_root(),
            path: std::env::var_os("PATH"),
        }
    }

    fn override_for(&self, name: &str) -> Option<&OsString> {
        let key = name.to_uppercase();
        self.overrides
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, value)| value)
    }

    /// The directory a tool version is cached in.
    pub fn cache_dir(&self, name: &str) -> Result<PathBuf> {
        let version = release(name)?.version;
        let cache = self
            .cache
            .clone()
            .ok_or_else(|| anyhow!("找不到用户数据目录，请设置 RESEARCH_BLOG_TOOLS_DIR。"))?;
        Ok(cache.join(format!("{name}-{version}")))
    }

    fn cached(&self, name: &str) -> Option<PathBuf> {
        let directory = self.cache_dir(name).ok()?;
        let binary = directory.join(binary_name(name));
        let marker = fs::read_to_string(directory.join(VERSION_MARKER)).ok()?;
        (binary.is_file() && marker.trim() == release(name).ok()?.version).then_some(binary)
    }

    fn which(&self, name: &str) -> Option<PathBuf> {
        let path = self.path.as_ref()?;
        let extensions: Vec<String> = if cfg!(windows) {
            std::env::var("PATHEXT")
                .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
                .split(';')
                .filter(|ext| !ext.is_empty())
                .map(str::to_lowercase)
                .collect()
        } else {
            vec![String::new()]
        };
        for directory in std::env::split_paths(path) {
            for extension in &extensions {
                let candidate = directory.join(format!("{name}{extension}"));
                if is_executable(&candidate) {
                    return Some(candidate);
                }
            }
        }
        None
    }
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && (metadata.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

/// `<local data>/research-blog/tools`, or RESEARCH_BLOG_TOOLS_DIR.
pub fn cache_root() -> Option<PathBuf> {
    if let Some(directory) = std::env::var_os("RESEARCH_BLOG_TOOLS_DIR").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(directory));
    }
    let home = || {
        std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        home().map(|home| home.join("Library").join("Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| home().map(|home| home.join(".local").join("share")))
    }?;
    Some(base.join("research-blog").join("tools"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Env,
    Tools,
    Cache,
    Path,
}

impl Source {
    fn key(self) -> &'static str {
        match self {
            Source::Env => "env",
            Source::Tools => "tools",
            Source::Cache => "cache",
            Source::Path => "path",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Source::Env => "环境变量",
            Source::Tools => ".tools",
            Source::Cache => "用户缓存",
            Source::Path => "PATH",
        }
    }
}

/// Where a tool would be found now, and which source wins, without checking
/// the version of a PATH copy.
pub fn locate(name: &str, root: &Path, env: &Environment) -> Option<(PathBuf, Source)> {
    if let Some(value) = env.override_for(name) {
        return Some((PathBuf::from(value), Source::Env));
    }
    let bundled = root.join(".tools").join(name).join(binary_name(name));
    if bundled.is_file() {
        return Some((bundled, Source::Tools));
    }
    if let Some(cached) = env.cached(name) {
        return Some((cached, Source::Cache));
    }
    env.which(name).map(|found| (found, Source::Path))
}

/// The tool to run, installing the pinned version into the user cache when
/// no acceptable copy exists.
pub fn tool_path(name: &str, root: &Path) -> Result<PathBuf> {
    let env = Environment::current();
    let version = release(name)?.version;
    match locate(name, root, &env) {
        Some((path, Source::Env | Source::Tools | Source::Cache)) => return Ok(path),
        Some((path, Source::Path)) if run_version(&path).as_deref() == Some(version) => {
            return Ok(path)
        }
        _ => {}
    }
    let directory = env.cache_dir(name)?;
    eprintln!("首次使用 {name}，下载 {version} 到 {}", directory.display());
    install(name, &env, &download_prefix(None), false, false)
}

fn download_prefix(explicit: Option<&str>) -> String {
    match explicit {
        Some(prefix) => prefix.to_string(),
        None => std::env::var("RESEARCH_BLOG_DOWNLOAD_PREFIX").unwrap_or_default(),
    }
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0; CHUNK];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex::encode(digest.finalize()))
}

/// A name unique to this process and call, for staging files.
fn unique(prefix: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    format!(
        "{prefix}-{}-{}-{nanos}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

/// Download `url` into a unique file next to `target`, returning that file.
pub fn download(url: &str, target: &Path, quiet: bool) -> Result<PathBuf> {
    let parent = target.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let name = target
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let partial = parent.join(unique(&format!(".{name}")) + ".part");
    let agent = ureq::AgentBuilder::new()
        .try_proxy_from_env(true)
        .timeout_connect(Duration::from_secs(30))
        .timeout_read(Duration::from_secs(120))
        .user_agent("research-blog-setup")
        .build();
    let result = (|| -> Result<()> {
        let response = agent
            .get(url)
            .call()
            .with_context(|| format!("下载失败：{url}"))?;
        let total: u64 = response
            .header("Content-Length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let mut reader = response.into_reader();
        let mut output = fs::File::create(&partial)?;
        let mut buffer = vec![0; CHUNK];
        let mut done = 0u64;
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            output.write_all(&buffer[..read])?;
            done += read as u64;
            if !quiet && total > 0 {
                eprint!(
                    "\r  下载 {name}  {:.0} / {:.0} MB",
                    done as f64 / 1e6,
                    total as f64 / 1e6
                );
                io::stderr().flush().ok();
            }
        }
        output.sync_all().ok();
        if !quiet {
            eprintln!();
        }
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&partial);
        return Err(error);
    }
    Ok(partial)
}

/// Reject archive members that are absolute or climb out with `..`.
pub fn check_member(archive: &Path, name: &str) -> Result<()> {
    let normalized = name.replace('\\', "/");
    let path = Path::new(&normalized);
    let drive = normalized.len() >= 2 && normalized.as_bytes()[1] == b':';
    if drive
        || normalized.starts_with('/')
        || path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!(
            "拒绝解压带有越界路径的压缩包：{} 里的 {name}",
            archive.file_name().unwrap_or_default().to_string_lossy()
        );
    }
    Ok(())
}

fn member_target(destination: &Path, name: &str) -> PathBuf {
    let mut target = destination.to_path_buf();
    for part in name.replace('\\', "/").split('/') {
        if !part.is_empty() && part != "." {
            target.push(part);
        }
    }
    target
}

/// Unpack into `destination`; members may not escape it, and links are skipped.
pub fn extract(archive: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    let file_name = archive
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    if file_name.ends_with(".zip") {
        let mut bundle = zip::ZipArchive::new(fs::File::open(archive)?)?;
        let names: Vec<String> = bundle.file_names().map(str::to_string).collect();
        for name in &names {
            check_member(archive, name)?;
        }
        for index in 0..bundle.len() {
            let mut entry = bundle.by_index(index)?;
            let name = entry.name().to_string();
            let target = member_target(destination, &name);
            if entry.is_dir() {
                fs::create_dir_all(&target)?;
            } else if entry.is_file() && !entry.is_symlink() {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                io::copy(&mut entry, &mut fs::File::create(&target)?)?;
            }
        }
        return Ok(());
    }
    let reader: Box<dyn Read> = if file_name.ends_with(".tar.xz") || file_name.ends_with(".txz") {
        let mut input = io::BufReader::new(fs::File::open(archive)?);
        let mut decompressed = Vec::new();
        lzma_rs::xz_decompress(&mut input, &mut decompressed)
            .map_err(|e| anyhow!("无法解压 {file_name}：{e}"))?;
        Box::new(io::Cursor::new(decompressed))
    } else if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
        Box::new(flate2::read::GzDecoder::new(fs::File::open(archive)?))
    } else {
        Box::new(fs::File::open(archive)?)
    };
    // Read the whole tar once so every member name is checked before writing.
    let mut entries = Vec::new();
    let mut bundle = tar::Archive::new(reader);
    for entry in bundle.entries()? {
        let mut entry = entry?;
        let name = String::from_utf8_lossy(&entry.path_bytes()).to_string();
        check_member(archive, &name)?;
        let kind = entry.header().entry_type();
        let mut data = Vec::new();
        if kind.is_file() {
            entry.read_to_end(&mut data)?;
        }
        entries.push((name, kind, data));
    }
    for (name, kind, data) in entries {
        let target = member_target(destination, &name);
        if kind.is_dir() {
            fs::create_dir_all(&target)?;
        } else if kind.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&target, data)?;
        }
    }
    Ok(())
}

pub fn find_binary(folder: &Path, name: &str) -> Result<PathBuf> {
    let wanted = binary_name(name);
    let mut matches = Vec::new();
    let mut pending = vec![folder.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() && entry.file_name().to_string_lossy() == wanted {
                matches.push(entry.path());
            }
        }
    }
    matches.sort();
    matches
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("压缩包里没有 {wanted}。"))
}

/// What to install and how to verify it; tests replace the network parts.
pub struct InstallPlan<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub asset: &'a str,
    pub digest: &'a str,
    pub url: String,
    pub directory: PathBuf,
    /// Run the unpacked binary and compare its `--version`.
    pub verify_version: bool,
}

/// Download, verify and unpack one tool into the per-user cache.
pub fn install(
    name: &str,
    env: &Environment,
    proxy_prefix: &str,
    force: bool,
    quiet: bool,
) -> Result<PathBuf> {
    let release = release(name)?;
    let (asset, digest) = asset_for(name, platform_key()?)?;
    let plan = InstallPlan {
        name,
        version: release.version,
        asset,
        digest,
        url: format!("{proxy_prefix}{}{asset}", release.base),
        directory: env.cache_dir(name)?,
        verify_version: true,
    };
    install_plan(&plan, force, quiet)
}

pub fn install_plan(plan: &InstallPlan, force: bool, quiet: bool) -> Result<PathBuf> {
    let binary = plan.directory.join(binary_name(plan.name));
    let marker = plan.directory.join(VERSION_MARKER);
    if !force
        && binary.is_file()
        && fs::read_to_string(&marker).is_ok_and(|v| v.trim() == plan.version)
    {
        return Ok(binary);
    }
    let cache = plan
        .directory
        .parent()
        .ok_or_else(|| anyhow!("无效的缓存目录：{}", plan.directory.display()))?;
    let archive = cache.join("downloads").join(plan.asset);
    if !archive.is_file() || sha256_file(&archive)? != plan.digest {
        let partial = download(&plan.url, &archive, quiet)?;
        let actual = sha256_file(&partial)?;
        if actual != plan.digest {
            let _ = fs::remove_file(&partial);
            bail!(
                "{} 的 SHA-256 和固定值不符，已删除下载文件。\n  期望 {}\n  实际 {actual}",
                plan.asset,
                plan.digest
            );
        }
        // Another process may have finished the same download meanwhile;
        // both files are verified, so either can win.
        if fs::rename(&partial, &archive).is_err() {
            let _ = fs::remove_file(&partial);
            if !archive.is_file() || sha256_file(&archive)? != plan.digest {
                bail!("无法保存下载文件：{}", archive.display());
            }
        }
    }
    let unpack = cache.join(unique(&format!(".{}-unpack", plan.name)));
    let staging = cache.join(unique(&format!(".{}-{}", plan.name, plan.version)));
    let result = (|| -> Result<PathBuf> {
        extract(&archive, &unpack)?;
        let found = find_binary(&unpack, plan.name)?;
        fs::create_dir_all(&staging)?;
        let staged = staging.join(found.file_name().unwrap());
        fs::copy(&found, &staged)?;
        if let Some(parent) = found.parent() {
            for entry in fs::read_dir(parent)? {
                let entry = entry?;
                let file = entry.file_name().to_string_lossy().to_string();
                if (file.starts_with("LICENSE") || file.starts_with("NOTICE"))
                    && entry.file_type()?.is_file()
                {
                    fs::copy(entry.path(), staging.join(&file))?;
                }
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&staged)?.permissions();
            permissions.set_mode(permissions.mode() | 0o111);
            fs::set_permissions(&staged, permissions)?;
        }
        if plan.verify_version {
            let found = run_version(&staged);
            if found.as_deref() != Some(plan.version) {
                bail!(
                    "{} 运行后报告的版本是 {}，不是 {}。",
                    staged.display(),
                    found.unwrap_or_else(|| "None".into()),
                    plan.version
                );
            }
        }
        fs::write(staging.join(VERSION_MARKER), plan.version)?;
        if force && plan.directory.exists() {
            fs::remove_dir_all(&plan.directory)?;
        }
        // Publish the complete directory in one rename. If another process
        // published first, keep its copy.
        if fs::rename(&staging, &plan.directory).is_err() {
            let ready = fs::read_to_string(&marker).is_ok_and(|v| v.trim() == plan.version)
                && binary.is_file();
            if !ready {
                bail!("无法安装到 {}", plan.directory.display());
            }
        }
        Ok(binary.clone())
    })();
    let _ = fs::remove_dir_all(&unpack);
    let _ = fs::remove_dir_all(&staging);
    result
}

#[derive(Clone, Debug)]
pub struct ToolStatus {
    pub tool: String,
    pub required: String,
    pub found: Option<String>,
    pub path: Option<PathBuf>,
    pub source: Option<Source>,
    pub ok: bool,
    pub note: Option<String>,
}

pub fn tool_status(name: &str, root: &Path, env: &Environment) -> Result<ToolStatus> {
    let required = release(name)?.version.to_string();
    let located = locate(name, root, env);
    let found = located.as_ref().and_then(|(path, _)| run_version(path));
    Ok(ToolStatus {
        tool: name.to_string(),
        ok: found.as_deref() == Some(required.as_str()),
        required,
        found,
        path: located.as_ref().map(|(path, _)| path.clone()),
        source: located.map(|(_, source)| source),
        note: None,
    })
}

/// True or false when it can be checked, None when it cannot.
pub fn cjk_font_present() -> Option<bool> {
    if cfg!(windows) {
        let fonts =
            PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into()))
                .join("Fonts");
        return Some(CJK_FONT_FILES.iter().any(|name| fonts.join(name).exists()));
    }
    if let Some(fc_list) = Environment::current().which("fc-list") {
        let output = Command::new(fc_list)
            .args([":lang=zh", "family"])
            .stderr(Stdio::null())
            .output()
            .ok()?;
        return Some(!String::from_utf8_lossy(&output.stdout).trim().is_empty());
    }
    if cfg!(target_os = "macos") && Path::new("/System/Library/Fonts/PingFang.ttc").exists() {
        return Some(true);
    }
    None
}

pub fn describe(status: &ToolStatus) -> String {
    let mark = if status.ok { "ok " } else { "-- " };
    let found = status.found.as_deref().unwrap_or("未找到");
    let place = status.source.map_or("", Source::label);
    let path = status
        .path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    format!(
        "{mark}{:<7} 需要 {:<8} 找到 {found:<10} {place} {path}",
        status.tool, status.required
    )
    .trim_end()
    .to_string()
}

fn status_json(status: &ToolStatus) -> Value {
    let mut value = json!({
        "tool": status.tool,
        "required": status.required,
        "found": status.found,
        "path": status.path.as_ref().map(|p| p.display().to_string()),
        "source": status.source.map(Source::key),
        "ok": status.ok,
    });
    if let Some(note) = &status.note {
        value["note"] = json!(note);
    }
    value
}

/// `blogctl tools`: report, then download what is missing into the user cache.
pub fn run_setup(
    root: &Path,
    check: bool,
    force: bool,
    json: bool,
    proxy_prefix: Option<&str>,
) -> Result<bool> {
    let env = Environment::current();
    let prefix = download_prefix(proxy_prefix);
    let quiet = json;
    let (system, arch) = platform_key()?;
    let mut statuses = Vec::new();
    let mut installed = Vec::new();
    for release in &RELEASES {
        let name = release.name;
        let mut status = tool_status(name, root, &env)?;
        if !check && (force || !status.ok) {
            match status.source {
                Some(Source::Env) if !force => {
                    status.note = Some(format!(
                        "环境变量 {} 指向了版本不对的程序，先改或删掉它。",
                        name.to_uppercase()
                    ));
                }
                Some(Source::Tools) if !force => {
                    status.note = Some(format!(
                        "项目里的 .tools/{name} 版本不对，删掉它后会使用用户缓存里的版本。"
                    ));
                }
                _ => {
                    if !quiet {
                        eprintln!(
                            "安装 {name} {} 到 {}",
                            release.version,
                            env.cache_dir(name)?.display()
                        );
                    }
                    install(name, &env, &prefix, force, quiet)?;
                    installed.push(name);
                    status = tool_status(name, root, &env)?;
                }
            }
        }
        statuses.push(status);
    }
    let font = cjk_font_present();
    let ok = statuses.iter().all(|status| status.ok);
    if json {
        let report = json!({
            "root": root.display().to_string(),
            "platform": [system, arch],
            "tools": statuses.iter().map(status_json).collect::<Vec<_>>(),
            "installed": installed,
            "font": font,
            "ok": ok,
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        for status in &statuses {
            println!("{}", describe(status));
            if let Some(note) = &status.note {
                println!("   {note}");
            }
        }
        match font {
            Some(true) => println!("ok  中文字体 已找到"),
            Some(false) => println!("--  中文字体 没找到，Linux 可以装 fonts-noto-cjk"),
            None => println!("?   中文字体 这里没法检查，Typst 图里有中文时需要一种中文字体"),
        }
        if !installed.is_empty() {
            println!(
                "已安装到用户缓存：{}。构建和预览会自动使用这里的版本。",
                installed.join("、")
            );
        }
        if !ok {
            let missing: Vec<&str> = statuses
                .iter()
                .filter(|s| !s.ok)
                .map(|s| s.tool.as_str())
                .collect();
            let hint = if check {
                "。去掉 --check 可以下载 Zola 和 Typst。"
            } else {
                "。"
            };
            println!("还缺：{}{hint}", missing.join("、"));
        }
    }
    Ok(ok)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn temp(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(unique(&format!("tools-test-{label}")));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn every_supported_platform_has_both_tools_pinned() {
        for key in [
            ("windows", "x86_64"),
            ("windows", "aarch64"),
            ("linux", "x86_64"),
            ("linux", "aarch64"),
            ("linux-musl", "x86_64"),
            ("linux-musl", "aarch64"),
            ("darwin", "x86_64"),
            ("darwin", "aarch64"),
        ] {
            for name in ["zola", "typst"] {
                let (asset, digest) = asset_for(name, key).unwrap();
                assert!(asset.starts_with(name), "{asset}");
                if name == "zola" {
                    assert!(asset.contains(ZOLA_VERSION));
                }
                assert_eq!(digest.len(), 64);
                assert!(digest
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
            }
        }
    }

    #[test]
    fn musl_linux_falls_back_to_glibc_zola_only_without_a_musl_build() {
        assert_eq!(
            asset_for("zola", ("linux-musl", "aarch64")).unwrap().0,
            "zola-v0.23.4-aarch64-unknown-linux-gnu.tar.gz"
        );
        assert_eq!(
            asset_for("zola", ("linux-musl", "x86_64")).unwrap().0,
            "zola-v0.23.4-x86_64-unknown-linux-musl.tar.gz"
        );
    }

    #[test]
    fn version_parsing() {
        assert_eq!(parse_version("zola 0.23.4").as_deref(), Some("0.23.4"));
        assert_eq!(
            parse_version("typst 0.15.1 (unknown commit)").as_deref(),
            Some("0.15.1")
        );
        assert_eq!(parse_version("v22.19.0\n").as_deref(), Some("22.19.0"));
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn lookup_prefers_env_then_tools_then_cache_then_path() {
        let root = temp("lookup");
        let cache = root.join("cache");
        let path_dir = root.join("bin");
        fs::create_dir_all(&path_dir).unwrap();
        let on_path = path_dir.join(binary_name("zola"));
        fs::write(&on_path, b"").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&on_path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut env = Environment {
            overrides: vec![("ZOLA".into(), "custom-zola".into())],
            cache: Some(cache.clone()),
            path: Some(std::env::join_paths([&path_dir]).unwrap()),
        };
        assert_eq!(
            locate("zola", &root, &env),
            Some((PathBuf::from("custom-zola"), Source::Env))
        );
        env.overrides.clear();
        assert_eq!(
            locate("zola", &root, &env),
            Some((on_path.clone(), Source::Path))
        );
        // A cached copy counts only with its version marker.
        let cached = env.cache_dir("zola").unwrap();
        fs::create_dir_all(&cached).unwrap();
        fs::write(cached.join(binary_name("zola")), b"").unwrap();
        assert_eq!(locate("zola", &root, &env).unwrap().1, Source::Path);
        fs::write(cached.join(VERSION_MARKER), ZOLA_VERSION).unwrap();
        assert_eq!(locate("zola", &root, &env).unwrap().1, Source::Cache);
        let bundled = root.join(".tools").join("zola").join(binary_name("zola"));
        fs::create_dir_all(bundled.parent().unwrap()).unwrap();
        fs::write(&bundled, b"").unwrap();
        assert_eq!(
            locate("zola", &root, &env),
            Some((bundled.clone(), Source::Tools))
        );
        fs::remove_file(&bundled).unwrap();
        fs::remove_dir_all(&cached).unwrap();
        env.path = None;
        assert_eq!(locate("zola", &root, &env), None);
        fs::remove_dir_all(&root).unwrap();
    }

    fn write_zip(path: &Path, members: &[(&str, &[u8])]) {
        let mut bundle = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, data) in members {
            bundle
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            bundle.write_all(data).unwrap();
        }
        bundle.finish().unwrap();
    }

    fn tar_bytes(members: &[(&str, &[u8])]) -> Vec<u8> {
        let mut bundle = tar::Builder::new(Vec::new());
        for (name, data) in members {
            let mut header = tar::Header::new_gnu();
            // Write the raw name so unsafe paths reach the archive unchanged.
            let bytes = name.as_bytes();
            header.as_old_mut().name[..bytes.len()].copy_from_slice(bytes);
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_entry_type(tar::EntryType::Regular);
            header.set_cksum();
            bundle.append(&header, *data).unwrap();
        }
        bundle.into_inner().unwrap()
    }

    #[test]
    fn archives_that_escape_the_destination_are_rejected() {
        let folder = temp("escape");
        let bad_zip = folder.join("bad.zip");
        write_zip(&bad_zip, &[("../escape.txt", b"x")]);
        let error = extract(&bad_zip, &folder.join("out"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("越界路径"), "{error}");
        let bad_tar = folder.join("bad.tar.gz");
        let mut encoder =
            flate2::write::GzEncoder::new(fs::File::create(&bad_tar).unwrap(), Default::default());
        encoder
            .write_all(&tar_bytes(&[("/abs.txt", b"x")]))
            .unwrap();
        encoder.finish().unwrap();
        assert!(extract(&bad_tar, &folder.join("out2")).is_err());
        assert!(!folder.join("escape.txt").exists());
        assert!(!folder.join("out2").join("abs.txt").exists());
        for name in ["C:/x", "a\\..\\b", "a/../../b"] {
            assert!(check_member(&bad_zip, name).is_err(), "{name}");
        }
        assert!(check_member(&bad_zip, "typst-x/typst.exe").is_ok());
        fs::remove_dir_all(&folder).unwrap();
    }

    fn xz(data: &[u8]) -> Vec<u8> {
        let mut output = Vec::new();
        lzma_rs::xz_compress(&mut io::Cursor::new(data), &mut output).unwrap();
        output
    }

    #[test]
    fn extract_finds_the_binary_inside_a_nested_folder() {
        let folder = temp("nested");
        let archive = folder.join("typst.tar.xz");
        let binary = format!("typst-demo/{}", binary_name("typst"));
        let data = tar_bytes(&[("typst-demo/README.md", b"readme"), (&binary, b"bin")]);
        fs::write(&archive, xz(&data)).unwrap();
        let out = folder.join("out");
        extract(&archive, &out).unwrap();
        assert_eq!(
            fs::read(find_binary(&out, "typst").unwrap()).unwrap(),
            b"bin"
        );
        assert!(find_binary(&out, "zola").is_err());
        fs::remove_dir_all(&folder).unwrap();
    }

    /// Serve `body` to every request on a local port.
    fn serve(body: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut request = [0u8; 2048];
                let _ = stream.read(&mut request);
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        format!("http://{address}/")
    }

    #[test]
    fn install_verifies_hash_and_publishes_atomically() {
        let folder = temp("install");
        let archive = folder.join("source.zip");
        let binary = format!("zola-x/{}", binary_name("zola"));
        write_zip(
            &archive,
            &[(&binary, b"fake zola"), ("zola-x/LICENSE", b"license")],
        );
        let bytes = fs::read(&archive).unwrap();
        let digest = hex::encode(Sha256::digest(&bytes));
        let base = serve(bytes);
        let cache = folder.join("cache");
        let mut plan = InstallPlan {
            name: "zola",
            version: ZOLA_VERSION,
            asset: "zola-test.zip",
            digest: &digest,
            url: format!("{base}zola-test.zip"),
            directory: cache.join(format!("zola-{ZOLA_VERSION}")),
            verify_version: false,
        };
        let installed = install_plan(&plan, false, true).unwrap();
        assert_eq!(fs::read(&installed).unwrap(), b"fake zola");
        assert!(plan.directory.join("LICENSE").is_file());
        assert_eq!(
            fs::read_to_string(plan.directory.join(VERSION_MARKER)).unwrap(),
            ZOLA_VERSION
        );
        // No staging leftovers beside the published directory.
        let leftovers: Vec<_> = fs::read_dir(&cache)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with('.'))
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        // A wrong pinned hash deletes the download and fails.
        let wrong = "0".repeat(64);
        plan.digest = &wrong;
        plan.asset = "zola-other.zip";
        let error = install_plan(&plan, true, true).unwrap_err().to_string();
        assert!(error.contains("SHA-256 和固定值不符"), "{error}");
        assert!(!cache.join("downloads").join("zola-other.zip").exists());
        assert_eq!(fs::read(&installed).unwrap(), b"fake zola");
        fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn describe_matches_the_python_report_layout() {
        let status = ToolStatus {
            tool: "zola".into(),
            required: "0.23.4".into(),
            found: Some("0.23.4".into()),
            path: Some(PathBuf::from("/x/zola")),
            source: Some(Source::Path),
            ok: true,
            note: None,
        };
        assert_eq!(
            describe(&status),
            "ok zola    需要 0.23.4   找到 0.23.4     PATH /x/zola"
        );
        let missing = ToolStatus {
            found: None,
            path: None,
            source: None,
            ok: false,
            ..status
        };
        assert_eq!(describe(&missing), "-- zola    需要 0.23.4   找到 未找到");
    }
}
