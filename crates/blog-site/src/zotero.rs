//! Run the official Zotero desktop client with a private, project-local profile.
//!
//! No Zotero database or PDF is placed under site/. A system installation is also
//! usable through the workbench; this helper offers a reproducible local client.

use crate::paths::is_link;
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Map, Value};
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const VERSION: &str = "10.0.6";
const PUBLISHER: &str = "Corporation for Digital Scholarship";
const KEY_CHARACTERS: &str = "23456789ABCDEFGHIJKLMNPQRSTUVWXYZ";

fn download_url() -> String {
    format!("https://download.zotero.org/client/release/{VERSION}/Zotero-{VERSION}_win-x64.zip")
}

/// An absolute path with `.` and `..` removed lexically, like Python's `resolve`
/// once links are known to be absent.
fn normalize(path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let mut result = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other),
        }
    }
    Ok(result)
}

/// The Zotero helper for one blog root; everything it writes stays in `<root>/.tools`.
pub struct Zotero {
    pub root: PathBuf,
    pub directory: PathBuf,
}

/// The pieces of `start` that reach outside the process, replaceable in tests.
pub trait Launcher {
    fn status(&mut self, port: u16) -> Map<String, Value>;
    fn setup(&mut self, zotero: &Zotero) -> Result<PathBuf>;
    fn spawn(&mut self, zotero: &Zotero, command: &[OsString], hidden: bool) -> Result<u32>;
}

pub struct System;

impl Launcher for System {
    fn status(&mut self, port: u16) -> Map<String, Value> {
        status(port)
    }

    fn setup(&mut self, zotero: &Zotero) -> Result<PathBuf> {
        zotero.setup()
    }

    fn spawn(&mut self, zotero: &Zotero, command: &[OsString], hidden: bool) -> Result<u32> {
        spawn(&zotero.root, command, hidden)
    }
}

impl Zotero {
    pub fn new(root: &Path) -> Result<Self> {
        let root = normalize(root)?;
        let directory = root.join(".tools").join("zotero");
        Ok(Zotero { root, directory })
    }

    pub fn private_path(&self, path: &Path) -> Result<PathBuf> {
        let path = std::path::absolute(path)?;
        for parent in path.ancestors() {
            if parent.as_os_str().is_empty() {
                break;
            }
            if fs::symlink_metadata(parent).is_ok_and(|metadata| is_link(&metadata)) {
                bail!(
                    "Zotero private storage must not use links: {}",
                    parent.display()
                );
            }
        }
        let tools = normalize(&self.root.join(".tools"))?;
        let resolved = normalize(&path)?;
        if !resolved.starts_with(&tools) {
            bail!("Zotero managed files must stay within the project .tools directory");
        }
        Ok(resolved)
    }

