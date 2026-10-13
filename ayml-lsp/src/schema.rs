use serde_json::Value as Json;

/// How many sub-schemas one lookup may visit while following `$ref`,
/// `allOf`, `anyOf`, `oneOf`, and `items`. Schemas come from documents and
/// may be cyclic, or branch exponentially.
const FUEL: usize = 1000;

/// Spend one unit of `fuel`; false if it is used up.
fn spend(fuel: &mut usize) -> bool {
    let ok = *fuel > 0;
    *fuel = fuel.saturating_sub(1);
    ok
}

/// Walk a JSON Schema to find the sub-schema at the given path segments.
/// Follows `properties` for object schemas and `items` for array schemas.
/// Resolves local `$ref` pointers within the root schema.
pub fn resolve_sub_schema<'a>(root: &'a Json, path: &[&str]) -> Option<&'a Json> {
    let mut schema = root;
    let mut fuel = FUEL;

    for segment in path {
        schema = resolve_refs(root, schema);
        schema = try_step(root, schema, segment, &mut fuel)?;
    }

    Some(resolve_refs(root, schema))
}

/// Try to step into a schema by one path segment, resolving through
/// `$ref`, `allOf`, `anyOf`, and `oneOf` as needed.
fn try_step<'a>(
    root: &'a Json,
    schema: &'a Json,
    segment: &str,
    fuel: &mut usize,
) -> Option<&'a Json> {
    if !spend(fuel) {
        return None;
    }
    // Direct step.
    if let Some(next) = step_into(schema, segment) {
        return Some(next);
    }

    // Try through allOf — merge all sub-schemas and try each.
    if let Some(all) = schema.get("allOf").and_then(|v| v.as_array()) {
        for sub in all {
            let resolved = resolve_refs(root, sub);
            if let Some(next) = try_step(root, resolved, segment, fuel) {
                return Some(next);
            }
        }
    }

    // Try through anyOf/oneOf variants.
    for keyword in &["anyOf", "oneOf"] {
        if let Some(variants) = schema.get(*keyword).and_then(|v| v.as_array()) {
            for variant in variants {
                let resolved = resolve_refs(root, variant);
                if let Some(next) = try_step(root, resolved, segment, fuel) {
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
    if segment.parse::<usize>().is_ok()
        && let Some(items) = schema.get("items")
    {
        return Some(items);
    }

    // Try `additionalProperties` as object schema
    if let Some(additional) = schema.get("additionalProperties")
        && additional.is_object()
    {
        return Some(additional);
    }

    None
}

/// Resolve local `$ref` pointers, following chains of references.
fn resolve_refs<'a>(root: &'a Json, mut schema: &'a Json) -> &'a Json {
    const MAX_CHAIN: usize = 32;
    for _ in 0..MAX_CHAIN {
        let target = schema
            .get("$ref")
            .and_then(Json::as_str)
            .and_then(|r| r.strip_prefix('#'))
            .and_then(|pointer| root.pointer(pointer));
        match target {
            Some(target) => schema = target,
            None => break,
        }
    }
    schema
}

/// Return a human-readable type string for the schema (e.g. `"all" | integer[]`).
pub fn type_string(root: &Json, schema: &Json) -> Option<String> {
    let schema = resolve_refs(root, schema);
    let effective = effective_schema(schema).map(|e| resolve_refs(root, e));
    let mut fuel = FUEL;
    schema_type_string(root, schema, &mut fuel)
        .or_else(|| effective.and_then(|e| schema_type_string(root, e, &mut fuel)))
}

/// Build a markdown hover string from a JSON sub-schema.
/// `root` is the full schema document, needed to resolve `$ref` pointers.
pub fn hover_content(root: &Json, schema: &Json) -> Option<String> {
    let schema = resolve_refs(root, schema);
    let effective = effective_schema(schema).map(|e| resolve_refs(root, e));
    hover_content_inner(root, schema, effective)
}

/// For composition schemas, return the most informative sub-schema:
/// - `allOf`: return the first sub-schema (typically the `$ref`)
/// - `anyOf`/`oneOf`: return the first non-null variant
fn effective_schema(schema: &Json) -> Option<&Json> {
    // allOf: return the first entry (usually the primary $ref)
    if let Some(all) = schema.get("allOf").and_then(|v| v.as_array())
        && let Some(first) = all.first()
    {
        return Some(first);
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

fn hover_content_inner(root: &Json, schema: &Json, effective: Option<&Json>) -> Option<String> {
    let mut parts = Vec::new();
    let mut fuel = FUEL;

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

    let ty = schema_type_string(root, schema, &mut fuel)
        .or_else(|| effective.and_then(|e| schema_type_string(root, e, &mut fuel)));
    if let Some(ty) = ty {
        parts.push(format!("**Type:** `{ty}`"));
    }

    let default = schema
        .get("default")
        .or_else(|| effective.and_then(|e| e.get("default")));
    if let Some(default) = default {
        parts.push(format!("**Default:** `{default}`"));
    }

    let enum_vals = schema
        .get("enum")
        .and_then(|e| e.as_array())
        .or_else(|| {
            effective
                .and_then(|e| e.get("enum"))
                .and_then(|e| e.as_array())
        })
        .or_else(|| {
            // For arrays of enums, pull enum values from the items schema.
            resolve_array_items_enum(root, schema, &mut fuel)
                .or_else(|| effective.and_then(|e| resolve_array_items_enum(root, e, &mut fuel)))
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

/// If the schema is an array type, resolve its items and return their enum values.
/// Follows `$ref`, `allOf`, `anyOf` to find the items schema.
fn resolve_array_items_enum<'a>(
    root: &'a Json,
    schema: &'a Json,
    fuel: &mut usize,
) -> Option<&'a Vec<Json>> {
    if !spend(fuel) {
        return None;
    }
    // Direct array with items
    if schema.get("type").and_then(|t| t.as_str()) == Some("array")
        && let Some(items) = schema.get("items")
    {
        let resolved = resolve_refs(root, items);
        if let Some(enum_vals) = resolved.get("enum").and_then(|e| e.as_array()) {
            return Some(enum_vals);
        }
    }
    // allOf: check each sub-schema
    if let Some(all) = schema.get("allOf").and_then(|v| v.as_array()) {
        for sub in all {
            let resolved = resolve_refs(root, sub);
            if let Some(vals) = resolve_array_items_enum(root, resolved, fuel) {
                return Some(vals);
            }
        }
    }
    // anyOf/oneOf: check each variant
    for keyword in &["anyOf", "oneOf"] {
        if let Some(variants) = schema.get(*keyword).and_then(|v| v.as_array()) {
            for variant in variants {
                let resolved = resolve_refs(root, variant);
                if let Some(vals) = resolve_array_items_enum(root, resolved, fuel) {
                    return Some(vals);
                }
            }
        }
    }
    None
}

/// Extract a human-readable type string from a schema node.
fn schema_type_string(root: &Json, schema: &Json, fuel: &mut usize) -> Option<String> {
    if !spend(fuel) {
        return None;
    }
    // Explicit "type" field
    if let Some(ty) = schema.get("type") {
        if let Some(s) = ty.as_str() {
            // Annotate arrays with their items type.
            if s == "array"
                && let Some(items) = schema.get("items")
                && let Some(label) = variant_type_label(root, items, fuel)
            {
                return Some(format!("{label}[]"));
            }
            return Some(s.to_string());
        }
        if let Some(arr) = ty.as_array() {
            let types: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
            return Some(types.join(" | "));
        }
    }

    // allOf: try each sub-schema
    if let Some(all) = schema.get("allOf").and_then(|v| v.as_array()) {
        for sub in all {
            let resolved = resolve_refs(root, sub);
            if let Some(ty) = schema_type_string(root, resolved, fuel) {
                return Some(ty);
            }
        }
    }

    // oneOf / anyOf: describe each variant
    for keyword in &["oneOf", "anyOf"] {
        if let Some(variants) = schema.get(*keyword).and_then(|v| v.as_array()) {
            let types: Vec<String> = variants
                .iter()
                .filter_map(|v| variant_type_label(root, v, fuel))
                .collect();
            if !types.is_empty() {
                return Some(types.join(" | "));
            }
        }
    }

    None
}

/// Produce a short type label for a single schema variant.
/// Takes the *unresolved* schema so `$ref` names can be preserved.
fn variant_type_label(root: &Json, schema: &Json, fuel: &mut usize) -> Option<String> {
    if !spend(fuel) {
        return None;
    }
    // Check for $ref first — use the definition name.
    if let Some(ref_str) = schema.get("$ref").and_then(|r| r.as_str()) {
        return Some(ref_str.rsplit('/').next().unwrap_or(ref_str).to_string());
    }

    // Simple type
    if let Some(ty) = schema.get("type").and_then(|t| t.as_str()) {
        if ty == "string"
            && let Some(vals) = schema.get("enum").and_then(|e| e.as_array())
        {
            let joined: Vec<String> = vals
                .iter()
                .filter_map(|v| v.as_str().map(|s| format!("\"{s}\"")))
                .collect();
            return Some(joined.join(" | "));
        }
        if ty == "array" {
            if let Some(items) = schema.get("items") {
                // Prefer $ref name for items, fall back to resolved type.
                if let Some(label) = variant_type_label(root, items, fuel) {
                    return Some(format!("{label}[]"));
                }
            }
            return Some("array".to_string());
        }
        return Some(ty.to_string());
    }

    // Nested composition — recurse
    schema_type_string(root, schema, fuel)
}
