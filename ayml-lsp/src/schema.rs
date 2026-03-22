use serde_json::Value as Json;

/// Walk a JSON Schema to find the sub-schema at the given path segments.
/// Follows `properties` for object schemas and `items` for array schemas.
/// Resolves local `$ref` pointers within the root schema.
pub fn resolve_sub_schema<'a>(root: &'a Json, path: &[&str]) -> Option<&'a Json> {
    let mut schema = root;

    for segment in path {
        schema = resolve_refs(root, schema);
        schema = try_step(root, schema, segment)?;
    }

    Some(resolve_refs(root, schema))
}

/// Try to step into a schema by one path segment, resolving through
/// `$ref`, `allOf`, `anyOf`, and `oneOf` as needed.
fn try_step<'a>(root: &'a Json, schema: &'a Json, segment: &str) -> Option<&'a Json> {
    // Direct step.
    if let Some(next) = step_into(schema, segment) {
        return Some(next);
    }

    // Try through allOf — merge all sub-schemas and try each.
    if let Some(all) = schema.get("allOf").and_then(|v| v.as_array()) {
        for sub in all {
            let resolved = resolve_refs(root, sub);
            if let Some(next) = try_step(root, resolved, segment) {
                return Some(next);
            }
        }
    }

    // Try through anyOf/oneOf variants.
    for keyword in &["anyOf", "oneOf"] {
        if let Some(variants) = schema.get(*keyword).and_then(|v| v.as_array()) {
            for variant in variants {
                let resolved = resolve_refs(root, variant);
                if let Some(next) = try_step(root, resolved, segment) {
                    return Some(next);
                }
            }
        }
    }

    None
}

/// Try to step into a schema by one path segment (direct lookup only).
fn step_into<'a>(schema: &'a Json, segment: &str) -> Option<&'a Json> {
    // Try `properties/<key>`
    if let Some(sub) = schema.get("properties").and_then(|p| p.get(segment)) {
        return Some(sub);
    }

    // Try `items` (array index)
    if segment.parse::<usize>().is_ok() {
        if let Some(items) = schema.get("items") {
            return Some(items);
        }
    }

    // Try `additionalProperties` as object schema
    if let Some(additional) = schema.get("additionalProperties") {
        if additional.is_object() {
            return Some(additional);
        }
    }

    None
}

/// Resolve `$ref` pointers (local JSON pointer refs only).
fn resolve_refs<'a>(root: &'a Json, schema: &'a Json) -> &'a Json {
    if let Some(ref_str) = schema.get("$ref").and_then(|r| r.as_str())
        && let Some(pointer) = ref_str.strip_prefix('#')
        && let Some(resolved) = pointer_lookup(root, pointer)
    {
        return resolved;
    }
    schema
}

/// Look up a JSON pointer (e.g. "/definitions/Foo") in a JSON value.
fn pointer_lookup<'a>(root: &'a Json, pointer: &str) -> Option<&'a Json> {
    if pointer.is_empty() || pointer == "/" {
        return Some(root);
    }
    let segments = pointer.strip_prefix('/')?.split('/');
    let mut current = root;
    for seg in segments {
        // Unescape JSON pointer encoding (~0 = ~, ~1 = /)
        let unescaped = seg.replace("~1", "/").replace("~0", "~");
        current = current.get(&unescaped)?;
    }
    Some(current)
}

/// Build a markdown hover string from a JSON sub-schema.
pub fn hover_content(schema: &Json) -> Option<String> {
    // If the schema itself has no description but is an anyOf/oneOf with a
    // non-null variant, pull info from that variant too.
    let effective = effective_schema(schema);
    hover_content_inner(schema, effective)
}

/// For composition schemas, return the most informative sub-schema:
/// - `allOf`: return the first sub-schema (typically the `$ref`)
/// - `anyOf`/`oneOf`: return the first non-null variant
fn effective_schema(schema: &Json) -> Option<&Json> {
    // allOf: return the first entry (usually the primary $ref)
    if let Some(all) = schema.get("allOf").and_then(|v| v.as_array()) {
        if let Some(first) = all.first() {
            return Some(first);
        }
    }
    for keyword in &["anyOf", "oneOf"] {
        if let Some(variants) = schema.get(*keyword).and_then(|v| v.as_array()) {
            for variant in variants {
                let is_null = variant.get("type").and_then(|t| t.as_str()) == Some("null");
                if !is_null {
                    return Some(variant);
                }
            }
        }
    }
    None
}

fn hover_content_inner(schema: &Json, effective: Option<&Json>) -> Option<String> {
    let mut parts = Vec::new();

    // Try description from the schema itself, then from the effective variant.
    let desc = schema
        .get("description")
        .and_then(|d| d.as_str())
        .or_else(|| {
            effective
                .and_then(|e| e.get("description"))
                .and_then(|d| d.as_str())
        });
    if let Some(desc) = desc {
        parts.push(desc.to_string());
    }

    let ty = schema_type_string(schema).or_else(|| effective.and_then(schema_type_string));
    if let Some(ty) = ty {
        parts.push(format!("**Type:** `{ty}`"));
    }

    let default = schema
        .get("default")
        .or_else(|| effective.and_then(|e| e.get("default")));
    if let Some(default) = default {
        parts.push(format!("**Default:** `{default}`"));
    }

    let enum_vals = schema.get("enum").and_then(|e| e.as_array()).or_else(|| {
        effective
            .and_then(|e| e.get("enum"))
            .and_then(|e| e.as_array())
    });
    if let Some(enum_vals) = enum_vals {
        let vals: Vec<String> = enum_vals.iter().map(|v| format!("`{v}`")).collect();
        parts.push(format!("**Allowed values:** {}", vals.join(", ")));
    }

    let pattern = schema.get("pattern").and_then(|p| p.as_str()).or_else(|| {
        effective
            .and_then(|e| e.get("pattern"))
            .and_then(|p| p.as_str())
    });
    if let Some(pattern) = pattern {
        parts.push(format!("**Pattern:** `{pattern}`"));
    }

    let min = schema
        .get("minimum")
        .or_else(|| effective.and_then(|e| e.get("minimum")));
    if let Some(min) = min {
        parts.push(format!("**Minimum:** `{min}`"));
    }

    let max = schema
        .get("maximum")
        .or_else(|| effective.and_then(|e| e.get("maximum")));
    if let Some(max) = max {
        parts.push(format!("**Maximum:** `{max}`"));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

/// Extract a human-readable type string from a schema node.
fn schema_type_string(schema: &Json) -> Option<String> {
    // Explicit "type" field
    if let Some(ty) = schema.get("type") {
        if let Some(s) = ty.as_str() {
            return Some(s.to_string());
        }
        if let Some(arr) = ty.as_array() {
            let types: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
            return Some(types.join(" | "));
        }
    }

    // oneOf / anyOf
    for keyword in &["oneOf", "anyOf"] {
        if let Some(variants) = schema.get(*keyword).and_then(|v| v.as_array()) {
            let types: Vec<String> = variants
                .iter()
                .filter_map(|v| {
                    v.get("type")
                        .and_then(|t| t.as_str())
                        .map(String::from)
                        .or_else(|| {
                            v.get("$ref").and_then(|r| r.as_str()).map(|r| {
                                // Show just the definition name from "#/definitions/Foo"
                                r.rsplit('/').next().unwrap_or(r).to_string()
                            })
                        })
                })
                .collect();
            if !types.is_empty() {
                return Some(types.join(" | "));
            }
        }
    }

    None
}
