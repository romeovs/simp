use serde::Deserialize;

use crate::diagnostic::{Diagnostic, Severity, Span};

use super::{Injection, Profile, StreamParser};

pub const PROFILE: Profile = Profile {
    name: "eslint",
    inject,
    parser,
};

// eslint's `--format json` is a single JSON array emitted at the end, so it
// can't be parsed incrementally: we accumulate the document and parse it whole
// once the stream closes.
fn parser() -> Box<dyn StreamParser> {
    Box::new(EslintParser::default())
}

#[derive(Default)]
struct EslintParser {
    document: String,
}

impl StreamParser for EslintParser {
    fn push_line(&mut self, line: &str) -> Vec<Diagnostic> {
        self.document.push_str(line);
        self.document.push('\n');
        Vec::new()
    }

    fn finish(&mut self) -> Vec<Diagnostic> {
        parse(&self.document)
    }
}

// eslint can emit many report formats; simp parses the JSON one. Respect an
// existing `--format json`, and refuse to fight a different formatter the user
// asked for.
fn inject(args: &[String]) -> Injection {
    match super::flag_value(args, &["-f", "--format"]) {
        Some("json") => Injection::Append(Vec::new()),
        Some(other) => Injection::Unsupported(format!(
            "eslint is set to `--format {other}`; simp needs `--format json`"
        )),
        None => Injection::Append(vec!["--format".to_string(), "json".to_string()]),
    }
}

fn parse(document: &str) -> Vec<Diagnostic> {
    let files: Vec<FileResult> = match serde_json::from_str(document) {
        Ok(files) => files,
        Err(_) => return Vec::new(),
    };

    files
        .into_iter()
        .flat_map(|file| {
            let path = file.file_path;
            file.messages.into_iter().map(move |message| Diagnostic {
                source: "eslint".to_string(),
                severity: severity_from(message.severity),
                file: Some(path.clone()),
                span: span_from(&message),
                code: message.rule_id,
                message: message.message,
            })
        })
        .collect()
}

#[derive(Deserialize)]
struct FileResult {
    #[serde(rename = "filePath")]
    file_path: String,
    messages: Vec<Message>,
}

#[derive(Deserialize)]
struct Message {
    #[serde(rename = "ruleId")]
    rule_id: Option<String>,
    severity: u8,
    message: String,
    line: Option<u32>,
    column: Option<u32>,
    #[serde(rename = "endLine")]
    end_line: Option<u32>,
    #[serde(rename = "endColumn")]
    end_column: Option<u32>,
}

fn severity_from(value: u8) -> Severity {
    match value {
        2 => Severity::Error,
        1 => Severity::Warning,
        _ => Severity::Info,
    }
}

fn span_from(message: &Message) -> Option<Span> {
    Some(Span {
        line: message.line?,
        column: message.column.unwrap_or(1),
        end_line: message.end_line,
        end_column: message.end_column,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[
        {
            "filePath": "/repo/src/api.ts",
            "messages": [
                {
                    "ruleId": "no-unused-vars",
                    "severity": 2,
                    "message": "'foo' is defined but never used.",
                    "line": 12,
                    "column": 5,
                    "endLine": 12,
                    "endColumn": 8
                },
                {
                    "ruleId": null,
                    "severity": 1,
                    "message": "Parsing skipped.",
                    "line": 1,
                    "column": 1
                }
            ]
        }
    ]"#;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    /// Drive the document through the streaming parser line by line, which is
    /// how the runner feeds it — exercises accumulation as well as parsing.
    fn run(input: &str) -> Vec<Diagnostic> {
        let mut parser = EslintParser::default();
        for line in input.lines() {
            assert!(parser.push_line(line).is_empty());
        }
        parser.finish()
    }

    #[test]
    fn injects_json_format_by_default() {
        assert_eq!(
            inject(&args(&["./src"])),
            Injection::Append(args(&["--format", "json"]))
        );
    }

    #[test]
    fn respects_existing_json_format() {
        assert_eq!(inject(&args(&["-f", "json"])), Injection::Append(vec![]));
        assert_eq!(inject(&args(&["--format=json"])), Injection::Append(vec![]));
    }

    #[test]
    fn rejects_other_formatter() {
        assert!(matches!(
            inject(&args(&["--format", "stylish"])),
            Injection::Unsupported(_)
        ));
    }

    #[test]
    fn parses_messages() {
        let diagnostics = run(SAMPLE);
        assert_eq!(diagnostics.len(), 2);

        let first = &diagnostics[0];
        assert_eq!(first.file.as_deref(), Some("/repo/src/api.ts"));
        assert_eq!(first.code.as_deref(), Some("no-unused-vars"));
        assert_eq!(first.severity, Severity::Error);
        let span = first.span.unwrap();
        assert_eq!((span.line, span.column), (12, 5));
        assert_eq!(span.end_column, Some(8));

        let second = &diagnostics[1];
        assert!(second.code.is_none());
        assert_eq!(second.severity, Severity::Warning);
    }

    #[test]
    fn empty_on_garbage() {
        assert!(run("not json").is_empty());
    }
}
