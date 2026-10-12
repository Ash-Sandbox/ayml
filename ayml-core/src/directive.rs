use crate::value::{Node, Value};

/// Find a document's schema directive.
#[must_use]
pub fn document_schema_uri(root: &Node) -> Option<&str> {
    let first_entry = match &root.value {
        Value::Seq(items) => items.first(),
        Value::Map(map) => map.first().map(|(_, v)| v),
        _ => None,
    };
    root.comment
        .as_deref()
        .or_else(|| first_entry.and_then(|e| e.comment.as_deref()))
        .and_then(schema_uri)
}

/// Extract a JSON Schema URI from a comment.
///
/// Recognizes directives of the form:
///
/// ```text
/// # yaml-language-server: $schema=<uri>
/// # language-server: $schema=<uri>
/// ```
///
/// The comment text passed here should already have the `# ` prefix stripped
/// (as stored in [`Node::comment`](crate::Node::comment)).
///
/// Returns `None` if no schema directive is found.
#[must_use]
pub fn schema_uri(comment: &str) -> Option<&str> {
    for line in comment.lines() {
        let trimmed = line.trim();
        // Try both recognized prefixes.
        let rest = trimmed
            .strip_prefix("yaml-language-server:")
            .or_else(|| trimmed.strip_prefix("language-server:"));
        if let Some(rest) = rest {
            let rest = rest.trim();
            if let Some(uri) = rest.strip_prefix("$schema=") {
                let uri = uri.trim();
                if !uri.is_empty() {
                    return Some(uri);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_language_server_prefix() {
        let comment = "yaml-language-server: $schema=https://example.com/schema.json";
        assert_eq!(schema_uri(comment), Some("https://example.com/schema.json"));
    }

    #[test]
    fn language_server_prefix() {
        let comment = "language-server: $schema=https://example.com/schema.json";
        assert_eq!(schema_uri(comment), Some("https://example.com/schema.json"));
    }

    #[test]
    fn directive_among_other_comments() {
        let comment = "This is a config file\nlanguage-server: $schema=https://example.com/s.json\nMore comments";
        assert_eq!(schema_uri(comment), Some("https://example.com/s.json"));
    }

    #[test]
    fn no_directive() {
        assert_eq!(schema_uri("just a normal comment"), None);
    }

    #[test]
    fn empty_uri() {
        assert_eq!(schema_uri("language-server: $schema="), None);
    }

    #[test]
    fn document_directive_on_first_entry() {
        let root = crate::parse("# yaml-language-server: $schema=s.json\na: 1\n").unwrap();
        assert_eq!(document_schema_uri(&root), Some("s.json"));
        let root = crate::parse("# language-server: $schema=t.json\n[1]\n").unwrap();
        assert_eq!(document_schema_uri(&root), Some("t.json"));
        let root = crate::parse("a: 1\n# language-server: $schema=t.json\nb: 2\n").unwrap();
        assert_eq!(document_schema_uri(&root), None);
    }

    #[test]
    fn extra_whitespace() {
        let comment = "  yaml-language-server:   $schema=https://example.com/s.json  ";
        assert_eq!(schema_uri(comment), Some("https://example.com/s.json"));
    }
}
