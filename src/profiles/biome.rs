use crate::diagnostic::{Diagnostic, Severity, Span};

use super::{Injection, Profile, StreamParser};

pub const PROFILE: Profile = Profile {
    name: "biome",
    inject,
    parser,
};

// Biome's JSON reporter omits severity and uses byte-offset spans. Its `github`
// reporter is the one that carries per-diagnostic severity *and* line/column,
// and it's line-oriented, so it streams.
fn inject(args: &[String]) -> Injection {
    match super::flag_value(args, &["--reporter"]) {
        Some("github") => Injection::Append(Vec::new()),
        Some(other) => Injection::Unsupported(format!(
            "biome is set to `--reporter {other}`; simp needs `--reporter=github`"
        )),
        None => Injection::Append(vec!["--reporter=github".to_string()]),
    }
}

fn parser() -> Box<dyn StreamParser> {
    Box::new(BiomeParser)
}

struct BiomeParser;

impl StreamParser for BiomeParser {
    fn push_line(&mut self, line: &str) -> Vec<Diagnostic> {
        parse_line(line).into_iter().collect()
    }
}

/// Parse one GitHub Actions annotation line, e.g.
///   ::warning title=lint/correctness/noUnusedImports,file=a.ts,line=1,endLine=1,col=8,endColumn=18::This import is unused.
fn parse_line(line: &str) -> Option<Diagnostic> {
    let rest = line.strip_prefix("::")?;
    // Properties never contain `::` (colons in values are escaped), so the first
    // `::` separates the command + properties from the message.
    let (head, message) = rest.split_once("::")?;
    let (severity_word, properties) = head.split_once(' ')?;
    let severity = match severity_word {
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        "notice" => Severity::Info,
        _ => return None,
    };

    let mut file = None;
    let mut code = None;
    let mut line_no = None;
    let mut column = None;
    let mut end_line = None;
    let mut end_column = None;
    for property in properties.split(',') {
        let (key, value) = property.split_once('=')?;
        match key {
            "file" => file = Some(unescape_property(value)),
            "title" => code = Some(unescape_property(value)),
            "line" => line_no = value.parse().ok(),
            "col" => column = value.parse().ok(),
            "endLine" => end_line = value.parse().ok(),
            "endColumn" => end_column = value.parse().ok(),
            _ => {}
        }
    }

    Some(Diagnostic {
        source: "biome".to_string(),
        severity,
        file,
        span: line_no.map(|line| Span {
            line,
            column: column.unwrap_or(1),
            end_line,
            end_column,
        }),
        code,
        message: unescape_message(message),
    })
}

/// Undo GitHub's command-string escaping for a property value.
fn unescape_property(value: &str) -> String {
    unescape_message(value).replace("%2C", ",").replace("%3A", ":")
}

/// Undo GitHub's command-string escaping for message text. `%25` is decoded
/// last so a decoded `%` can't be reinterpreted as another escape.
fn unescape_message(value: &str) -> String {
    value
        .replace("%0A", "\n")
        .replace("%0D", "\r")
        .replace("%25", "%")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    // Captured verbatim from `biome lint --reporter=github`.
    const SAMPLE: &str = "::warning title=lint/correctness/noUnusedImports,file=sample.ts,line=1,endLine=1,col=8,endColumn=18::This import is unused.";

    #[test]
    fn parses_github_annotation() {
        let diagnostic = parse_line(SAMPLE).expect("a diagnostic");
        assert_eq!(diagnostic.source, "biome");
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert_eq!(diagnostic.file.as_deref(), Some("sample.ts"));
        assert_eq!(
            diagnostic.code.as_deref(),
            Some("lint/correctness/noUnusedImports")
        );
        assert_eq!(diagnostic.message, "This import is unused.");
        let span = diagnostic.span.unwrap();
        assert_eq!((span.line, span.column), (1, 8));
        assert_eq!((span.end_line, span.end_column), (Some(1), Some(18)));
    }

    #[test]
    fn maps_error_and_ignores_noise() {
        assert_eq!(
            parse_line("::error title=x,file=a.ts,line=2,col=1::boom")
                .unwrap()
                .severity,
            Severity::Error
        );
        assert!(parse_line("Checking files...").is_none());
    }

    #[test]
    fn unescapes_message_newline() {
        let diagnostic = parse_line("::error title=t,file=a.ts,line=1,col=1::a%0Ab").unwrap();
        assert_eq!(diagnostic.message, "a\nb");
    }

    #[test]
    fn injects_github_reporter_by_default() {
        assert_eq!(
            inject(&args(&["check", "./src"])),
            Injection::Append(args(&["--reporter=github"]))
        );
    }

    #[test]
    fn rejects_other_reporter() {
        assert!(matches!(
            inject(&args(&["lint", "--reporter", "json"])),
            Injection::Unsupported(_)
        ));
        assert_eq!(
            inject(&args(&["lint", "--reporter=github"])),
            Injection::Append(vec![])
        );
    }
}