    pub fn executable(&self) -> PathBuf {
        let client = self.directory.join("client");
        let direct = client.join("zotero.exe");
        if direct.is_file() {
            return direct;
        }
        let mut found: Vec<PathBuf> = fs::read_dir(&client)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path().join("zotero.exe"))
            .filter(|path| path.is_file())
            .collect();
        found.sort();
        found.into_iter().next().unwrap_or(direct)
    }

    /// Authenticate the official Windows executable before returning or launching it.
    pub fn verify_signature(&self, app: &Path) -> Result<PathBuf> {
        let app = self.private_path(app)?;
        let quoted = app.display().to_string().replace('\'', "''");
        let shell = which("pwsh.exe")
            .or_else(|| which("powershell.exe"))
            .ok_or_else(|| {
                anyhow!("PowerShell is required to verify the official Zotero signature")
            })?;
        // VS Code may pass PowerShell 7's module path to Windows PowerShell 5. Load
        // the chosen shell's own signed modules explicitly rather than autoloading.
        let command = format!(
            "Import-Module \"$PSHOME/Modules/Microsoft.PowerShell.Security/Microsoft.PowerShell.Security.psd1\" -ErrorAction Stop; \
             Import-Module \"$PSHOME/Modules/Microsoft.PowerShell.Utility/Microsoft.PowerShell.Utility.psd1\" -ErrorAction Stop; \
             $s = Get-AuthenticodeSignature -LiteralPath '{quoted}'; \
             @{{status=$s.Status.ToString(); subject=$s.SignerCertificate.Subject}} | ConvertTo-Json -Compress"
        );
        let mut process = Command::new(shell);
        process
            .args(["-NoProfile", "-NonInteractive", "-Command", &command])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        no_window(&mut process);
        let (success, stdout) = run_with_timeout(&mut process, Duration::from_secs(30))?;
        check_signature(success, &stdout)?;
        Ok(app)
    }

    pub fn check_zip_member(
        &self,
        client: &Path,
        raw_name: &str,
        unix_mode: Option<u32>,
    ) -> Result<PathBuf> {
        let unsafe_path = || anyhow!("Unsafe path in the Zotero ZIP");
        let original = raw_name;
        let mut name = original.split('\0').next().unwrap_or("").to_string();
        // Python's zipfile turns the platform separator into '/' before any check.
        if cfg!(windows) {
            name = name.replace('\\', "/");
        }
        let parts: Vec<&str> = name.trim_end_matches('/').split('/').collect();
        // Reject Windows normalization, drive paths, ADS and device names before
        // extraction normalizes them into a different destination.
        if name.is_empty()
            || name.contains('\\')
            || name.contains(':')
            || original.contains('\0')
            || name.starts_with('/')
            || parts.iter().any(|part| {
                ["", ".", ".."].contains(part) || part.trim_end_matches([' ', '.']) != *part
            })
        {
            return Err(unsafe_path());
        }
        let device = |part: &str| {
            let stem = part.split('.').next().unwrap_or("").to_uppercase();
            ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
                || (stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && ('1'..='9').contains(&stem.chars().nth(3).unwrap()))
        };
        if parts.iter().any(|part| device(part)) {
            bail!("Unsafe Windows device path in the Zotero ZIP");
        }
        let destination = self.private_path(
            &parts
                .iter()
                .fold(client.to_path_buf(), |path, part| path.join(part)),
        )?;
        if !destination.starts_with(normalize(client)?) {
            return Err(unsafe_path());
        }
        if unix_mode.is_some_and(|mode| mode & 0o170000 == 0o120000) {
            bail!("Links are not accepted in the Zotero ZIP");
        }
        Ok(destination)
    }

    pub fn setup(&self) -> Result<PathBuf> {
        if !cfg!(windows) {
            bail!("On macOS/Linux, install Zotero from https://www.zotero.org/download/ and enable its local API.");
        }
        fs::create_dir_all(self.private_path(&self.directory)?)?;
        let app = self.executable();
        if app.is_file() {
            return self.verify_signature(&app);
        }
        let archive =
            self.private_path(&self.directory.join(format!("Zotero-{VERSION}_win-x64.zip")))?;
        if !archive.is_file() {
            eprintln!("Downloading official Zotero {VERSION}...");
            let partial = crate::tools::download(&download_url(), &archive, true)?;
            if let Err(error) = fs::rename(&partial, &archive) {
                let _ = fs::remove_file(&partial);
                return Err(error.into());
            }
        }
        let client = self.private_path(&self.directory.join("client"))?;
        fs::create_dir_all(&client)?;
        self.extract(&archive, &client)?;
        let app = self.executable();
        if !app.is_file() {
            bail!("The official ZIP did not contain zotero.exe");
        }
        self.verify_signature(&app)
    }

    /// Validate every member before writing any of them.
    fn extract(&self, archive: &Path, client: &Path) -> Result<()> {
        let mut bundle = zip::ZipArchive::new(fs::File::open(archive)?)?;
        let mut plan = Vec::new();
        for index in 0..bundle.len() {
            let member = bundle.by_index_raw(index)?;
            let name = String::from_utf8_lossy(member.name_raw()).into_owned();
            let destination = self.check_zip_member(client, &name, member.unix_mode())?;
            plan.push((index, destination, member.is_dir()));
        }
        for (index, destination, directory) in plan {
            if directory {
                fs::create_dir_all(&destination)?;
                continue;
            }
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut member = bundle.by_index(index)?;
            let mut output = fs::File::create(&destination)?;
            std::io::copy(&mut member, &mut output)?;
        }
        Ok(())
    }

    pub fn prepare_profile(&self, profile: &Path, port: u16) -> Result<PathBuf> {
        let profile = self.private_path(profile)?;
        fs::create_dir_all(&profile)?;
        let name = profile
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let library_name = if name == "profile" {
            "library".to_string()
        } else {
            format!("{name}-library")
        };
        let library =
            self.private_path(&profile.parent().unwrap_or(&profile).join(library_name))?;
        fs::create_dir_all(&library)?;
        let preferences = [
            ("extensions.zotero.httpServer.enabled", json!(true)),
            ("extensions.zotero.httpServer.localAPI.enabled", json!(true)),
            ("extensions.zotero.httpServer.port", json!(port)),
            ("extensions.zotero.useDataDir", json!(true)),
            (
                "extensions.zotero.dataDir",
                json!(library.display().to_string()),
            ),
            ("extensions.zotero.firstRun2", json!(false)),
        ];
        // Only our managed profile is configured. Existing user profiles are untouched.
        let text: String = preferences
            .iter()
            .map(|(name, value)| {
                format!(
                    "user_pref({}, {});\n",
                    python_json(&json!(name)),
                    python_json(value)
                )
            })
            .collect();
        fs::write(self.private_path(&profile.join("user.js"))?, text)?;
        Ok(profile)
    }

    pub fn start(
        &self,
        port: u16,
        profile: Option<&Path>,
        uri: Option<&str>,
        hidden: bool,
        launcher: &mut dyn Launcher,
    ) -> Result<Map<String, Value>> {
        check_port(port.into())?;
        if let Some(uri) = uri.filter(|uri| !uri.is_empty()) {
            validate_uri(uri)?;
        }
        let uri = uri.filter(|uri| !uri.is_empty());
        let isolated = profile.is_some();
        let profile = profile.map(|path| self.private_path(path)).transpose()?;
        let current = launcher.status(port);
        let running = current.get("running") == Some(&Value::Bool(true));
        if isolated && running {
            bail!("The isolated test port is already used by Zotero; use another port or the running test instance directly");
        }
        if running && uri.is_none() {
            return Ok(current);
        }
        let app = launcher.setup(self)?;
        let profile = self.prepare_profile(
            &profile.unwrap_or_else(|| self.directory.join("profile")),
            port,
        )?;
        let mut command: Vec<OsString> = vec![app.into_os_string()];
        if isolated {
            command.push("-no-remote".into());
        }
        command.push("-profile".into());
        command.push(profile.clone().into_os_string());
        if let Some(uri) = uri {
            command.push("-url".into());
            command.push(uri.into());
        }
        let pid = launcher.spawn(self, &command, hidden)?;
        let mut result = Map::new();
        result.insert("started".into(), json!(true));
        result.insert("pid".into(), json!(pid));
        result.insert("profile".into(), json!(profile.display().to_string()));
        result.insert("port".into(), json!(port));
        Ok(result)
    }
}

