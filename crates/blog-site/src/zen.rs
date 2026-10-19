//! ZEN.md is the authoring source of the visual style. This module validates
//! its two JSON blocks and generates every consumer: the site's zen.css, the
//! Typst figure template's parameter slot and, in the source repository, the
//! VS Code extension data, snippet and editor themes.

pub mod js;

use anyhow::{anyhow, bail, Result};
use js::{is_js_space, stringify, to_string, trim, truthy, utf16_cmp, Js, JS_SPACE_CLASS};
use regex::Regex;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

const TYPST_PATH: &str = "design/visual-system/typst/blog-diagram.typ";
const SNIPPETS_PATH: &str = "extensions/research-blog/snippets/blog.code-snippets";
const THEME_TEMPLATE_PATH: &str = "extensions/research-notes-theme/scripts/preview.template.html";
const EXTENSION_MARKER: &str = "extensions/research-blog/package.json";
const SNIPPET_NAME: &str = "博客 Typst 图形模板";

/// VS Code color roles for one editor palette. `"@key"` is a palette value and
/// `"@key/aa"` that color with its alpha replaced; everything else is literal.
const THEME_TEMPLATE: &str = include_str!("zen/theme.json");

static COLOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^#[0-9a-f]{6}$").unwrap());
static COLOR_ALPHA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^#[0-9a-f]{6}(?:[0-9a-f]{2})?$").unwrap());
static GENERIC_FONT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:sans-serif|serif|monospace|system-ui|-apple-system|BlinkMacSystemFont)$")
        .unwrap()
});
static TYPST_SLOT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"// zen:begin[^\n]*\n[\s\S]*?// zen:end").unwrap());

fn block(markdown: &str, name: &str) -> Result<Js> {
    let ws = JS_SPACE_CLASS;
    let pattern =
        format!(r"<!-- zen:{name} -->{ws}*```json{ws}*([\s\S]*?){ws}*```{ws}*<!-- /zen:{name} -->");
    let expression = Regex::new(&pattern).unwrap();
    let matches: Vec<_> = expression.captures_iter(markdown).collect();
    if matches.len() != 1 {
        bail!("ZEN.md must contain exactly one zen:{name} JSON block.");
    }
    js::parse(&matches[0][1])
}

/// Property access that throws like JavaScript on `null`/`undefined`.
fn read<'a>(value: Option<&'a Js>, key: &str) -> Result<Option<&'a Js>> {
    match value {
        None => Err(anyhow!(
            "Cannot read properties of undefined (reading '{key}')"
        )),
        Some(Js::Null) => Err(anyhow!("Cannot read properties of null (reading '{key}')")),
        Some(value) => Ok(value.get(key)),
    }
}

/// Optional chaining: `value?.[key]`.
fn opt<'a>(value: Option<&'a Js>, key: &str) -> Option<&'a Js> {
    value.and_then(|value| value.get(key))
}

fn positive(value: Option<&Js>, label: &str) -> Result<()> {
    match value {
        Some(Js::Num(n)) if n.is_finite() && *n > 0.0 => Ok(()),
        _ => bail!("Invalid ZEN {label}."),
    }
}

fn non_blank_string(value: Option<&Js>) -> bool {
    matches!(value, Some(Js::Str(s)) if !trim(s).is_empty())
}

/// `regex.test(value || '')`.
fn test_color(expression: &Regex, value: Option<&Js>) -> bool {
    let text = if truthy(value) {
        to_string(value)
    } else {
        String::new()
    };
    expression.is_match(&text)
}

/// `Object.keys(value)` for the shapes JSON can produce.
fn keys(value: &Js) -> Vec<String> {
    match value {
        Js::Obj(_) => value
            .entries()
            .into_iter()
            .map(|(k, _)| k.clone())
            .collect(),
        Js::Arr(items) => (0..items.len()).map(|i| i.to_string()).collect(),
        Js::Str(s) => (0..s.encode_utf16().count())
            .map(|i| i.to_string())
            .collect(),
        _ => Vec::new(),
    }
}

