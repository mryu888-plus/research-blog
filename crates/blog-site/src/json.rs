//! JSON text in the layout Python's `json.dumps(..., sort_keys=True)` writes.

use serde_json::Value;

pub fn python_dumps(value: &Value) -> String {
    let mut output = String::new();
    write(value, &mut output);
    output
}

fn write(value: &Value, output: &mut String) {
    match value {
        Value::Array(items) => {
            output.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    output.push_str(", ");
                }
                write(item, output);
            }
            output.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            output.push('{');
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    output.push_str(", ");
                }
                output.push_str(&serde_json::to_string(key).unwrap());
                output.push_str(": ");
                write(&map[key], output);
            }
            output.push('}');
        }
        other => output.push_str(&other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn matches_python_layout() {
        let value = serde_json::json!({"b": [1, "é\n"], "a": {"c": true}});
        assert_eq!(
            super::python_dumps(&value),
            r#"{"a": {"c": true}, "b": [1, "é\n"]}"#
        );
    }
}