pub fn check_port(port: u32) -> Result<u16> {
    if !(1024..=65535).contains(&port) {
        bail!("Port must be between 1024 and 65535");
    }
    Ok(port as u16)
}

fn check_signature(success: bool, stdout: &str) -> Result<()> {
    let signature: Value = serde_json::from_str(stdout.trim())
        .map_err(|_| anyhow!("Could not verify the downloaded Zotero executable signature"))?;
    let status = signature.get("status").and_then(Value::as_str);
    let subject = signature
        .get("subject")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !success || !signature.is_object() || status != Some("Valid") || !subject.contains(PUBLISHER)
    {
        bail!("Zotero executable must have a valid {PUBLISHER} signature");
    }
    Ok(())
}

fn is_key(value: &str) -> bool {
    value.chars().count() == 8 && value.chars().all(|c| KEY_CHARACTERS.contains(c))
}

fn is_digits(value: &str) -> bool {
    !value.is_empty() && value.chars().all(crate::markdown::is_digit)
}

fn positive(value: &str) -> bool {
    is_digits(value) && value.parse::<u64>().is_ok_and(|n| n > 0)
}

/// Python's `parse_qs(query, keep_blank_values=True)`.
fn parse_query(query: &str) -> Vec<(String, Vec<String>)> {
    let decode = |text: &str| crate::check::url::unquote(&text.replace('+', " "));
    let mut result: Vec<(String, Vec<String>)> = Vec::new();
    for piece in query.split('&').filter(|piece| !piece.is_empty()) {
        let (name, value) = piece.split_once('=').unwrap_or((piece, ""));
        let (name, value) = (decode(name), decode(value));
        match result.iter_mut().find(|(existing, _)| *existing == name) {
            Some((_, values)) => values.push(value),
            None => result.push((name, vec![value])),
        }
    }
    result
}

