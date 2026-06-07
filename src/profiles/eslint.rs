use serde::Deserialize;

use crate::diagnostic::{Diagnostic, Severity, Span};

use super::{Injection, Profile, RawOutput};

pub const PROFILE: Profile = Profile {
    name: "eslint",
    inject,
    parse,
};

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

fn parse(raw: &RawOutput) -> Vec<Diagnostic> {
    let files: Vec<FileResult> = match serde_json::from_str(raw.stdout) {
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
        let diagnostics = parse(&RawOutput {
            stdout: SAMPLE,
            stderr: "",
        });
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
        let diagnostics = parse(&RawOutput {
            stdout: "not json",
            stderr: "",
        });
        assert!(diagnostics.is_empty());
    }
}
