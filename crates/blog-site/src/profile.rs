//! Build the website profile and resume from profile/cv.typ using Typst itself.
//!
//! Generated data and PDFs are staged first: a compiler or schema error never
//! replaces the last successful website/profile outputs. No regex parsing of Typst.

use crate::references::read_utf8_sig;
use crate::tools::tool_path;
use anyhow::{anyhow, bail, Result};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{LazyLock, Mutex};

const SCALARS: [&str; 9] = [
    "display_name",
    "english_name",
    "intro",
    "current_research",
    "research_interests",
    "github",
    "email",
    "location",
    "tagline",
];
const ARRAYS: [&str; 3] = ["education", "experiences", "honors"];

/// Python's `repr` of a sorted list of keys, as in `['a', 'b']`.
fn key_list(mut keys: Vec<&str>) -> String {
    keys.sort();
    let quoted: Vec<String> = keys
        .iter()
        .map(|key| {
            if key.contains('\'') && !key.contains('"') {
                format!("\"{key}\"")
            } else {
                format!("'{}'", key.replace('\\', "\\\\").replace('\'', "\\'"))
            }
        })
        .collect();
    format!("[{}]", quoted.join(", "))
}

/// Exactly `required` (plus `optional`) keys, all with string values.
fn strings(
    value: &Map<String, Value>,
    required: &[&str],
    optional: &[&str],
    place: &str,
) -> Result<()> {
    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|key| !value.contains_key(*key))
        .collect();
    let unknown: Vec<&str> = value
        .keys()
        .map(String::as_str)
        .filter(|key| !required.contains(key) && !optional.contains(key))
        .collect();
    if !missing.is_empty() || !unknown.is_empty() {
        bail!(
            "{place}: missing fields {}, unknown fields {}",
            key_list(missing),
            key_list(unknown)
        );
    }
    for (key, item) in value {
        if !item.is_string() {
            bail!("{place}.{key}: use a quoted Typst string");
        }
    }
    Ok(())
}

fn dictionary<'a>(value: &'a Value, place: &str) -> Result<&'a Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| anyhow!("{place}: expected a dictionary"))
}

/// The parts of Python's `urlsplit` used to accept a public web link.
fn web_url(value: &str, place: &str) -> Result<()> {
    let mut scheme = "";
    let mut rest = value;
    if let Some(colon) = value.find(':') {
        let candidate = &value[..colon];
        if colon > 0
            && candidate.as_bytes()[0].is_ascii_alphabetic()
            && candidate
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
        {
            scheme = candidate;
            rest = &value[colon + 1..];
        }
    }
    let netloc = rest
        .strip_prefix("//")
        .map(|after| &after[..after.find(['/', '?', '#']).unwrap_or(after.len())])
        .unwrap_or("");
    let scheme = scheme.to_ascii_lowercase();
    if !["https", "http"].contains(&scheme.as_str())
        || netloc.is_empty()
        || value.chars().any(char::is_whitespace)
    {
        bail!("{place}: expected an absolute http(s) URL");
    }
    Ok(())
}