/// Only Zotero library and PDF links with valid local item keys are accepted.
pub fn validate_uri(uri: &str) -> Result<()> {
    let invalid =
        || anyhow!("Only Zotero library and PDF links with valid local item keys are accepted");
    if uri.chars().any(|c| (c as u32) < 32) {
        return Err(invalid());
    }
    // urlsplit strips surrounding C0 controls and spaces before parsing.
    let text = uri.trim_matches(|c: char| c as u32 <= 32);
    let (scheme, rest) = match text.find(':') {
        Some(colon)
            if colon > 0
                && text.as_bytes()[0].is_ascii_alphabetic()
                && text[..colon]
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b)) =>
        {
            (text[..colon].to_ascii_lowercase(), &text[colon + 1..])
        }
        _ => (String::new(), text),
    };
    let (netloc, rest) = match rest.strip_prefix("//") {
        Some(after) => {
            let end = after.find(['/', '?', '#']).unwrap_or(after.len());
            (&after[..end], &after[end..])
        }
        None => ("", rest),
    };
    let (rest, fragment) = rest.split_once('#').unwrap_or((rest, ""));
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let components: Vec<&str> = path.trim_matches('/').split('/').collect();
    let group_prefix =
        components.len() >= 2 && components[0] == "groups" && positive(components[1]);
    let library_path = components == ["library"] || (components.len() == 2 && group_prefix);
    let item_path = (components.len() == 3
        && components[0] == "library"
        && components[1] == "items"
        && is_key(components[2]))
        || (components.len() == 4
            && group_prefix
            && components[2] == "items"
            && is_key(components[3]));
    let parameters = parse_query(query);
    let value = |name: &str| {
        parameters
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, values)| values[0].as_str())
    };
    let valid_parameters = parameters
        .iter()
        .all(|(name, values)| ["page", "annotation"].contains(&name.as_str()) && values.len() == 1)
        && value("page").is_none_or(positive)
        && value("annotation").is_none_or(is_key);
    if scheme != "zotero"
        || !["select", "open-pdf"].contains(&netloc)
        || !fragment.is_empty()
        || !(item_path || (netloc == "select" && library_path))
        || !valid_parameters
    {
        return Err(invalid());
    }
    Ok(())
}

pub fn status(port: u16) -> Map<String, Value> {
    let agent = ureq::AgentBuilder::new()
        .try_proxy_from_env(false)
        .redirects(0)
        .timeout(Duration::from_secs(2))
        .build();
    let response = agent
        .get(&format!("http://127.0.0.1:{port}/api/"))
        .set("Zotero-API-Version", "3")
        .call();
    let response = match response {
        Ok(response) => response,
        Err(ureq::Error::Status(_, response)) => response,
        Err(ureq::Error::Transport(_)) => {
            let mut state = Map::new();
            state.insert("running".into(), json!(false));
            state.insert("apiEnabled".into(), json!(false));
            state.insert("port".into(), json!(port));
            return state;
        }
    };
    let header = |name: &str| response.header(name).map(str::to_string);
    let identified = header("X-Zotero-Version").is_some_and(|v| !v.is_empty())
        && header("Zotero-API-Version").is_some_and(|v| !v.is_empty());
    let mut state = Map::new();
    state.insert("running".into(), json!(identified));
    if (200..300).contains(&response.status()) {
        state.insert("apiEnabled".into(), json!(identified));
        state.insert("apiVersion".into(), json!(header("Zotero-API-Version")));
        state.insert("serverID".into(), json!(header("Zotero-Server-ID")));
    } else {
        // Like urllib: any non-2xx answer, redirects included, is an HTTP error.
        state.insert("apiEnabled".into(), json!(false));
        state.insert("status".into(), json!(response.status()));
    }
    state.insert("port".into(), json!(port));
    state
}

