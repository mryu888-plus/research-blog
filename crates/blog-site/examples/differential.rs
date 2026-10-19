//! Differential test driver: reads JSON requests on stdin, one per line, and
//! answers each with one JSON line. Used to compare against the Python scripts.
use blog_site::{math::protect_math, prepare::PreparedSite, references};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::Path;

fn handle(request: &Value) -> anyhow::Result<Value> {
    let text = request["text"].as_str().unwrap_or("");
    match request["op"].as_str().unwrap_or("") {
        "math" => Ok(json!({"text": protect_math(text)})),
        "cite" => {
            let items: references::Items = serde_json::from_value(request["items"].clone())?;
            let (text, keys) = references::transform_citations(text, &items, "article")?;
            Ok(json!({"text": text, "keys": keys}))
        }
        "bib" => {
            let items = references::read_bibtex(text, "refs.bib")?;
            Ok(
                json!({"items": items.into_iter().map(|(_, item)| Value::Object(item)).collect::<Vec<_>>()}),
            )
        }
        "prepare" => {
            let mut site = PreparedSite::new(
                Path::new(request["source"].as_str().unwrap()),
                Path::new(request["target"].as_str().unwrap()),
            )?;
            let changed = site.sync()?;
            Ok(json!({"changed": changed.iter().map(|c| c.as_str()).collect::<Vec<_>>()}))
        }
        other => anyhow::bail!("unknown op {other}"),
    }
}

fn main() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let request: Value = serde_json::from_str(&line.unwrap()).unwrap();
        let answer = handle(&request).unwrap_or_else(|error| json!({"error": error.to_string()}));
        writeln!(stdout, "{answer}").unwrap();
        stdout.flush().unwrap();
    }
}