/// Parse and validate ZEN.md, returning the tokens with `prompts` merged in.
pub fn read_zen(markdown: &str) -> Result<Js> {
    let tokens = block(markdown, "tokens")?;
    let prompts = block(markdown, "prompts")?;
    let t = Some(&tokens);
    if !matches!(read(t, "schemaVersion")?, Some(Js::Num(n)) if *n == 1.0) {
        bail!("Unsupported ZEN schema.");
    }
    if !non_blank_string(read(t, "name")?) {
        bail!("Invalid ZEN name.");
    }
    for (group, names) in [
        (
            "layout",
            &["pageWidth", "readingWidth", "bodyLineHeight"][..],
        ),
        ("figure", &["finePt", "strongPt", "labelPt", "marginPt"][..]),
    ] {
        let value = read(t, group)?;
        if !truthy(value) {
            bail!("Missing ZEN {group}.");
        }
        for key in names {
            positive(read(value, key)?, &format!("{group}.{key}"))?;
        }
    }
    for key in ["articleWidth", "articleLineHeight"] {
        let value = read(read(t, "layout")?, key)?;
        if value.is_some() {
            positive(value, &format!("layout.{key}"))?;
        }
    }
    for (group, names) in [
        ("radius", &["small", "medium", "large"][..]),
        ("motion", &["feedbackMs"][..]),
    ] {
        for key in names {
            match opt(read(t, group)?, key) {
                Some(Js::Num(n)) if n.is_finite() && *n >= 0.0 => {}
                _ => bail!("Invalid ZEN {group}.{key}."),
            }
        }
    }
    match read(t, "spacing")? {
        Some(Js::Arr(steps)) if steps.len() == 7 => {
            for step in steps {
                positive(Some(step), "spacing")?;
            }
        }
        _ => bail!("ZEN spacing must have seven steps."),
    }
    for key in ["sans", "mono", "serif", "figure"] {
        let valid = match opt(read(t, "fonts")?, key) {
            Some(Js::Arr(fonts)) => {
                !fonts.is_empty() && fonts.iter().all(|font| non_blank_string(Some(font)))
            }
            _ => false,
        };
        if !valid {
            bail!("Invalid ZEN font {key}.");
        }
    }
    let colors = [
        "background",
        "surface",
        "raised",
        "border",
        "edge",
        "ink",
        "secondary",
        "muted",
        "accent",
        "accentMuted",
    ];
    let editor_colors = [
        "panel",
        "raised",
        "string",
        "typeColor",
        "number",
        "function",
        "purple",
        "error",
        "warning",
        "success",
        "info",
        "selection",
        "inactiveSelection",
        "find",
        "line",
        "buttonFg",
        "buttonHover",
        "shadow",
        "ansiBlack",
        "ansiWhite",
        "ansiCyan",
        "ansiBrightCyan",
    ];
    for key in [
        "background",
        "surface",
        "raised",
        "border",
        "ink",
        "secondary",
        "muted",
        "accent",
    ] {
        if !test_color(&COLOR, opt(read(t, "print")?, key)) {
            bail!("Invalid ZEN print color {key}.");
        }
    }
    let palettes = read(t, "palettes")?;
    let mut names = if truthy(palettes) {
        keys(palettes.unwrap())
    } else {
        Vec::new()
    };
    names.sort_by(|a, b| utf16_cmp(a, b));
    if names.join(",") != "graphite,paper" {
        bail!("ZEN requires Graphite and Paper palettes.");
    }
    for name in ["graphite", "paper"] {
        let palette = opt(palettes, name);
        if !truthy(palette) {
            bail!("Missing ZEN palette {name}.");
        }
        for key in colors {
            if !test_color(&COLOR, read(palette, key)?) {
                bail!("Invalid ZEN color {name}.{key}.");
            }
        }
        for key in editor_colors {
            if !test_color(&COLOR_ALPHA, opt(read(palette, "editor")?, key)) {
                bail!("Invalid ZEN editor color {name}.{key}.");
            }
        }
        for key in ["secondary", "muted", "border"] {
            if !test_color(&COLOR, opt(read(palette, "contrast")?, key)) {
                bail!("Invalid ZEN contrast color {name}.{key}.");
            }
        }
        for key in ["halo", "scanline", "nav", "edgeLight", "shadow"] {
            if !non_blank_string(opt(read(palette, "effects")?, key)) {
                bail!("Missing ZEN effect {name}.{key}.");
            }
        }
        match read(palette, "imageWords")? {
            None | Some(Js::Null) => bail!("Missing ZEN image description {name}."),
            Some(Js::Str(words)) if !trim(words).is_empty() => {}
            Some(Js::Str(_)) => bail!("Missing ZEN image description {name}."),
            Some(_) => bail!("palette.imageWords?.trim is not a function"),
        }
    }
    let p = Some(&prompts);
    let image = read(p, "image")?;
    let typst = read(p, "typst")?;
    let compositions = opt(image, "compositions");
    let texts = [
        opt(image, "opening"),
        opt(image, "treatment"),
        opt(image, "constraints"),
        opt(compositions, "editorial"),
        opt(compositions, "cover"),
        opt(compositions, "concept"),
        opt(typst, "opening"),
        opt(typst, "constraints"),
    ];
    if !texts.iter().all(|text| non_blank_string(*text)) {
        bail!("Missing ZEN prompt rule.");
    }
    let mut zen = tokens.clone();
    zen.set("prompts", prompts);
    Ok(zen)
}

