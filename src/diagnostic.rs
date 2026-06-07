use std::collections::BTreeSet;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Span {
    pub line: u32,
    pub column: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_column: Option<u32>,
}

/// A single normalized diagnostic. Every parser maps its tool's output into
/// this shape; every formatter renders out of it.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub source: String,
    pub severity: Severity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub message: String,
}

#[derive(Debug, Default, Serialize)]
pub struct Summary {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub files: usize,
}

/// Tallies diagnostics as they stream past, so the flat formatter can print a
/// summary without holding every diagnostic in memory.
#[derive(Default)]
pub struct SummaryAccumulator {
    errors: usize,
    warnings: usize,
    infos: usize,
    files: BTreeSet<String>,
}

impl SummaryAccumulator {
    pub fn add(&mut self, diagnostic: &Diagnostic) {
        match diagnostic.severity {
            Severity::Error => self.errors += 1,
            Severity::Warning => self.warnings += 1,
            Severity::Info => self.infos += 1,
        }
        if let Some(file) = &diagnostic.file {
            self.files.insert(file.clone());
        }
    }

    pub fn summary(&self) -> Summary {
        Summary {
            errors: self.errors,
            warnings: self.warnings,
            infos: self.infos,
            files: self.files.len(),
        }
    }
}

/// The full result of running and parsing one tool invocation.
#[derive(Debug, Serialize)]
pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
    /// The wrapped tool's real exit code, so simp can mirror it.
    pub tool_exit: i32,
    pub summary: Summary,
}

impl Report {
    pub fn new(diagnostics: Vec<Diagnostic>, tool_exit: i32) -> Self {
        let summary = summarize(&diagnostics);
        Report {
            diagnostics,
            tool_exit,
            summary,
        }
    }
}

fn summarize(diagnostics: &[Diagnostic]) -> Summary {
    let mut summary = Summary::default();
    let mut files = BTreeSet::new();
    for diagnostic in diagnostics {
        match diagnostic.severity {
            Severity::Error => summary.errors += 1,
            Severity::Warning => summary.warnings += 1,
            Severity::Info => summary.infos += 1,
        }
        if let Some(file) = &diagnostic.file {
            files.insert(file.clone());
        }
    }
    summary.files = files.len();
    summary
}
