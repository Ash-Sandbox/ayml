//! Strongly typed Ash policy (v1) with comment preservation.
//!
//! Schema: https://hub.ashell.dev/schemas/policy/v1.json
//!
//! Every field is wrapped in `Commented<T>` so that AYML comments can be
//! attached to any value in the tree and survive round-trips.
//!
//! Run: cargo run -p ayml --example policy

use ayml::commented::Commented;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Top-level ───────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Policy {
    schema_version: Commented<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    publish: Option<Commented<PublishMetadata>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dependencies: Option<Commented<HashMap<String, Dependency>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    network: Option<Commented<NetworkPolicy>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    files: Option<Commented<FilesystemPolicy>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exec: Option<Commented<ExecPolicy>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    environment: Option<Commented<EnvironmentPolicy>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    io_devices: Option<Commented<IoDevicePolicy>>,
}

// ── Publish ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct PublishMetadata {
    name: Commented<String>,
    version: Commented<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<Commented<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    license: Option<Commented<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    homepage: Option<Commented<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repository: Option<Commented<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    authors: Vec<Commented<String>>,
}

// ── Dependencies ────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
enum Dependency {
    Short(String),
    Registry(RegistryDep),
    Local(LocalDep),
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct RegistryDep {
    version: Commented<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    registry: Option<Commented<String>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct LocalDep {
    path: Commented<String>,
}

// ── Network ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct NetworkPolicy {
    rules: Vec<Commented<NetworkRule>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct NetworkRule {
    host: Commented<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<Commented<Action>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    direction: Option<Commented<Direction>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ports: Option<Commented<Ports>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transports: Option<Commented<Transport>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    precedence: Option<Commented<i32>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Action {
    Allow,
    Deny,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Direction {
    Inbound,
    Outbound,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
enum Ports {
    All(PortsAll),
    List(Vec<u16>),
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum PortsAll {
    #[serde(rename = "all")]
    All,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Transport {
    Tcp,
    Udp,
    All,
}

// ── Filesystem ──────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct FilesystemPolicy {
    rules: Vec<Commented<FilesystemRule>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct FilesystemRule {
    path: Commented<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<Commented<Action>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    operations: Option<Commented<Vec<FileOperation>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    precedence: Option<Commented<i32>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum FileOperation {
    Read,
    Write,
    Create,
    Delete,
    Rename,
}

// ── Exec ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct ExecPolicy {
    rules: Vec<Commented<ExecRule>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct ExecRule {
    path: Commented<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<Commented<Action>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    args: Option<Commented<Vec<ArgSelector>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subcommand: Option<Commented<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    precedence: Option<Commented<i32>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
enum ArgSelector {
    Any(ArgSelectorKeyword),
    Flag {
        flag: String,
    },
    Option {
        option: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<String>,
    },
    Positional {
        positional: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        index: Option<u64>,
    },
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ArgSelectorKeyword {
    Any,
    AnyFlag,
    AnyOption,
    AnyPositional,
}

// ── Environment ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct EnvironmentPolicy {
    rules: Commented<EnvironmentRules>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct EnvironmentRules {
    #[serde(skip_serializing_if = "Option::is_none")]
    allow: Option<Commented<EnvironmentAllow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deny: Option<Commented<Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    set: Option<Commented<HashMap<String, String>>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
enum EnvironmentAllow {
    All(EnvAllowAll),
    List(Vec<String>),
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum EnvAllowAll {
    #[serde(rename = "all")]
    All,
}

// ── IO Devices ──────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct IoDevicePolicy {
    rules: Vec<Commented<IoDeviceRule>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct IoDeviceRule {
    class: Commented<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<Commented<Action>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    precedence: Option<Commented<i32>>,
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Shorthand: wrap a value with a top comment.
fn top<T>(comment: &str, value: T) -> Commented<T> {
    Commented {
        top_comment: Some(comment.into()),
        inline_comment: None,
        value,
    }
}

/// Shorthand: wrap a value with an inline comment.
fn inline<T>(value: T, comment: &str) -> Commented<T> {
    Commented {
        top_comment: None,
        inline_comment: Some(comment.into()),
        value,
    }
}

// ── Main ────────────────────────────────────────────────────────────

fn main() {
    let policy = Policy {
        schema_version: Commented::new(1),
        publish: Some(Commented::new(PublishMetadata {
            name: Commented::new("example/web-server".into()),
            version: Commented::new("1.0.0".into()),
            description: Some(Commented::new("Policy for a typical web server".into())),
            license: Some(inline("MIT".into(), "SPDX")),
            homepage: None,
            repository: None,
            authors: vec![Commented::new("alice".into())],
        })),
        dependencies: None,
        network: Some(top(
            "Network access rules",
            NetworkPolicy {
                rules: vec![
                    top(
                        "Allow HTTPS to our API",
                        NetworkRule {
                            host: Commented::new("*.example.com".into()),
                            action: None,
                            direction: Some(Commented::new(Direction::Outbound)),
                            ports: Some(top("HTTPS only", Ports::List(vec![443]))),
                            transports: Some(Commented::new(Transport::Tcp)),
                            precedence: None,
                        },
                    ),
                    top(
                        "Deny everything else",
                        NetworkRule {
                            host: inline("0.0.0.0/0".into(), "all IPv4"),
                            action: Some(Commented::new(Action::Deny)),
                            direction: None,
                            ports: Some(Commented::new(Ports::All(PortsAll::All))),
                            transports: None,
                            precedence: Some(inline(-1, "lowest priority")),
                        },
                    ),
                ],
            },
        )),
        files: Some(top(
            "Filesystem access rules",
            FilesystemPolicy {
                rules: vec![
                    top(
                        "Read-only access to the working directory",
                        FilesystemRule {
                            path: Commented::new("$CWD/**".into()),
                            action: None,
                            operations: Some(Commented::new(vec![FileOperation::Read])),
                            precedence: None,
                        },
                    ),
                    top(
                        "Full access to session temp directory",
                        FilesystemRule {
                            path: Commented::new("/tmp/$ASH_SESSION_ID/**".into()),
                            action: None,
                            operations: Some(Commented::new(vec![
                                FileOperation::Read,
                                FileOperation::Write,
                                FileOperation::Create,
                                FileOperation::Delete,
                            ])),
                            precedence: None,
                        },
                    ),
                ],
            },
        )),
        exec: Some(top(
            "Process execution rules",
            ExecPolicy {
                rules: vec![
                    Commented::new(ExecRule {
                        path: inline("/usr/bin/curl".into(), "HTTP client"),
                        action: None,
                        args: Some(Commented::new(vec![
                            ArgSelector::Flag {
                                flag: "--silent".into(),
                            },
                            ArgSelector::Option {
                                option: "--output".into(),
                                value: None,
                            },
                        ])),
                        subcommand: None,
                        precedence: None,
                    }),
                    Commented::new(ExecRule {
                        path: Commented::new("**/git".into()),
                        action: None,
                        args: None,
                        subcommand: Some(inline("status".into(), "read-only")),
                        precedence: None,
                    }),
                ],
            },
        )),
        environment: Some(top(
            "Environment variable rules",
            EnvironmentPolicy {
                rules: Commented::new(EnvironmentRules {
                    allow: Some(Commented::new(EnvironmentAllow::List(vec![
                        "PATH".into(),
                        "HOME".into(),
                        "LANG".into(),
                    ]))),
                    deny: None,
                    set: Some(Commented::new(HashMap::from([(
                        "NODE_ENV".into(),
                        "production".into(),
                    )]))),
                }),
            },
        )),
        io_devices: None,
    };

    // ── Serialize ───────────────────────────────────────────────────
    let ayml = ayml::to_string(&policy).expect("serialize");
    println!("── Serialized AYML ──\n");
    println!("{ayml}");

    // ── Deserialize back ────────────────────────────────────────────
    let roundtripped: Policy = ayml::from_str(&ayml).expect("deserialize");
    assert_eq!(policy, roundtripped, "roundtrip mismatch");
    println!("── Roundtrip OK ──");

    // ── Minimal seq comment test ──────────────────────────────────────
    let seq_test = "\
rules:
  # first rule
  - host: a
  # second rule
  - host: b
";
    let parsed_net: NetworkPolicy = ayml::from_str(seq_test).expect("parse seq_test");
    println!("Rule 0 top: {:?}", parsed_net.rules[0].top_comment);
    println!("Rule 1 top: {:?}", parsed_net.rules[1].top_comment);
    assert_eq!(
        parsed_net.rules[0].top_comment.as_deref(),
        Some("first rule")
    );
    assert_eq!(
        parsed_net.rules[1].top_comment.as_deref(),
        Some("second rule")
    );

    // ── Roundtrip a hand-written policy with comments ─────────────────
    //
    // The input uses the same formatting the serializer produces so
    // that the roundtrip is byte-identical.
    let hand_written = "\
schema_version: 1
network:
  # Network access rules
  rules:
        # Allow GitHub
    - host: *.github.com # primary domain
      direction: outbound
      ports:
        - 443
        - 22
        # Deny all other traffic
    - host: 0.0.0.0/0
      action: deny
      ports: all
      precedence: -1
";
    let parsed: Policy = ayml::from_str(hand_written).expect("parse hand-written");

    let net = parsed.network.as_ref().unwrap();
    assert_eq!(net.top_comment.as_deref(), Some("Network access rules"));

    let rule0 = &net.value.rules[0];
    assert_eq!(rule0.top_comment.as_deref(), Some("Allow GitHub"));
    assert_eq!(rule0.value.host.value, "*.github.com");
    assert_eq!(
        rule0.value.host.inline_comment.as_deref(),
        Some("primary domain")
    );

    let rule1 = &net.value.rules[1];
    assert_eq!(rule1.top_comment.as_deref(), Some("Deny all other traffic"));

    // Re-serialize and verify byte-identical roundtrip
    let reserialized = ayml::to_string(&parsed).expect("reserialize");
    assert_eq!(
        hand_written, reserialized,
        "hand-written roundtrip not identical"
    );
    println!("── Hand-written roundtrip OK ──");
}
