use ayml_lsp::schema;

use serde_json::Value as Json;

/// A snapshot of <https://hub.ashell.dev/schemas/policy/v1.json>, vendored so
/// the tests run offline.
fn load_policy_schema() -> Json {
    serde_json::from_str(include_str!("fixtures/policy-v1.schema.json"))
        .expect("failed to parse schema JSON")
}

#[test]
fn network_type_shows_definition_name() {
    let root = load_policy_schema();
    let sub = schema::resolve_sub_schema(&root, &["network"]).unwrap();
    let content = schema::hover_content(&root, sub).unwrap();
    assert!(
        content.contains("NetworkPolicy"),
        "expected NetworkPolicy in type, got:\n{content}"
    );
}

#[test]
fn network_rules_type_shows_item_type() {
    let root = load_policy_schema();
    let sub = schema::resolve_sub_schema(&root, &["network", "rules"]).unwrap();
    let content = schema::hover_content(&root, sub).unwrap();
    assert!(
        content.contains("NetworkRule[]"),
        "expected NetworkRule[] in type, got:\n{content}"
    );
}

#[test]
fn ports_type_shows_variants() {
    let root = load_policy_schema();
    let sub = schema::resolve_sub_schema(&root, &["network", "rules", "0", "ports"]).unwrap();
    let content = schema::hover_content(&root, sub).unwrap();
    assert!(
        content.contains("integer[]") || content.contains("int"),
        "expected integer[] in type, got:\n{content}"
    );
}

#[test]
fn host_type() {
    let root = load_policy_schema();
    let sub = schema::resolve_sub_schema(&root, &["network", "rules", "0", "host"]).unwrap();
    let content = schema::hover_content(&root, sub).unwrap();
    assert!(!content.is_empty());
}

#[test]
fn action_type() {
    let root = load_policy_schema();
    let sub = schema::resolve_sub_schema(&root, &["network", "rules", "0", "action"]).unwrap();
    let content = schema::hover_content(&root, sub).unwrap();
    assert!(!content.is_empty());
}

#[test]
fn operations_key_shows_enum_values() {
    let root = load_policy_schema();
    let sub = schema::resolve_sub_schema(&root, &["files", "rules", "0", "operations"]).unwrap();
    let content = schema::hover_content(&root, sub).unwrap();
    assert!(
        content.contains("read") && content.contains("write") && content.contains("delete"),
        "expected enum values in operations hover, got:\n{content}"
    );
}

#[test]
fn operations_element_shows_enum_values() {
    let root = load_policy_schema();
    let sub =
        schema::resolve_sub_schema(&root, &["files", "rules", "0", "operations", "0"]).unwrap();
    let content = schema::hover_content(&root, sub).unwrap();
    assert!(
        content.contains("read") && content.contains("write") && content.contains("delete"),
        "expected enum values in operations element hover, got:\n{content}"
    );
}

#[test]
fn cyclic_schemas_terminate() {
    let root: Json = serde_json::from_str(
        r##"{"anyOf": [{"$ref": "#"}], "allOf": [{"$ref": "#"}], "oneOf": [{"$ref": "#/oneOf/0"}]}"##,
    )
    .unwrap();
    assert!(schema::resolve_sub_schema(&root, &["a", "b"]).is_none());
    assert!(schema::type_string(&root, &root).is_none());
    let _ = schema::hover_content(&root, &root);
}
