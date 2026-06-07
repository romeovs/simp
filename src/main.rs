mod diagnostic;
mod format;
mod profiles;
mod runner;

use std::process::ExitCode;

use anyhow::{bail, Result};
use clap::Parser as _;

use crate::format::{Format, Renderer};
use crate::profiles::Profile;

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
    let stdout = std::io::stdout();
    let mut renderer = Renderer::new(stdout.lock(), format);

    // `None` in stdin mode, where there's no wrapped tool whose code we mirror.
    let tool_exit: Option<i32> = if let Some(from) = &cli.from {
        let profile = resolve_or_bail(from)?;
        runner::stream_stdin(profile, |diagnostic| renderer.diagnostic(diagnostic))?;
        None
    } else {
        let Some((program, args)) = cli.command.split_first() else {
            bail!(
                "no command given. Use `simp <tool> <args>` or `<tool> | simp --from <tool>`.\nKnown tools: {}",
                profiles::known_names().join(", ")
            );
        };
        let profile = resolve_or_bail(program)?;
        let exit = runner::stream_command(program, args, profile, |diagnostic| {
            renderer.diagnostic(diagnostic)
        })?;
        Some(exit)
    };

    let errors = renderer.error_count();
    renderer.finish(tool_exit.unwrap_or(0))?;

    Ok(match tool_exit {
        // Wrapper mode mirrors the tool's exit code so simp is transparent in CI.
        Some(code) => ExitCode::from(code.clamp(0, 255) as u8),
        // Stdin mode has no tool exit, so fail iff we parsed any errors.
        None if errors > 0 => ExitCode::from(1),
        None => ExitCode::SUCCESS,
    })
}

fn resolve_or_bail(name: &str) -> Result<&'static Profile> {
    profiles::resolve(name).ok_or_else(|| {
        anyhow::anyhow!(
            "no profile for `{name}`. Known tools: {}",
            profiles::known_names().join(", ")
        )
    })
}
