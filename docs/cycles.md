# Dependency cycles

AYML is checked by [`spaghetti`](https://github.com/Ash-Sandbox/spaghetti), which
detects dependency cycles and fails when new ones appear.

    spaghetti check      # fail on cycles not in the baseline
    spaghetti list       # show every cycle
    spaghetti explain ID # show the edges of one cycle

The baseline in `spaghetti.baseline.json` records 12 cycles. All 12 are
**structural** — they follow from the shape of the data format, the grammar, or
the serde API, and cannot be removed without giving up one of the project's
design goals. They are documented here so the analysis is not repeated.

`fail_on_stale` is enabled: if a cycle below is ever genuinely eliminated, the
baseline must shrink with it, so the count can only ratchet downward.

## Recursive document tree (4 cycles)

| id | cycle |
| --- | --- |
| `t:8da1df3be9afd813` | `Node` ↔ `Value` |
| `t:b9cac5fd27416ff3` | `CommentedValue` ↔ `CommentedValueKind` |
| `t:857a9afbac87fd13` | `Commented` → `CommentedValueKind` → `CommentedValue` |
| `t:715f6743fc6cfcf7` | `Commented` ↔ `CommentedValueKind` |

An AYML document is a tree: a sequence holds nodes, and a node holds a value
that may itself be a sequence. `Value::Seq(Vec<Node>)` and `Node { value: Value }`
are two halves of that single recursive type, and the `Commented*` pair mirrors it
for the comment-preserving tree. The `Display` edges recurse for the same reason —
formatting a tree walks the tree.

Breaking these means flattening the tree into an arena of ids, or dropping the
per-node wrapper that carries comments and spans. The first trades a type cycle
for arena plumbing through the parser, emitter, serde layer, and LSP; the second
gives up comments as part of the document structure (goal 5) and the spans the
LSP reports positions from. Both cost more than the cycle does.

## serde's required shapes (4 cycles)

| id | cycle |
| --- | --- |
| `t:90f6740ebc4739c6` | `Value` ↔ `ValueVisitor` |
| `t:5e054264a0f8c6ae` | `CommentedValueKind` ↔ `CommentedValueKindVisitor` |
| `t:7c297cddb4b9aa6e` | `Commented` ↔ `CommentedVisitor` |
| `t:2fe2d60952c2b755` | `CommentedAccess` ↔ `Deserializer` |

The first three are the visitor pattern that `serde::Deserialize` mandates:
`deserialize` names its visitor, and the visitor's `Value` associated type names
the type back. The fourth is the `MapAccess` pattern — the accessor borrows the
`Deserializer` that constructs it. Every serde `Deserializer` has this shape.
Neither can be written any other way through serde's public API.

These are not produced by the `trait_impl_edges` setting; they are real call and
declaration edges, and turning that setting off does not remove them.

## Recursive-descent grammar (4 cycles)

| id | cycle |
| --- | --- |
| `f:312782ce48477381` | `parse_flow_node` → `parse_flow_sequence` |
| `f:413176c4b65fecf0` | `parse_flow_node` → `parse_flow_mapping` |
| `f:babd212aa615be75` | `parse_block_mapping` → `parse_mapping_value` → `try_block_mapping` |
| `f:f0ccec816a49d6c1` | `parse_block_sequence` → `parse_seq_entry_value` → `try_block_sequence` |

The parser is recursive descent, so its call graph is the grammar: a flow
sequence contains flow nodes, and a flow node may be a flow sequence. The mutual
recursion between these functions *is* the production rules, which is what makes
`grammar.rs` readable against `spec.md`.

Removing it means rewriting the parser as an explicit state machine over a manual
stack. That converts a set of short functions that each read like one grammar
rule into interleaved state transitions — a large, risky change to
spec-conformance-tested code that trades away goal 1 (easy to read and
understand) to move a number. Unbounded recursion is already handled: nesting
depth is capped by `enter_nested`/`leave_nested` in `parse_flow_node` and
`parse_indented_value`.

## What would count as a real cycle

The baseline exists to catch the cycles that are *not* on this list: a type in
one module reaching back into a module that already depends on it, a helper that
calls back into its own caller's layer, or `ayml-core` acquiring a dependency on
`ayml`. Those are the tangles worth failing CI over, and there are currently none.
