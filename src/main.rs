mod diagnostic;
mod format;
mod profiles;
mod runner;

use std::process::ExitCode;

use anyhow::{bail, Result};
use clap::Parser as _;

use crate::diagnostic::Report;
use crate::format::Format;
use crate::profiles::{Profile, RawOutput};
use crate::runner::Captured;

/// Normalize diagnostic-tool output into a consistent, token-efficient format.
#[derive(clap::Parser)]
#[command(name = "simp", version, trailing_var_arg = true)]
struct Cli {
    /// Output format.
    #[arg(long, value_enum, default_value_t = FormatArg::Flat, global = true)]
    format: FormatArg,

    /// Parse piped stdin instead of running a command, using this tool's profile.
    #[arg(long, value_name = "TOOL")]
    from: Option<String>,

    /// The tool to wrap and its arguments, e.g. `simp tsc --noEmit`.
    #[arg(value_name = "COMMAND", num_args = 0..)]
    command: Vec<String>,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum FormatArg {
    Flat,
    Json,
}

impl From<FormatArg> for Format {
    fn from(arg: FormatArg) -> Self {
        match arg {
            FormatArg::Flat => Format::Flat,
            FormatArg::Json => Format::Json,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("simp: {error:#}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    let format: Format = cli.format.into();

    let (profile_name, captured) = if let Some(from) = &cli.from {
        (from.clone(), runner::read_stdin()?)
    } else {
        let Some((program, args)) = cli.command.split_first() else {
            bail!(
                "no command given. Use `simp <tool> <args>` or `<tool> | simp --from <tool>`.\nKnown tools: {}",
                profiles::known_names().join(", ")
            );
        };
        let profile = resolve_or_bail(program)?;
        let captured = runner::run_wrapped(program, args, profile)?;
        (program.clone(), captured)
    };

    let profile = resolve_or_bail(&profile_name)?;
    let report = parse(profile, &captured);

    print!("{}", format::render(&report, format));

    Ok(exit_code_for(&report, cli.from.is_some()))
}

fn resolve_or_bail(name: &str) -> Result<&'static Profile> {
    profiles::resolve(name).ok_or_else(|| {
        anyhow::anyhow!(
            "no profile for `{name}`. Known tools: {}",
            profiles::known_names().join(", ")
        )
    })
}

fn parse(profile: &Profile, captured: &Captured) -> Report {
    let raw = RawOutput {
        stdout: &captured.stdout,
        stderr: &captured.stderr,
    };
    let diagnostics = (profile.parse)(&raw);
    Report::new(diagnostics, captured.exit_code)
}

/// In wrapper mode, mirror the tool's exit code so simp is transparent in CI.
/// In stdin mode there's no tool exit, so fail iff we parsed any errors.
fn exit_code_for(report: &Report, stdin_mode: bool) -> ExitCode {
    if stdin_mode {
        return if report.summary.errors > 0 {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        };
    }
    ExitCode::from(report.tool_exit.clamp(0, 255) as u8)
}
