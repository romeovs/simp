use crate::diagnostic::{Diagnostic, Severity};

use super::StreamParser;

/// A streaming parser for formatters whose `--list-different` output is one
/// unformatted file path per line (prettier, oxfmt). Each path becomes a
/// file-level "not formatted" diagnostic, emitted as its line arrives.
pub struct ListDifferent {
    pub source: &'static str,
}

impl StreamParser for ListDifferent {
    fn push_line(&mut self, line: &str) -> Vec<Diagnostic> {
        let path = line.trim();
        if path.is_empty() {
            return Vec::new();
        }
        vec![Diagnostic {
            source: self.source.to_string(),
            severity: Severity::Warning,
            file: Some(path.to_string()),
            span: None,
            code: None,
            message: "not formatted".to_string(),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_diagnostic_per_path() {
        let mut parser = ListDifferent { source: "prettier" };
        let first = parser.push_line("src/a.ts");
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].source, "prettier");
        assert_eq!(first[0].file.as_deref(), Some("src/a.ts"));
        assert_eq!(first[0].severity, Severity::Warning);
        assert_eq!(first[0].message, "not formatted");
        assert!(first[0].span.is_none());

        assert!(parser.push_line("   ").is_empty());
        assert!(parser.finish().is_empty());
    }
}