/// Validate the public schema; never pass unknown/private fields to Zola.
pub fn validate_profile(value: &Value) -> Result<()> {
    let version_ok = match value.get("schema_version") {
        Some(Value::Number(n)) => n.as_f64() == Some(1.0),
        Some(Value::Bool(b)) => *b,
        _ => false,
    };
    let Some(profile) = value.as_object().filter(|_| version_ok) else {
        bail!("Expected profile metadata with schema_version: 1");
    };
    let unknown: Vec<&str> = profile
        .keys()
        .map(String::as_str)
        .filter(|key| !SCALARS.contains(key) && !ARRAYS.contains(key) && *key != "schema_version")
        .collect();
    if !unknown.is_empty() {
        bail!("Unknown public profile fields: {}", key_list(unknown));
    }
    let scalars: Map<String, Value> = SCALARS
        .iter()
        .filter_map(|key| profile.get(*key).map(|v| (key.to_string(), v.clone())))
        .collect();
    strings(&scalars, &SCALARS, &[], "profile")?;
    for key in ARRAYS {
        if !profile.get(key).is_some_and(Value::is_array) {
            bail!("profile.{key}: expected an array");
        }
    }
    web_url(profile["github"].as_str().unwrap(), "profile.github")?;
    let email = profile["email"].as_str().unwrap();
    if !email.contains('@')
        || email.chars().any(char::is_whitespace)
        || email.contains(['?', '&', '#', ':'])
    {
        bail!("profile.email: expected a plain email address");
    }
    for (i, entry) in profile["education"].as_array().unwrap().iter().enumerate() {
        let place = format!("education[{i}]");
        strings(
            dictionary(entry, &place)?,
            &["degree", "school", "date", "detail"],
            &[],
            &place,
        )?;
    }
    for (i, entry) in profile["experiences"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let place = format!("experiences[{i}]");
        let entry = dictionary(entry, &place)?;
        let fields: Map<String, Value> = entry
            .iter()
            .filter(|(k, _)| *k != "bullets")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        strings(
            &fields,
            &[
                "kind",
                "org",
                "role",
                "date",
                "venue",
                "status",
                "paper_title",
                "web_title",
                "summary",
            ],
            &["paper_url"],
            &place,
        )?;
        if !["internship", "research"].contains(&entry["kind"].as_str().unwrap()) {
            bail!("{place}.kind: use internship or research");
        }
        let bullets_ok = entry
            .get("bullets")
            .and_then(Value::as_array)
            .is_some_and(|bullets| bullets.iter().all(Value::is_string));
        if !bullets_ok {
            bail!("{place}.bullets: expected an array of strings");
        }
        if let Some(url) = entry
            .get("paper_url")
            .and_then(Value::as_str)
            .filter(|u| !u.is_empty())
        {
            web_url(url, &format!("{place}.paper_url"))?;
        }
    }
    for (i, entry) in profile["honors"].as_array().unwrap().iter().enumerate() {
        let place = format!("honors[{i}]");
        strings(dictionary(entry, &place)?, &["year", "title"], &[], &place)?;
    }
    Ok(())
}

/// Run Typst, returning stdout. Errors carry only Typst's own diagnostics,
/// never the command line (which may contain a local phone number).
fn run(typst: &Path, args: &[OsString]) -> Result<String> {
    let output = Command::new(typst).args(args).output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let subcommand = args
        .first()
        .map(|a| a.to_string_lossy().to_string())
        .unwrap_or_default();
    if !output.status.success() {
        bail!("Typst {subcommand} failed:\n{}", stderr.trim());
    }
    if !stderr.trim().is_empty() {
        println!("{}", stderr.trim());
    }
    String::from_utf8(output.stdout)
        .map_err(|_| anyhow!("Typst {subcommand} printed invalid UTF-8"))
}

type FontKey = (PathBuf, String, Option<OsString>);
static FONT_INPUTS: LazyLock<Mutex<HashMap<FontKey, Vec<OsString>>>> =
    LazyLock::new(Default::default);

/// Keep original Noto fonts when installed, with explicit Windows fallbacks.
fn font_inputs(typst: &Path, font_path: &Path) -> Result<Vec<OsString>> {
    let key = (
        typst.to_path_buf(),
        font_path.display().to_string(),
        std::env::var_os("TYPST_FONT_PATHS"),
    );
    if let Some(cached) = FONT_INPUTS.lock().unwrap().get(&key) {
        return Ok(cached.clone());
    }
    let mut args: Vec<OsString> = vec!["--font-path".into(), font_path.into()];
    let mut fonts_args = vec![OsString::from("fonts")];
    fonts_args.extend(args.iter().cloned());
    let listing = run(typst, &fonts_args)?;
    let available: Vec<&str> = listing.lines().collect();
    for (input, candidates) in [
        (
            "body-font",
            [
                "Noto Sans CJK SC",
                "Source Han Sans SC",
                "Microsoft YaHei",
                "DengXian",
            ],
        ),
        (
            "serif-font",
            [
                "Noto Serif CJK SC",
                "Source Han Serif SC",
                "SimSun",
                "Microsoft YaHei",
            ],
        ),
    ] {
        let selected = candidates.iter().find(|font| available.contains(font)).ok_or_else(|| {
            anyhow!("No Chinese resume font found. Install Noto Sans/Serif CJK SC (CI: fonts-noto-cjk).")
        })?;
        args.push("--input".into());
        args.push(format!("{input}={selected}").into());
    }
    FONT_INPUTS.lock().unwrap().insert(key, args.clone());
    Ok(args)
}

