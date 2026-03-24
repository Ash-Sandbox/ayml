/// Integration tests: parse and round-trip every `.yml` file in `files/`.
///
/// New fixture files added to `files/` are automatically picked up.
use std::fs;
use std::path::Path;

use ayml_core::{emit, parse, value::Value};

fn fixture_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("files")
        .leak()
}

fn collect_fixtures() -> Vec<(String, String)> {
    let dir = fixture_dir();
    let mut files: Vec<(String, String)> = Vec::new();
    for entry in fs::read_dir(dir).expect("failed to read files/ directory") {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("yml") {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let content = fs::read_to_string(&path).unwrap();
            files.push((name, content));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

#[test]
fn parse_all_fixtures() {
    let fixtures = collect_fixtures();
    assert!(!fixtures.is_empty(), "no .yml fixtures found in files/");

    for (name, content) in &fixtures {
        let result = parse(content);
        assert!(
            result.is_ok(),
            "failed to parse {name}: {}",
            result.unwrap_err()
        );
    }
}

#[test]
fn round_trip_all_fixtures() {
    let fixtures = collect_fixtures();

    for (name, content) in &fixtures {
        // Parse original
        let node = parse(content).unwrap_or_else(|e| {
            panic!("failed to parse {name}: {e}");
        });

        // Emit back to string
        let emitted = emit(&node);

        // Re-parse the emitted output
        let reparsed = parse(&emitted).unwrap_or_else(|e| {
            panic!(
                "failed to re-parse emitted output for {name}: {e}\n\
                 --- emitted ---\n{emitted}\n---"
            );
        });

        // The value trees must be equivalent (ignoring comments for now,
        // since the emitter may reformat them).
        assert_values_eq(&node.value, &reparsed.value, &format!("{name} (root)"));
    }
}

/// Recursively compare two Value trees, handling NaN specially.
fn assert_values_eq(a: &Value, b: &Value, context: &str) {
    match (a, b) {
        (Value::Null, Value::Null) => {}
        (Value::Bool(a), Value::Bool(b)) => {
            assert_eq!(a, b, "bool mismatch in {context}");
        }
        (Value::Int(a), Value::Int(b)) => {
            assert_eq!(a, b, "int mismatch in {context}");
        }
        (Value::Float(a), Value::Float(b)) => {
            if a.is_nan() && b.is_nan() {
                // Both NaN — ok
            } else {
                assert_eq!(a, b, "float mismatch in {context}");
            }
        }
        (Value::Str(a), Value::Str(b)) => {
            assert_eq!(a, b, "string mismatch in {context}");
        }
        (Value::Seq(a), Value::Seq(b)) => {
            assert_eq!(a.len(), b.len(), "sequence length mismatch in {context}");
            for (i, (na, nb)) in a.iter().zip(b.iter()).enumerate() {
                assert_values_eq(&na.value, &nb.value, &format!("{context}[{i}]"));
            }
        }
        (Value::Map(a), Value::Map(b)) => {
            assert_eq!(
                a.len(),
                b.len(),
                "mapping size mismatch in {context}: keys a={:?}, keys b={:?}",
                a.keys().collect::<Vec<_>>(),
                b.keys().collect::<Vec<_>>(),
            );
            for (key, node_a) in a {
                let node_b = b.get(key).unwrap_or_else(|| {
                    panic!("key {key} missing after round-trip in {context}");
                });
                assert_values_eq(&node_a.value, &node_b.value, &format!("{context}.{key}"));
            }
        }
        _ => {
            panic!(
                "type mismatch in {context}: {:?} vs {:?}",
                std::mem::discriminant(a),
                std::mem::discriminant(b),
            );
        }
    }
}
