use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::thread;

use anyhow::{Context, Result};

use crate::diagnostic::Diagnostic;
use crate::profiles::{Injection, Profile};

/// Spawn the wrapped command and stream its stdout through the profile's parser,
/// handing each diagnostic to `on_diagnostic` as soon as it's parsed. Returns
/// the tool's exit code so simp can mirror it.
pub fn stream_command(
    program: &str,
    args: &[String],
    profile: &Profile,
    mut on_diagnostic: impl FnMut(Diagnostic),
) -> Result<i32> {
    let mut command = Command::new(program);
    command.args(args);
    match (profile.inject)(args) {
        Injection::Append(flags) => {
            command.args(&flags);
        }
        // We can't make the output parseable, but still run so the tool's exit
        // code passes through; warn so empty diagnostics aren't a mystery.
        Injection::Unsupported(reason) => {
            eprintln!("simp: {reason}; diagnostics may be empty");
        }
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .with_context(|| format!("failed to spawn `{program}`"))?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");

    // Drain stderr on a separate thread so a full pipe can't deadlock the child
    // while we're busy streaming stdout. We keep the text for the fallback below.
    let stderr_drain = thread::spawn(move || {
        let mut text = String::new();
        let _ = BufReader::new(stderr).read_to_string(&mut text);
        text
    });

    let produced = stream_lines(BufReader::new(stdout), profile, &mut on_diagnostic)?;

    let stderr_text = stderr_drain.join().unwrap_or_default();
    // Some tools emit diagnostics on stderr; only consult it when stdout gave us
    // nothing, to avoid double-reporting.
    if !produced {
        stream_lines(stderr_text.as_bytes(), profile, &mut on_diagnostic)?;
    }

    let status = child.wait().context("waiting for tool to exit")?;
    Ok(status.code().unwrap_or(1))
}

/// Feed a reader's lines through a fresh parser, returning whether any
/// diagnostic was produced.
fn stream_lines(
    reader: impl BufRead,
    profile: &Profile,
    on_diagnostic: &mut dyn FnMut(Diagnostic),
) -> Result<bool> {
    let mut parser = (profile.parser)();
    let mut produced = false;
    let mut emit = |diagnostics: Vec<Diagnostic>| {
        for diagnostic in diagnostics {
            produced = true;
            on_diagnostic(diagnostic);
        }
    };
    for line in reader.lines() {
        let line = line.context("reading tool output")?;
        emit(parser.push_line(&line));
    }
    emit(parser.finish());
    Ok(produced)
}
