use std::io::{self, Write};

use crate::diagnostic::{Diagnostic, Report, Span, Summary, SummaryAccumulator};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Flat one-diagnostic-per-line, self-contained. Default.
    Flat,
    /// Machine-readable JSON for downstream tooling.
    Json,
}

/// Renders diagnostics to an output as they stream in. `flat` writes each
/// diagnostic immediately and prints a summary at the end; `json` must buffer,
/// since a single JSON document can't be emitted incrementally.
pub struct Renderer<W: Write> {
    out: W,
    format: Format,
    summary: SummaryAccumulator,
    buffered: Vec<Diagnostic>,
    printed_any: bool,
}

impl<W: Write> Renderer<W> {
    pub fn new(out: W, format: Format) -> Self {
        Renderer {
            out,
            format,
            summary: SummaryAccumulator::default(),
            buffered: Vec::new(),
            printed_any: false,
        }
    }

    /// Take one diagnostic. In flat mode it's written right away.
    pub fn diagnostic(&mut self, diagnostic: Diagnostic) {
        self.summary.add(&diagnostic);
        match self.format {
            Format::Flat => {
                // Best-effort: a broken pipe downstream shouldn't crash simp.
                let _ = writeln!(self.out, "{}", flat_line(&diagnostic));
                self.printed_any = true;
            }
            Format::Json => self.buffered.push(diagnostic),
        }
    }

    pub fn error_count(&self) -> usize {
        self.summary.errors()
    }

    /// Emit the trailing summary (flat) or the whole report (json).
    pub fn finish(mut self, tool_exit: i32) -> io::Result<()> {
        match self.format {
            Format::Flat => {
                if self.printed_any {
                    writeln!(self.out)?;
                }
                writeln!(self.out, "{}", summary_line(&self.summary.summary()))?;
            }
            Format::Json => {
                let report = Report::new(self.buffered, tool_exit);
                writeln!(self.out, "{}", serde_json::to_string_pretty(&report)?)?;
            }
        }
        Ok(())
    }
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

fn summary_line(summary: &Summary) -> String {
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
