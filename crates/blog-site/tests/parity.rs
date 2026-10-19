//! Frozen outputs of the Python scripts the Rust port replaced.
//!
//! The fixtures were recorded from `scripts/math_markdown.py` and
//! `scripts/references.py` before they were removed: every call the Python
//! test suite made, plus deterministic fuzz strings of Markdown syntax.

use blog_site::math::protect_math;
use blog_site::references::{read_bibtex, transform_citations, Items};
use serde_json::Value;

fn cases(name: &str) -> (Value, Vec<Value>) {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let mut data: Value = serde_json::from_str(&text).unwrap();
    let cases = data["cases"].as_array().unwrap().clone();
    assert!(!cases.is_empty(), "{name} has no cases");
    (data["meta"].take(), cases)
}

/// The expected text: `output`, or the input when Python left it unchanged.
fn expected_text(case: &Value) -> &str {
    if case["unchanged"] == Value::Bool(true) {
        case["input"].as_str().unwrap()
    } else {
        case["output"].as_str().unwrap()
    }
}

fn report(failures: &[String], total: usize) {
    assert!(
        failures.is_empty(),
        "{} of {total} cases differ from Python:\n{}",
        failures.len(),
        failures
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn protect_math_matches_python_math_markdown() {
    let (_, cases) = cases("math.json");
    let mut failures = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let input = case["input"].as_str().unwrap();
        let got = protect_math(input);
        if got != expected_text(case) {
            failures.push(format!(
                "case {index} ({}): input {input:?}\n  got {got:?}",
                case["source"]
            ));
        }
    }
    report(&failures, cases.len());
}

#[test]
fn transform_citations_matches_python_references() {
    let (meta, cases) = cases("citations.json");
    let default_items: Items = serde_json::from_value(meta["default_items"].clone()).unwrap();
    let mut failures = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let input = case["input"].as_str().unwrap();
        let items: Items = match case.get("items") {
            Some(items) => serde_json::from_value(items.clone()).unwrap(),
            None => default_items.clone(),
        };
        let got = transform_citations(input, &items, "article");
        let ok = match (&got, case.get("error").and_then(Value::as_str)) {
            // tomllib and the toml crate word syntax errors differently.
            (Err(_), Some("toml-error")) => true,
            (Err(error), Some(expected)) => error.to_string() == expected,
            (Ok((text, keys)), None) => {
                let expected_keys: Vec<String> =
                    serde_json::from_value(case["keys"].clone()).unwrap();
                text == expected_text(case) && *keys == expected_keys
            }
            _ => false,
        };
        if !ok {
            failures.push(format!(
                "case {index} ({}): input {input:?}\n  got {got:?}",
                case["source"]
            ));
        }
    }
    report(&failures, cases.len());
}

#[test]
fn read_bibtex_matches_python_references() {
    let (_, cases) = cases("bibtex.json");
    let mut failures = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let input = case["input"].as_str().unwrap();
        let got = read_bibtex(input, "refs.bib");
        let ok = match (&got, case.get("error").and_then(Value::as_str)) {
            (Err(error), Some(expected)) => error.to_string() == expected,
            (Ok(items), None) => {
                let items: Vec<Value> = items
                    .iter()
                    .map(|(_, item)| Value::Object(item.clone()))
                    .collect();
                Value::Array(items) == case["items"]
            }
            _ => false,
        };
        if !ok {
            failures.push(format!(
                "case {index} ({}): input {input:?}\n  got {got:?}",
                case["source"]
            ));
        }
    }
    report(&failures, cases.len());
}
