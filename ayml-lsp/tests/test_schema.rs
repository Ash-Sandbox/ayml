#[path = "../src/schema.rs"]
mod schema;

use serde_json::Value as Json;

fn load_policy_schema() -> Json {
    let content = include_str!("/tmp/policy_schema.json");
    serde_json::from_str(content).unwrap()
}

#[test]
fn network_type_shows_definition_name() {
    let root = load_policy_schema();
    let sub = schema::resolve_sub_schema(&root, &["network"]).unwrap();
    let content = schema::hover_content(&root, sub).unwrap();
    eprintln!("network hover:\n{content}\n");
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
    eprintln!("network.rules hover:\n{content}\n");
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
    eprintln!("ports hover:\n{content}\n");
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
    eprintln!("host hover:\n{content}\n");
    assert!(!content.is_empty());
}

#[test]
fn action_type() {
    let root = load_policy_schema();
    let sub = schema::resolve_sub_schema(&root, &["network", "rules", "0", "action"]).unwrap();
    let content = schema::hover_content(&root, sub).unwrap();
    eprintln!("action hover:\n{content}\n");
    assert!(!content.is_empty());
}