/// Python `json.dumps` with its default separators and ASCII escaping.
pub fn python_json(value: &Value) -> String {
    match value {
        Value::String(text) => {
            let mut output = String::from("\"");
            for c in text.chars() {
                match c {
                    '"' => output.push_str("\\\""),
                    '\\' => output.push_str("\\\\"),
                    '\n' => output.push_str("\\n"),
                    '\r' => output.push_str("\\r"),
                    '\t' => output.push_str("\\t"),
                    '\u{8}' => output.push_str("\\b"),
                    '\u{c}' => output.push_str("\\f"),
                    c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                        let mut units = [0u16; 2];
                        for unit in c.encode_utf16(&mut units) {
                            output.push_str(&format!("\\u{unit:04x}"));
                        }
                    }
                    c => output.push(c),
                }
            }
            output.push('"');
            output
        }
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(python_json).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(k, v)| format!("{}: {}", python_json(&json!(k)), python_json(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => other.to_string(),
    }
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

fn no_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}

fn run_with_timeout(command: &mut Command, timeout: Duration) -> Result<(bool, String)> {
    let mut child = command.spawn()?;
    let mut stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Could not verify the downloaded Zotero executable signature");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let bytes = reader.join().unwrap_or_default();
    Ok((
        status.success(),
        String::from_utf8_lossy(&bytes).into_owned(),
    ))
}

/// Quote one argument by the rules MSVC programs use to split a command line.
#[cfg_attr(not(windows), allow(dead_code))]
fn quote_argument(argument: &str) -> String {
    if !argument.is_empty() && !argument.contains([' ', '\t', '\n', '\x0b', '"']) {
        return argument.to_string();
    }
    let mut quoted = String::from("\"");
    let mut backslashes = 0;
    for c in argument.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                quoted.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.extend(std::iter::repeat_n('\\', backslashes));
                quoted.push(c);
                backslashes = 0;
            }
        }
    }
    quoted.extend(std::iter::repeat_n('\\', backslashes * 2));
    quoted.push('"');
    quoted
}

fn spawn(root: &Path, command: &[OsString], hidden: bool) -> Result<u32> {
    #[cfg(windows)]
    if hidden {
        return spawn_hidden(root, command);
    }
    let _ = hidden;
    let child = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(child.id())
}

/// Start a GUI program with its main window hidden (STARTF_USESHOWWINDOW, SW_HIDE).
#[cfg(windows)]
fn spawn_hidden(root: &Path, command: &[OsString]) -> Result<u32> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        CreateProcessW, PROCESS_INFORMATION, STARTF_USESHOWWINDOW, STARTUPINFOW,
    };
    let line: Vec<String> = command
        .iter()
        .map(|part| quote_argument(&part.to_string_lossy()))
        .collect();
    let mut line: Vec<u16> = line.join(" ").encode_utf16().chain(Some(0)).collect();
    let directory: Vec<u16> = root.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: all pointers refer to live, NUL-terminated buffers or zeroed
    // structures owned by this frame; returned handles are closed below.
    unsafe {
        let mut startup: STARTUPINFOW = std::mem::zeroed();
        startup.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        startup.dwFlags = STARTF_USESHOWWINDOW;
        startup.wShowWindow = 0;
        let mut information: PROCESS_INFORMATION = std::mem::zeroed();
        let created = CreateProcessW(
            std::ptr::null(),
            line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            0,
            std::ptr::null(),
            directory.as_ptr(),
            &startup,
            &mut information,
        );
        if created == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        CloseHandle(information.hThread);
        CloseHandle(information.hProcess);
        Ok(information.dwProcessId)
    }
}

/// `blogctl zotero <action>`: the JSON line the command prints.
pub fn run(
    root: &Path,
    action: &str,
    port: u32,
    profile: Option<&Path>,
    uri: Option<&str>,
    hidden: bool,
) -> Result<String> {
    let port = check_port(port)?;
    let zotero = Zotero::new(root)?;
    let value = match action {
        "setup" => {
            let mut result = Map::new();
            result.insert(
                "executable".into(),
                json!(zotero.setup()?.display().to_string()),
            );
            result.insert("version".into(), json!(VERSION));
            result
        }
        "status" => status(port),
        "start" => zotero.start(port, profile, uri, hidden, &mut System)?,
        other => bail!("unknown action {other}"),
    };
    Ok(python_json(&Value::Object(value)))
}

#[cfg(test)]
mod tests;
