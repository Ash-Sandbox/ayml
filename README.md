# AYML

> [!NOTE]
> Status: Beta Software, Handle With Care

Simplified YAML variant.

AYML is a software configuration language that looks like YAML and acts like JSON. It is meant to be a human-friendly, cross language, Unicode based software configuration language. Unlike YAML, it is not designed as a fully featured data serialization framework. Most YAML features are omitted by design to avoid unintentional or malicious misuse.

## Example

It looks a lot like YAML:

```ayml
# yaml-language-server: $schema=https://hub.ashell.dev/schemas/policy/v1.json
schema_version: 1
dependencies:
  # default macOS permissions
  ash/base-macos: ^0
  # allow git command line tools
  ash/git: ^0
  # Ash internal team policy, hosted locally
  ash/internal:
    path: ~/.ash/policies/internal.ayml
files:
  rules:
    # full access to everything in the current directory
    - path: ./**
network:
  rules:
    - host: **.crates.io # crate publishing
      ports:
        - 443
```

## Goals

Design goals, prioritized:

1. simple to read and understand structured data
2. simple to write correctly, hard to write incorrectly
3. deserializes naturally into strongly typed data structures
4. deserializes naturally into dynamic language core types
5. parses comments as a formal part of the document structure

## Omitted

- YES/NO for booleans
- tags
- structures (anchors, aliases, `---` separators)
- unordered sets
- duplicate keys
- JSON compatibility quirks
- empty nodes
- `?` indicators
- directives
- document prefixes
- multi-document files
- single-quoted strings and block scalars
- non-string keys

## Testing

Run unit, integration, and property tests:

```bash
cargo test
```

Run fuzz tests:

```bash
rustup toolchain install nightly
cargo install cargo-fuzz
cd ayml/fuzz

# Parse arbitrary bytes into `ayml::Value`
cargo +nightly fuzz run fuzz_deserialize

# Parse, serialize, and parse again with serde
cargo +nightly fuzz run fuzz_roundtrip

# Parse and check that round trips produce identical output
cargo +nightly fuzz run fuzz_core_roundtrip

# Deserialize arbitrary bytes into a struct
cargo +nightly fuzz run fuzz_typed

# Check that streaming and tree-based deserialization produce identical output
cargo +nightly fuzz run fuzz_streaming_vs_tree

# Limit to 5 minutes
cargo +nightly fuzz run fuzz_deserialize -- -max_total_time=300
```

Run benchmarks:

```sh
cargo bench
```

All tests performed on a Macbook Pro M4 Max. Deserialization benchmarks are typed. Deserialization into an untyped value is roughly half as fast.

| Benchmark   | Deserialize | Serialize |
| ----------- | ----------- | --------- |
| flat        | 211 MiB/s   | 374 MiB/s |
| nested      | 211 MiB/s   | 437 MiB/s |
| seq_of_maps | 232 MiB/s   | 477 MiB/s |
| strings     | 445 MiB/s   | 824 MiB/s |
| large_50    | 262 MiB/s   | 535 MiB/s |

If max performance is a requirement, consider `serde_json`. Comparative benchmarks show it is about 2-3x faster than AYML.

## Acknowledgements

- The AYML specification is derived mainly from the [YAML specification](https://yaml.org/spec).
- The AYML serde implementation is heavily influenced by [serde_json](https://github.com/serde-rs/json)
