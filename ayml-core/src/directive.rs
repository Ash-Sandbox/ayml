/// Extract a JSON Schema URI from a document's leading comment.
///
/// Recognizes directives of the form:
///
/// ```text
/// # yaml-language-server: $schema=<uri>
/// # language-server: $schema=<uri>
/// ```
///
/// The comment text passed here should already have the `# ` prefix stripped
/// (as stored in [`Node::comment`]).
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
    fn extra_whitespace() {
        let comment = "  yaml-language-server:   $schema=https://example.com/s.json  ";
        assert_eq!(schema_uri(comment), Some("https://example.com/s.json"));
    }
}
