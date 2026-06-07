use std::fmt::Write as _;

use crate::diagnostic::{Diagnostic, Report, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Flat one-diagnostic-per-line, self-contained. Default.
    Flat,
    /// Machine-readable JSON for downstream tooling.
    Json,
}

pub fn render(report: &Report, format: Format) -> String {
    match format {
        Format::Flat => render_flat(report),
        Format::Json => serde_json::to_string_pretty(report).unwrap_or_default(),
    }
}

fn render_flat(report: &Report) -> String {
    let mut out = String::new();
    for diagnostic in &report.diagnostics {
        writeln!(out, "{}", flat_line(diagnostic)).ok();
    }
    if !report.diagnostics.is_empty() {
        out.push('\n');
    }
    out.push_str(&summary_line(report));
    out.push('\n');
    out
}

fn flat_line(diagnostic: &Diagnostic) -> String {
    let location = match (&diagnostic.file, &diagnostic.span) {
        (Some(file), Some(span)) => format!("{file}:{}", span_coords(span)),
        (Some(file), None) => file.clone(),
        (None, _) => "<unknown>".to_string(),
    };
    let mut line = format!("{} {location}", diagnostic.severity.label());
    if let Some(code) = &diagnostic.code {
        line.push(' ');
        line.push_str(code);
    }
    line.push(' ');
    line.push_str(&diagnostic.message);
    line
}

fn span_coords(span: &Span) -> String {
    format!("{}:{}", span.line, span.column)
}

fn summary_line(report: &Report) -> String {
    let summary = &report.summary;
    let mut parts = Vec::new();
    if summary.errors > 0 {
        parts.push(pluralize(summary.errors, "error"));
    }
    if summary.warnings > 0 {
        parts.push(pluralize(summary.warnings, "warning"));
    }
    if summary.infos > 0 {
        parts.push(pluralize(summary.infos, "info"));
    }
    if parts.is_empty() {
        return "no problems".to_string();
    }
    format!("{}, {}", parts.join(", "), pluralize(summary.files, "file"))
}

fn pluralize(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}
