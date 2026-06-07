use std::io::Read;
use std::process::Command;

use anyhow::{Context, Result};

use crate::profiles::{Injection, Profile};

pub struct Captured {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Run the wrapped command, injecting the profile's format flags after the
/// subcommand args. Captures both streams and the real exit code.
pub fn run_wrapped(program: &str, args: &[String], profile: &Profile) -> Result<Captured> {
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

    let output = command
        .output()
        .with_context(|| format!("failed to spawn `{program}`"))?;

    Ok(Captured {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code().unwrap_or(1),
    })
}

/// Read all of stdin (pipe / fallback mode). We can't know the tool's exit
/// code here, so callers derive simp's exit from whether errors were found.
pub fn read_stdin() -> Result<Captured> {
    let mut stdout = String::new();
    std::io::stdin()
        .read_to_string(&mut stdout)
        .context("failed to read stdin")?;
    Ok(Captured {
        stdout,
        stderr: String::new(),
        exit_code: 0,
    })
}
