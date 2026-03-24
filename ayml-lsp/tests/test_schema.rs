#[path = "../src/schema.rs"]
#[allow(dead_code)]
mod schema;

use serde_json::Value as Json;

fn load_policy_schema() -> Json {
    let body = ureq::get("https://hub.ashell.dev/schemas/policy/v1.json")
        .call()
        .expect("failed to fetch policy schema")
        .body_mut()
        .read_to_string()
        .expect("failed to read schema body");
    serde_json::from_str(&body).expect("failed to parse schema JSON")
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