fn replace_if_changed(path: &Path, content: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    if fs::read(path).is_ok_and(|existing| existing == content) {
        return Ok(());
    }
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    fs::write(&temporary, content)?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    Ok(())
}

/// `json.dumps(value, ensure_ascii=False, indent=2)`.
pub fn python_dumps_indent(value: &Value) -> String {
    fn write(value: &Value, level: usize, output: &mut String) {
        let indent = |n: usize| "  ".repeat(n);
        match value {
            Value::Array(items) if !items.is_empty() => {
                output.push_str("[\n");
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        output.push_str(",\n");
                    }
                    output.push_str(&indent(level + 1));
                    write(item, level + 1, output);
                }
                output.push('\n');
                output.push_str(&indent(level));
                output.push(']');
            }
            Value::Object(map) if !map.is_empty() => {
                output.push_str("{\n");
                for (index, (key, item)) in map.iter().enumerate() {
                    if index > 0 {
                        output.push_str(",\n");
                    }
                    output.push_str(&indent(level + 1));
                    output.push_str(&serde_json::to_string(key).unwrap());
                    output.push_str(": ");
                    write(item, level + 1, output);
                }
                output.push('\n');
                output.push_str(&indent(level));
                output.push('}');
            }
            Value::Array(_) => output.push_str("[]"),
            Value::Object(_) => output.push_str("{}"),
            other => output.push_str(&other.to_string()),
        }
    }
    let mut output = String::new();
    write(value, 0, &mut output);
    output
}