/// Field access on validated ZEN data.
fn at<'a>(value: &'a Js, path: &[&str]) -> Option<&'a Js> {
    path.iter().try_fold(value, |current, key| current.get(key))
}

fn s(value: &Js, path: &[&str]) -> String {
    to_string(at(value, path))
}

fn font_list(fonts: Option<&Js>) -> String {
    let Some(Js::Arr(fonts)) = fonts else {
        return String::new();
    };
    fonts
        .iter()
        .map(|font| {
            let text = to_string(Some(font));
            if GENERIC_FONT.is_match(&text) {
                text
            } else {
                stringify(font, 0)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn variables(values: &[(String, String)]) -> String {
    values
        .iter()
        .map(|(key, value)| format!("  --{key}: {value};"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn pairs(items: &[(&str, String)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

fn css(zen: &Js) -> String {
    let palette_vars = |p: &Js| {
        pairs(&[
            ("surface-0", s(p, &["background"])),
            ("surface-1", s(p, &["surface"])),
            ("surface-2", s(p, &["raised"])),
            ("surface-3", s(p, &["border"])),
            ("surface-4", s(p, &["edge"])),
            ("text-primary", s(p, &["ink"])),
            ("text-secondary", s(p, &["secondary"])),
            ("text-muted", s(p, &["muted"])),
            ("accent", s(p, &["accent"])),
            ("accent-muted", s(p, &["accentMuted"])),
            ("halo", s(p, &["effects", "halo"])),
            ("scanline", s(p, &["effects", "scanline"])),
            ("nav-background", s(p, &["effects", "nav"])),
            ("edge-light", s(p, &["effects", "edgeLight"])),
            ("panel-shadow", s(p, &["effects", "shadow"])),
        ])
    };
    let or_default = |path: &[&str], default: f64| match at(zen, path) {
        None | Some(Js::Null) => js::number(default),
        value => to_string(value),
    };
    let mut common = pairs(&[
        ("font-sans", font_list(at(zen, &["fonts", "sans"]))),
        ("font-mono", font_list(at(zen, &["fonts", "mono"]))),
        ("font-serif", font_list(at(zen, &["fonts", "serif"]))),
        ("radius-sm", format!("{}px", s(zen, &["radius", "small"]))),
        ("radius-md", format!("{}px", s(zen, &["radius", "medium"]))),
        ("radius-lg", format!("{}px", s(zen, &["radius", "large"]))),
        ("radius-full", "999px".into()),
        (
            "page-width",
            format!("{}px", s(zen, &["layout", "pageWidth"])),
        ),
        (
            "reading-width",
            format!("{}px", s(zen, &["layout", "readingWidth"])),
        ),
        (
            "article-width",
            format!("{}px", or_default(&["layout", "articleWidth"], 700.0)),
        ),
        (
            "article-line-height",
            or_default(&["layout", "articleLineHeight"], 1.95),
        ),
        ("body-line-height", s(zen, &["layout", "bodyLineHeight"])),
        (
            "feedback-duration",
            format!("{}ms", s(zen, &["motion", "feedbackMs"])),
        ),
    ]);
    let palettes = at(zen, &["palettes"]).cloned().unwrap_or(Js::obj());
    for (key, palette) in palettes.entries() {
        for role in ["background", "accent"] {
            common.push((format!("zen-{key}-{role}"), s(palette, &[role])));
        }
    }
    let spacing = |index: usize| match at(zen, &["spacing"]) {
        Some(Js::Arr(steps)) => to_string(steps.get(index)),
        _ => "undefined".into(),
    };
    for (index, key) in [8, 12, 16, 24, 32, 48, 64].iter().enumerate() {
        common.push((format!("space-{key}"), format!("{}px", spacing(index))));
    }
    for (index, key) in ["xs", "sm", "md", "lg", "xl", "2xl", "3xl"]
        .iter()
        .enumerate()
    {
        common.push((format!("spacing-{key}"), format!("{}px", spacing(index))));
    }
    let contrast = |p: &Js| {
        variables(&pairs(&[
            ("text-secondary", s(p, &["contrast", "secondary"])),
            ("text-muted", s(p, &["contrast", "muted"])),
            ("surface-3", s(p, &["contrast", "border"])),
        ]))
    };
    let graphite = at(&palettes, &["graphite"]).cloned().unwrap_or(Js::obj());
    let paper = at(&palettes, &["paper"]).cloned().unwrap_or(Js::obj());
    let print_colors = at(zen, &["print"]).cloned().unwrap_or(Js::obj());
    let p = &print_colors;
    let print = variables(&pairs(&[
        ("surface-0", s(p, &["background"])),
        ("surface-1", s(p, &["surface"])),
        ("surface-2", s(p, &["raised"])),
        ("surface-3", s(p, &["border"])),
        ("text-primary", s(p, &["ink"])),
        ("text-secondary", s(p, &["secondary"])),
        ("text-muted", s(p, &["muted"])),
        ("accent", s(p, &["accent"])),
    ]));
    format!(
        "/* Generated from ZEN.md by scripts/sync_zen.cjs. Do not edit. */\n:root {{\n  color-scheme: dark;\n{}\n{}\n}}\n:root[data-theme=\"light\"] {{\n  color-scheme: light;\n{}\n}}\n@media (prefers-contrast: more) {{\n  :root {{\n{}\n  }}\n  :root[data-theme=\"light\"] {{\n{}\n  }}\n}}\n@media print {{\n  :root, :root[data-theme=\"light\"] {{\n{}\n  }}\n}}\n",
        variables(&common),
        variables(&palette_vars(&graphite)),
        variables(&palette_vars(&paper)),
        contrast(&graphite),
        contrast(&paper),
        print
    )
}

fn editor_palettes(zen: &Js) -> Js {
    let mut result = Js::obj();
    let Some(palettes) = at(zen, &["palettes"]) else {
        return result;
    };
    for (key, p) in palettes.entries() {
        let graphite = key == "graphite";
        let mut palette = Js::obj();
        palette.set(
            "name",
            Js::Str(format!(
                "Research Notes {}",
                if graphite { "Graphite" } else { "Paper" }
            )),
        );
        palette.set(
            "type",
            Js::Str(if graphite { "dark" } else { "light" }.into()),
        );
        for (name, role) in [
            ("bg", "background"),
            ("fg", "ink"),
            ("secondary", "secondary"),
            ("muted", "muted"),
            ("accent", "accent"),
            ("border", "border"),
            ("edge", "edge"),
        ] {
            if let Some(value) = p.get(role) {
                palette.set(name, value.clone());
            }
        }
        if let Some(editor) = p.get("editor") {
            for (name, value) in editor.entries() {
                palette.set(name, value.clone());
            }
        }
        result.set(key, palette);
    }
    result
}

fn typst_guidance(zen: &Js) -> String {
    let p = at(zen, &["palettes", "paper"])
        .cloned()
        .unwrap_or(Js::obj());
    let d = at(zen, &["palettes", "graphite"])
        .cloned()
        .unwrap_or(Js::obj());
    let f = at(zen, &["figure"]).cloned().unwrap_or(Js::obj());
    let label_plus_one = match f.get("labelPt") {
        Some(Js::Num(n)) => js::number(n + 1.0),
        other => format!("{}1", to_string(other)),
    };
    format!(
        "{} Use the configured light palette: background {}, ink {}, secondary {}, accent {}; the dark palette uses background {}, ink {}, secondary {}, accent {}. Use {}pt fine strokes and {}pt primary strokes, {}-{}pt labels. {}",
        s(zen, &["prompts", "typst", "opening"]),
        s(&p, &["background"]),
        s(&p, &["ink"]),
        s(&p, &["secondary"]),
        s(&p, &["accent"]),
        s(&d, &["background"]),
        s(&d, &["ink"]),
        s(&d, &["secondary"]),
        s(&d, &["accent"]),
        s(&f, &["finePt"]),
        s(&f, &["strongPt"]),
        s(&f, &["labelPt"]),
        label_plus_one,
        s(zen, &["prompts", "typst", "constraints"]),
    )
}

fn typst_header(zen: &Js) -> String {
    let d = at(zen, &["palettes", "graphite"])
        .cloned()
        .unwrap_or(Js::obj());
    let p = at(zen, &["palettes", "paper"])
        .cloned()
        .unwrap_or(Js::obj());
    let mut lines = vec![
        "// zen:begin — generated from ZEN.md; keep this template standalone.".to_string(),
        "#let dark = sys.inputs.at(\"theme\", default: \"paper\") == \"graphite\"".to_string(),
    ];
    for (key, role) in [
        ("bg", "background"),
        ("ink", "ink"),
        ("muted", "secondary"),
        ("accent", "accent"),
    ] {
        lines.push(format!(
            "#let {key} = if dark {{ rgb(\"{}\") }} else {{ rgb(\"{}\") }}",
            s(&d, &[role]),
            s(&p, &[role])
        ));
    }
    let fonts = match at(zen, &["fonts", "figure"]) {
        Some(Js::Arr(fonts)) => fonts
            .iter()
            .map(|font| stringify(font, 0))
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    };
    lines.push(format!("#let fine = {}pt", s(zen, &["figure", "finePt"])));
    lines.push(format!(
        "#let strong = {}pt",
        s(zen, &["figure", "strongPt"])
    ));
    lines.push(format!(
        "#set page(width: auto, height: auto, margin: {}pt, fill: bg)",
        s(zen, &["figure", "marginPt"])
    ));
    lines.push(format!(
        "#set text(font: ({fonts}), size: {}pt, fill: ink)",
        s(zen, &["figure", "labelPt"])
    ));
    lines.push("// zen:end".to_string());
    lines.join("\n")
}

/// Fill one theme template node from an editor palette; `None` is `undefined`.
fn fill_theme(node: &Js, palette: &Js) -> Option<Js> {
    match node {
        Js::Str(text) if text.starts_with('@') => {
            let (key, alpha) = match text[1..].split_once('/') {
                Some((key, alpha)) => (key, Some(alpha)),
                None => (&text[1..], None),
            };
            let value = palette.get(key)?;
            match alpha {
                None => Some(value.clone()),
                // Explicit overlay alpha replaces any author-supplied alpha.
                Some(alpha) => {
                    let color = to_string(Some(value));
                    let units: Vec<u16> = color.encode_utf16().take(7).collect();
                    Some(Js::Str(format!(
                        "{}{alpha}",
                        String::from_utf16_lossy(&units)
                    )))
                }
            }
        }
        Js::Arr(items) => Some(Js::Arr(
            items
                .iter()
                .map(|item| fill_theme(item, palette).unwrap_or(Js::Null))
                .collect(),
        )),
        Js::Obj(entries) => Some(Js::Obj(
            entries
                .iter()
                .filter_map(|(k, v)| fill_theme(v, palette).map(|v| (k.clone(), v)))
                .collect(),
        )),
        other => Some(other.clone()),
    }
}

/// `String.prototype.replace` with a string pattern: first occurrence, `$` patterns expanded.
fn replace_first(haystack: &str, pattern: &str, replacement: &str) -> String {
    let Some(position) = haystack.find(pattern) else {
        return haystack.to_string();
    };
    let (before, after) = (&haystack[..position], &haystack[position + pattern.len()..]);
    let mut expanded = String::new();
    let mut chars = replacement.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '$' {
            expanded.push(c);
            continue;
        }
        match chars.peek() {
            Some('$') => {
                chars.next();
                expanded.push('$');
            }
            Some('&') => {
                chars.next();
                expanded.push_str(pattern);
            }
            Some('`') => {
                chars.next();
                expanded.push_str(before);
            }
            Some('\'') => {
                chars.next();
                expanded.push_str(after);
            }
            _ => expanded.push('$'),
        }
    }
    format!("{before}{expanded}{after}")
}

fn render_themes(palettes: &Js, template: &str) -> Result<Vec<(String, String)>> {
    let theme_template = js::parse(THEME_TEMPLATE)?;
    let mut themes = Js::obj();
    let mut outputs = Vec::new();
    for (key, palette) in palettes.entries() {
        let theme = fill_theme(&theme_template, palette).unwrap_or(Js::Null);
        outputs.push((
            format!("themes/research-notes-{key}-color-theme.json"),
            stringify(&theme, 2) + "\n",
        ));
        themes.set(key, theme);
    }
    let data = format!("const THEMES = {};", stringify(&themes, 0));
    outputs.push((
        "preview.html".into(),
        replace_first(template, "/* THEME_DATA */", &data),
    ));
    Ok(outputs)
}

fn read_normalized(root: &Path, relative: &str) -> Result<String> {
    let path = root.join(relative);
    match fs::read(&path) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).replace("\r\n", "\n")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            bail!(
                "ENOENT: no such file or directory, open '{}'",
                path.display()
            )
        }
        Err(error) => Err(error.into()),
    }
}

/// Every generated consumer and its content, in a fixed order.
pub fn render_outputs(root: &Path, zen: &Js) -> Result<Vec<(String, String)>> {
    let extension = root.join(EXTENSION_MARKER).is_file();
    let mut data = zen.clone();
    let palettes = editor_palettes(zen);
    data.set("editorPalettes", palettes.clone());
    data.set("typstGuidance", Js::Str(typst_guidance(zen)));
    let source = read_normalized(root, TYPST_PATH)?;
    let slots = TYPST_SLOT.find_iter(&source).count();
    if slots != 1 {
        bail!("Typst template must contain exactly one ZEN parameter slot.");
    }
    let header = typst_header(zen);
    let typst = TYPST_SLOT
        .replace(&source, regex::NoExpand(&header))
        .into_owned();
    let mut outputs = vec![("site/static/zen.css".to_string(), css(zen))];
    if !extension {
        outputs.push((TYPST_PATH.to_string(), typst));
        return Ok(outputs);
    }
    let mut snippets = js::parse(&read_normalized(root, SNIPPETS_PATH)?)?;
    let snippet = snippets.get_mut(SNIPPET_NAME);
    let Some(snippet) =
        snippet.filter(|s| s.get("prefix").and_then(Js::as_str) == Some("blogdiagram"))
    else {
        bail!("Missing blogdiagram snippet.");
    };
    snippet.set(
        "description",
        Js::Str("ZEN 图形：配色、线宽、标签与精确几何".into()),
    );
    // Literal dollars/backslashes must survive the VS Code snippet parser.
    let body = typst
        .trim_end_matches(is_js_space)
        .split('\n')
        .map(|line| Js::Str(line.replace('\\', "\\\\").replace('$', "\\$")))
        .collect();
    snippet.set("body", Js::Arr(body));
    outputs.push((
        "design/zen.generated.cjs".to_string(),
        format!(
            "'use strict';\n// Generated from ZEN.md by scripts/sync_zen.cjs. Do not edit.\nmodule.exports = {};\n",
            stringify(&data, 2)
        ),
    ));
    outputs.push((TYPST_PATH.to_string(), typst));
    outputs.push((SNIPPETS_PATH.to_string(), stringify(&snippets, 2) + "\n"));
    let template = read_normalized(root, THEME_TEMPLATE_PATH)?;
    for (name, content) in render_themes(&palettes, &template)? {
        outputs.push((format!("extensions/research-notes-theme/{name}"), content));
    }
    Ok(outputs)
}

/// Regenerate every consumer of ZEN.md under `root`. With `check`, nothing is
/// written and stale outputs are an error. Returns the changed paths.
pub fn sync(root: &Path, check: bool) -> Result<Vec<String>> {
    let markdown = String::from_utf8_lossy(&fs::read(root.join("ZEN.md"))?).into_owned();
    let zen = read_zen(&markdown)?;
    // Render and validate everything before writing any consumer.
    let outputs = render_outputs(root, &zen)?;
    let mut changed = Vec::new();
    for (relative, content) in outputs {
        let filename = root.join(&relative);
        if filename.exists()
            && String::from_utf8_lossy(&fs::read(&filename)?).replace("\r\n", "\n") == content
        {
            continue;
        }
        if !check {
            if let Some(parent) = filename.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&filename, content)?;
        }
        changed.push(relative);
    }
    if check && !changed.is_empty() {
        bail!(
            "ZEN generated files are stale. Run blogctl zen:\n{}",
            changed.join("\n")
        );
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repo() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn source() -> String {
        fs::read_to_string(repo().join("ZEN.md")).unwrap()
    }

    fn zen() -> Js {
        read_zen(&source()).unwrap()
    }

    /// ZEN.md with both JSON blocks replaced by the given style.
    fn markdown_with_style(style: &Js) -> String {
        let mut tokens = style.clone();
        let prompts = style.get("prompts").cloned().unwrap();
        if let Js::Obj(entries) = &mut tokens {
            entries.retain(|(k, _)| k != "prompts");
        }
        let mut text = source();
        for (name, value) in [("tokens", tokens), ("prompts", prompts)] {
            let expression = Regex::new(&format!(
                r"(<!-- zen:{name} -->\s*```json\s*)[\s\S]*?(\s*```\s*<!-- /zen:{name} -->)"
            ))
            .unwrap();
            let json = stringify(&value, 2);
            text = expression
                .replace(&text, |c: &regex::Captures| {
                    format!("{}{json}{}", &c[1], &c[2])
                })
                .into_owned();
        }
        text
    }

    fn fixture(name: &str, extension: bool) -> PathBuf {
        let root = std::env::temp_dir().join(format!("blog-zen-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mut files = vec![TYPST_PATH];
        if extension {
            files.extend([SNIPPETS_PATH, THEME_TEMPLATE_PATH, EXTENSION_MARKER]);
        }
        for relative in files {
            let destination = root.join(relative);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(repo().join(relative), destination).unwrap();
        }
        root
    }

    fn set(value: &mut Js, path: &[&str], new: Js) {
        let (last, parents) = path.split_last().unwrap();
        let target = parents
            .iter()
            .fold(value, |current, key| current.get_mut(key).unwrap());
        target.set(last, new);
    }

    fn num(n: f64) -> Js {
        Js::Num(n)
    }

    fn output<'a>(outputs: &'a [(String, String)], name: &str) -> &'a str {
        &outputs.iter().find(|(k, _)| k == name).unwrap().1
    }

    #[test]
    fn repository_consumers_are_current() {
        assert_eq!(sync(&repo(), true).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn custom_style_reaches_every_consumer() {
        let mut style = zen();
        set(&mut style, &["name"], Js::Str("青蓝实验室".into()));
        set(
            &mut style,
            &["fonts", "sans"],
            js::parse(r#"["Source Sans 3", "sans-serif"]"#).unwrap(),
        );
        set(
            &mut style,
            &["fonts", "figure"],
            js::parse(r#"["Noto Sans", "Arial"]"#).unwrap(),
        );
        set(&mut style, &["layout", "pageWidth"], num(980.0));
        set(&mut style, &["layout", "bodyLineHeight"], num(1.6));
        set(&mut style, &["radius", "small"], num(0.0));
        set(&mut style, &["motion", "feedbackMs"], num(0.0));
        set(
            &mut style,
            &["spacing"],
            js::parse("[4, 9, 14, 20, 28, 40, 56]").unwrap(),
        );
        set(
            &mut style,
            &["figure"],
            js::parse(r#"{"finePt": 0.8, "strongPt": 1.3, "labelPt": 11, "marginPt": 24}"#)
                .unwrap(),
        );
        set(
            &mut style,
            &["palettes", "paper", "background"],
            Js::Str("#f2f9fc".into()),
        );
        set(
            &mut style,
            &["palettes", "paper", "accent"],
            Js::Str("#007d80".into()),
        );
        let root = fixture("custom", true);
        fs::write(root.join("ZEN.md"), markdown_with_style(&style)).unwrap();
        assert!(!sync(&root, false).unwrap().is_empty());
        assert!(sync(&root, true).unwrap().is_empty());
        let read = |relative: &str| fs::read_to_string(root.join(relative)).unwrap();
        let css = read("site/static/zen.css");
        for line in [
            "--page-width: 980px;",
            "--body-line-height: 1.6;",
            "--radius-sm: 0px;",
            "--feedback-duration: 0ms;",
            "--space-12: 9px;",
            "--spacing-3xl: 56px;",
            "--font-sans: \"Source Sans 3\", sans-serif;",
            "--surface-0: #f2f9fc;",
        ] {
            assert!(css.contains(line), "{line}");
        }
        let theme = js::parse(&read(
            "extensions/research-notes-theme/themes/research-notes-paper-color-theme.json",
        ))
        .unwrap();
        assert_eq!(
            at(&theme, &["colors", "editor.background"]).and_then(Js::as_str),
            Some("#f2f9fc")
        );
        assert_eq!(
            at(&theme, &["colors", "editorCursor.foreground"]).and_then(Js::as_str),
            Some("#007d80")
        );
        let typst = read(TYPST_PATH);
        assert!(typst.contains("#let fine = 0.8pt") && typst.contains("margin: 24pt"));
        assert!(typst.contains("font: (\"Noto Sans\", \"Arial\"), size: 11pt"));
        assert!(read(SNIPPETS_PATH).contains("#let fine = 0.8pt"));
        let generated = read("design/zen.generated.cjs");
        assert!(
            generated.contains("\"name\": \"青蓝实验室\"")
                && generated.contains("0.8pt fine strokes")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn end_user_site_gets_css_and_typst_only() {
        let root = fixture("end-user", false);
        fs::write(root.join("ZEN.md"), source()).unwrap();
        sync(&root, false).unwrap();
        assert!(root.join("site/static/zen.css").is_file());
        assert!(!root.join("design/zen.generated.cjs").exists());
        assert!(!root.join("extensions").exists());
        assert_eq!(sync(&root, true).unwrap(), Vec::<String>::new());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn optional_layout_fields_keep_defaults() {
        let mut style = zen();
        if let Some(Js::Obj(layout)) = style.get_mut("layout") {
            layout.retain(|(k, _)| k != "articleWidth" && k != "articleLineHeight");
        }
        let css = css(&read_zen(&markdown_with_style(&style)).unwrap());
        assert!(css.contains("--article-width: 700px;"));
        assert!(css.contains("--article-line-height: 1.95;"));
    }

    #[test]
    fn overlays_replace_editor_alpha() {
        let mut style = zen();
        for name in ["graphite", "paper"] {
            let editor = style
                .get_mut("palettes")
                .unwrap()
                .get_mut(name)
                .unwrap()
                .get_mut("editor")
                .unwrap();
            if let Js::Obj(entries) = editor {
                for (_, value) in entries.iter_mut() {
                    let color = value.as_str().unwrap()[..7].to_string();
                    *value = Js::Str(color + "80");
                }
            }
        }
        let outputs =
            render_outputs(&repo(), &read_zen(&markdown_with_style(&style)).unwrap()).unwrap();
        for name in ["graphite", "paper"] {
            let path = format!(
                "extensions/research-notes-theme/themes/research-notes-{name}-color-theme.json"
            );
            let theme = js::parse(output(&outputs, &path)).unwrap();
            let editor = at(&style, &["palettes", name, "editor"]).unwrap();
            let color = |key: &str| {
                at(&theme, &["colors", key])
                    .and_then(Js::as_str)
                    .unwrap()
                    .to_string()
            };
            let base = |key: &str| editor.get(key).and_then(Js::as_str).unwrap()[..7].to_string();
            for (_, value) in at(&theme, &["colors"]).unwrap().entries() {
                assert!(COLOR_ALPHA.is_match(value.as_str().unwrap()));
            }
            assert_eq!(color("list.dropBackground"), base("selection") + "aa");
            assert_eq!(
                color("diffEditor.insertedTextBackground"),
                base("success") + "24"
            );
            // Source-control foregrounds keep the author-specified alpha.
            assert_eq!(
                color("gitDecoration.addedResourceForeground"),
                base("success") + "80"
            );
        }
    }

    #[test]
    fn invalid_values_fail_before_writing() {
        for name in [
            Js::Str(String::new()),
            Js::Str("   ".into()),
            Js::Null,
            num(42.0),
        ] {
            let mut style = zen();
            set(&mut style, &["name"], name);
            assert!(read_zen(&markdown_with_style(&style))
                .unwrap_err()
                .to_string()
                .contains("name"));
        }
        for (group, keys) in [
            (
                "layout",
                &[
                    "pageWidth",
                    "readingWidth",
                    "bodyLineHeight",
                    "articleWidth",
                    "articleLineHeight",
                ][..],
            ),
            ("radius", &["small", "medium", "large"][..]),
            ("motion", &["feedbackMs"][..]),
            ("figure", &["finePt", "strongPt", "labelPt", "marginPt"][..]),
        ] {
            for key in keys {
                let mut style = zen();
                set(&mut style, &[group, key], num(-1.0));
                let error = read_zen(&markdown_with_style(&style))
                    .unwrap_err()
                    .to_string();
                assert!(error.contains(&format!("{group}.{key}")), "{error}");
            }
        }
        let root = fixture("invalid", true);
        let mut style = zen();
        set(&mut style, &["layout", "articleWidth"], num(-1.0));
        fs::write(root.join("ZEN.md"), markdown_with_style(&style)).unwrap();
        fs::create_dir_all(root.join("site/static")).unwrap();
        fs::write(
            root.join("site/static/zen.css"),
            "existing generated consumer",
        )
        .unwrap();
        assert!(sync(&root, false)
            .unwrap_err()
            .to_string()
            .contains("layout.articleWidth"));
        assert_eq!(
            fs::read_to_string(root.join("site/static/zen.css")).unwrap(),
            "existing generated consumer"
        );
        assert!(!root.join("design/zen.generated.cjs").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_blocks_are_rejected() {
        let text = source();
        let error = |markdown: String| read_zen(&markdown).unwrap_err().to_string();
        assert!(
            error(text.replace("<!-- zen:tokens -->", "<!-- missing -->")).contains("exactly one")
        );
        assert!(error(format!("{text}\n{text}")).contains("exactly one"));
        assert!(
            error(text.replace("\"schemaVersion\": 1", "\"schemaVersion\": 99")).contains("schema")
        );
        assert!(
            error(text.replace("\"pageWidth\": 1040", "\"pageWidth\": -1"))
                .contains("layout.pageWidth")
        );
        assert!(
            error(text.replace("\"feedbackMs\": 160", "\"missing\": 160")).contains("feedbackMs")
        );
        assert!(error(text.replace("\"#121213\"", "\"not a color\"")).contains("color"));
    }

    #[test]
    fn stale_outputs_fail_the_check_without_writing() {
        let root = fixture("stale", false);
        fs::write(root.join("ZEN.md"), source()).unwrap();
        let error = sync(&root, true).unwrap_err().to_string();
        assert_eq!(
            error,
            "ZEN generated files are stale. Run blogctl zen:\nsite/static/zen.css"
        );
        assert!(!root.join("site/static/zen.css").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
