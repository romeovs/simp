use crate::diagnostic::{Diagnostic, Severity, Span};

use super::{Injection, Profile, RawOutput};

pub const PROFILE: Profile = Profile {
    name: "tsc",
    inject,
    parse,
};

// tsc has no stable JSON reporter; we parse its non-pretty text form, so we
// need `--pretty false`. If the user already forced pretty output on, we can't
// parse it.
fn inject(args: &[String]) -> Injection {
    match pretty_flag(args) {
        Some(false) => Injection::Append(Vec::new()),
        Some(true) => Injection::Unsupported(
            "tsc `--pretty` is enabled; simp needs `--pretty false` to parse output".to_string(),
        ),
        None => Injection::Append(vec!["--pretty".to_string(), "false".to_string()]),
    }
}

/// Read tsc's `--pretty` setting if present. Handles `--pretty=false`,
/// `--pretty false`, and a bare `--pretty` (which tsc treats as on).
fn pretty_flag(args: &[String]) -> Option<bool> {
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if let Some(value) = arg.strip_prefix("--pretty=") {
            return Some(value != "false");
        }
        if arg == "--pretty" {
            // A following `false` turns it off; anything else (another flag, or
            // an explicit `true`) leaves it on.
            let on = iter.peek().map(|value| value.as_str()) != Some("false");
            return Some(on);
        }
    }
    None
}

fn parse(raw: &RawOutput) -> Vec<Diagnostic> {
    // tsc writes diagnostics to stdout; fall back to stderr just in case.
    let body = if raw.stdout.trim().is_empty() {
        raw.stderr
    } else {
        raw.stdout
    };
    body.lines().filter_map(parse_line).collect()
}

/// Parse one `--pretty false` tsc line. Two shapes are handled:
///   src/api.ts(12,5): error TS2304: Cannot find name 'foo'.
///   error TS18003: No inputs were found in config file ...
fn parse_line(line: &str) -> Option<Diagnostic> {
    let (location, rest) = split_location(line);

    // `rest` starts at "<severity> TS<code>: <message>".
    let (severity_word, after_severity) = rest.split_once(' ')?;
    let severity = match severity_word {
        "error" => Severity::Error,
        "warning" => Severity::Warning,
        _ => return None,
    };

    let (code, message) = after_severity.split_once(": ")?;
    if !code.starts_with("TS") {
        return None;
    }

    Some(Diagnostic {
        source: "tsc".to_string(),
        severity,
        file: location.as_ref().map(|loc| loc.file.clone()),
        span: location.map(|loc| Span {
            line: loc.line,
            column: loc.column,
            end_line: None,
            end_column: None,
        }),
        code: Some(code.to_string()),
        message: message.trim().to_string(),
    })
}

struct Location {
    file: String,
    line: u32,
    column: u32,
}

/// Split a leading `path(line,col): ` location off the front of a line.
/// Returns the parsed location (if present) and the remainder after `): `.
fn split_location(line: &str) -> (Option<Location>, &str) {
    let Some(marker) = line.find("): ") else {
        return (None, line);
    };
    let (head, tail) = line.split_at(marker);
    let rest = &tail["): ".len()..];

    let Some(open) = head.rfind('(') else {
        return (None, line);
    };
    let file = &head[..open];
    let coords = &head[open + 1..];
    let Some((line_str, col_str)) = coords.split_once(',') else {
        return (None, line);
    };
    let (Ok(line_no), Ok(col_no)) = (line_str.parse(), col_str.parse()) else {
        return (None, line);
    };

    (
        Some(Location {
            file: file.to_string(),
            line: line_no,
            column: col_no,
        }),
        rest,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(input: &str) -> Vec<Diagnostic> {
        parse(&RawOutput {
            stdout: input,
            stderr: "",
        })
    }

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn injects_pretty_false_by_default() {
        assert_eq!(
            inject(&args(&["--noEmit"])),
            Injection::Append(args(&["--pretty", "false"]))
        );
    }

    #[test]
    fn respects_existing_pretty_false() {
        assert_eq!(inject(&args(&["--pretty", "false"])), Injection::Append(vec![]));
        assert_eq!(inject(&args(&["--pretty=false"])), Injection::Append(vec![]));
    }

    #[test]
    fn rejects_pretty_on() {
        assert!(matches!(inject(&args(&["--pretty"])), Injection::Unsupported(_)));
        assert!(matches!(inject(&args(&["--pretty", "true"])), Injection::Unsupported(_)));
        assert!(matches!(inject(&args(&["--pretty=true"])), Injection::Unsupported(_)));
    }

    #[test]
    fn located_error() {
        let diagnostics = run("src/api.ts(12,5): error TS2304: Cannot find name 'foo'.");
        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.file.as_deref(), Some("src/api.ts"));
        let span = diagnostic.span.unwrap();
        assert_eq!((span.line, span.column), (12, 5));
        assert_eq!(diagnostic.code.as_deref(), Some("TS2304"));
        assert_eq!(diagnostic.message, "Cannot find name 'foo'.");
        assert_eq!(diagnostic.severity, Severity::Error);
    }

    #[test]
    fn unlocated_error() {
        let diagnostics = run("error TS18003: No inputs were found in config file.");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].file.is_none());
        assert!(diagnostics[0].span.is_none());
        assert_eq!(diagnostics[0].code.as_deref(), Some("TS18003"));
    }

    #[test]
    fn ignores_noise() {
        let diagnostics = run("Found 2 errors in 1 file.\n\nCompiling...");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn path_with_spaces() {
        let diagnostics = run("src/my file.ts(3,1): warning TS6133: 'x' is declared but never used.");
        assert_eq!(diagnostics[0].file.as_deref(), Some("src/my file.ts"));
        assert_eq!(diagnostics[0].severity, Severity::Warning);
    }
}