/// A directory removed when dropped.
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temp_dir_in(parent: &Path) -> Result<TempDir> {
    for attempt in 0..1000u32 {
        let path = parent.join(format!("profile-build-{}-{attempt}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(TempDir(path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    bail!("cannot create a staging directory in {}", parent.display())
}

pub fn build_profile(root: &Path, include_private: bool) -> Result<Value> {
    let root = std::path::absolute(root)?;
    let source = root.join("profile").join("cv.typ");
    if !source.is_file() {
        bail!("Missing resume source: {}", source.display());
    }
    let typst = tool_path("typst", &root)?;
    let source_dir = source.parent().unwrap().to_path_buf();
    let mut options: Vec<OsString> = vec!["--root".into(), source_dir.clone().into()];
    options.extend(font_inputs(&typst, &source_dir)?);
    // Public extraction and compilation NEVER receive local contact inputs.
    let mut eval: Vec<OsString> = vec![
        "eval".into(),
        "query(<profile>).map(it => it.value)".into(),
        "--in".into(),
        source.clone().into(),
    ];
    eval.extend(options.iter().cloned());
    let values: Value = serde_json::from_str(&run(&typst, &eval)?)?;
    let profile = match values {
        Value::Array(mut values) if values.len() == 1 => values.remove(0),
        _ => bail!("cv.typ must emit exactly one #metadata(profile) <profile>"),
    };
    validate_profile(&profile)?;
    let mut phone = String::new();
    let local_contact = root.join("profile").join("contact.local.json");
    if include_private && local_contact.exists() {
        let contact: Value = serde_json::from_str(&read_utf8_sig(&local_contact)?)?;
        let valid = contact.as_object().filter(|c| {
            c.keys().all(|k| k == "phone") && c.get("phone").is_none_or(Value::is_string)
        });
        let Some(contact) = valid else {
            bail!("contact.local.json must contain only a phone string");
        };
        phone = contact
            .get("phone")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
    }
    let output = root.join("output").join("pdf");
    fs::create_dir_all(&output)?;
    let staging = temp_dir_in(&output)?;
    let public_pdf = staging.0.join("public.pdf");
    let mut compile: Vec<OsString> = vec![
        "compile".into(),
        source.clone().into(),
        public_pdf.clone().into(),
    ];
    compile.extend(options.iter().cloned());
    run(&typst, &compile)?;
    let public_bytes = fs::read(&public_pdf)?;
    if !public_bytes.starts_with(b"%PDF-") {
        bail!("Typst did not produce a PDF");
    }
    let mut private_bytes = public_bytes.clone();
    if include_private && !phone.is_empty() {
        let private_pdf = staging.0.join("private.pdf");
        let mut compile: Vec<OsString> = vec![
            "compile".into(),
            source.clone().into(),
            private_pdf.clone().into(),
        ];
        compile.extend(options.iter().cloned());
        compile.push("--input".into());
        compile.push(format!("phone={phone}").into());
        run(&typst, &compile)?;
        private_bytes = fs::read(&private_pdf)?;
    }
    // All validation and compiles have succeeded before changing served data.
    replace_if_changed(
        &root.join("site").join("static").join("resume.pdf"),
        &public_bytes,
    )?;
    replace_if_changed(&output.join("resume-public.pdf"), &public_bytes)?;
    if include_private {
        replace_if_changed(&output.join("resume.pdf"), &private_bytes)?;
    }
    let data = python_dumps_indent(&profile) + "\n";
    replace_if_changed(
        &root.join("site").join("data").join("profile.json"),
        data.as_bytes(),
    )?;
    drop(staging);
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{locate, Environment};

    const OUTPUTS: [&str; 4] = [
        "site/data/profile.json",
        "site/static/resume.pdf",
        "output/pdf/resume.pdf",
        "output/pdf/resume-public.pdf",
    ];

    /// A copy of the repository's profile/, or None when Typst is unavailable.
    fn fixture(label: &str) -> Option<PathBuf> {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let env = Environment::current();
        let typst = locate("typst", &repo, &env)?;
        crate::tools::run_version(&typst.0)?;
        let root =
            std::env::temp_dir().join(format!("profile-test-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("profile")).unwrap();
        for entry in fs::read_dir(repo.join("profile")).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type().unwrap().is_file()
                && name != "contact.local.json"
                && !name.ends_with(".pdf")
            {
                fs::copy(entry.path(), root.join("profile").join(&name)).unwrap();
            }
        }
        Some(root)
    }

    fn snapshot(root: &Path) -> Vec<Vec<u8>> {
        OUTPUTS
            .iter()
            .map(|name| fs::read(root.join(name)).unwrap())
            .collect()
    }

    #[test]
    fn typst_edits_update_data_and_recover_after_errors() {
        let Some(root) = fixture("edits") else { return };
        let source = root.join("profile/cv.typ");
        let original = fs::read_to_string(&source).unwrap();
        let first = build_profile(&root, true).unwrap();
        let baseline = snapshot(&root);
        fs::write(
            root.join("profile/name.typ"),
            "#let alias = \"Pipeline Probe\"\n",
        )
        .unwrap();
        let name = first["display_name"].as_str().unwrap();
        let changed = format!(
            "#import \"name.typ\": alias\n{}",
            original.replacen(
                &format!("display_name: \"{name}\""),
                "display_name: alias + \" Updated\"",
                1,
            )
        );
        assert_ne!(
            changed.len(),
            original.len() + "#import \"name.typ\": alias\n".len()
        );
        fs::write(&source, &changed).unwrap();
        let updated = build_profile(&root, true).unwrap();
        assert_eq!(updated["display_name"], "Pipeline Probe Updated");
        let data: Value =
            serde_json::from_slice(&fs::read(root.join("site/data/profile.json")).unwrap())
                .unwrap();
        assert_eq!(data["display_name"], updated["display_name"]);
        let latest = snapshot(&root);
        assert_ne!(baseline[1], latest[1]);
        fs::write(&source, format!("{changed}\n#let broken = (")).unwrap();
        let error = build_profile(&root, true).unwrap_err().to_string();
        assert!(error.starts_with("Typst eval failed"), "{error}");
        assert_eq!(
            latest,
            snapshot(&root),
            "A Typst error must preserve all last-good outputs"
        );
        fs::write(&source, &original).unwrap();
        assert_eq!(
            build_profile(&root, true).unwrap()["display_name"],
            first["display_name"]
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn public_schema_rejects_private_fields_and_unsafe_links() {
        let Some(root) = fixture("schema") else {
            return;
        };
        let data = build_profile(&root, false).unwrap();
        let mut private = data.clone();
        private["phone"] = "must not publish".into();
        assert!(validate_profile(&private)
            .unwrap_err()
            .to_string()
            .contains("Unknown public profile fields"));
        let mut unsafe_link = data.clone();
        unsafe_link["experiences"][0]["paper_url"] = "javascript:alert(1)".into();
        assert!(validate_profile(&unsafe_link).is_err());
        let mut missing = data.clone();
        missing.as_object_mut().unwrap().remove("email");
        assert_eq!(
            validate_profile(&missing).unwrap_err().to_string(),
            "profile: missing fields ['email'], unknown fields []"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn schema_rules_without_typst() {
        let base = serde_json::json!({
            "schema_version": 1, "display_name": "A", "english_name": "A", "intro": "", "current_research": "",
            "research_interests": "", "github": "https://github.com/a", "email": "a@b.org", "location": "",
            "tagline": "", "education": [], "experiences": [], "honors": []
        });
        validate_profile(&base).unwrap();
        let mut wrong = base.clone();
        wrong["schema_version"] = 2.into();
        assert!(validate_profile(&wrong).is_err());
        let mut email = base.clone();
        email["email"] = "mailto:a@b.org".into();
        assert_eq!(
            validate_profile(&email).unwrap_err().to_string(),
            "profile.email: expected a plain email address"
        );
        let mut number = base.clone();
        number["intro"] = 3.into();
        assert_eq!(
            validate_profile(&number).unwrap_err().to_string(),
            "profile.intro: use a quoted Typst string"
        );
        let mut honors = base.clone();
        honors["honors"] = serde_json::json!([{"year": "2020"}]);
        assert_eq!(
            validate_profile(&honors).unwrap_err().to_string(),
            "honors[0]: missing fields ['title'], unknown fields []"
        );
        let mut github = base;
        github["github"] = "github.com/a".into();
        assert_eq!(
            validate_profile(&github).unwrap_err().to_string(),
            "profile.github: expected an absolute http(s) URL"
        );
    }

    #[test]
    fn local_contact_never_changes_public_outputs() {
        let Some(root) = fixture("contact") else {
            return;
        };
        // Freeze document timestamps so byte equality is meaningful.
        std::env::set_var("SOURCE_DATE_EPOCH", "1789776000");
        let data = build_profile(&root, true).unwrap();
        let baseline = snapshot(&root);
        fs::write(
            root.join("profile/contact.local.json"),
            r#"{"phone": "+00 123 4567"}"#,
        )
        .unwrap();
        assert_eq!(build_profile(&root, true).unwrap(), data);
        let full = snapshot(&root);
        for index in [0, 1, 3] {
            assert_eq!(baseline[index], full[index], "{}", OUTPUTS[index]);
        }
        assert_ne!(baseline[2], full[2]);
        assert!(!String::from_utf8_lossy(&full[0]).contains("phone"));
        build_profile(&root, false).unwrap();
        assert_eq!(
            full[2],
            fs::read(root.join("output/pdf/resume.pdf")).unwrap()
        );
        fs::write(
            root.join("profile/contact.local.json"),
            r#"{"phone": "1", "email": "x"}"#,
        )
        .unwrap();
        let error = build_profile(&root, true).unwrap_err().to_string();
        assert_eq!(error, "contact.local.json must contain only a phone string");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn indented_json_matches_python() {
        let value = serde_json::json!({"b": [], "a": {"x": ["é\n", 1]}, "c": {}});
        assert_eq!(
            python_dumps_indent(&value),
            "{\n  \"b\": [],\n  \"a\": {\n    \"x\": [\n      \"é\\n\",\n      1\n    ]\n  },\n  \"c\": {}\n}"
        );
    }
}
