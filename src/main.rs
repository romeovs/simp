mod agent;
mod diagnostic;
mod format;
mod profiles;
mod runner;

use std::process::{Command, ExitCode};

use anyhow::{Context, Result};
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

    /// Whether to normalize output. `auto` passes the tool through untouched
    /// unless simp detects it's running inside an AI agent.
    #[arg(long, value_enum, default_value_t = Enabled::Auto, env = "SIMP_ENABLED")]
    enabled: Enabled,

    /// The tool to wrap and its arguments, e.g. `simp tsc --noEmit`.
    #[arg(value_name = "COMMAND", num_args = 0..)]
    command: Vec<String>,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum FormatArg {
    Flat,
    Json,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum Enabled {
    True,
    False,
    Auto,
}

impl Enabled {
    /// Resolve the tri-state to a decision, detecting the agent for `auto`.
    fn resolve(self) -> bool {
        match self {
            Enabled::True => true,
            Enabled::False => false,
            Enabled::Auto => agent::detected(),
        }
    }
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
    if !cli.enabled.resolve() {
        return passthrough(&cli);
    }

    let format: Format = cli.format.into();
    let stdout = std::io::stdout();
    let mut renderer = Renderer::new(stdout.lock(), format);

    let (program, args) = command_or_bail(&cli)?;
    let profile = resolve_or_bail(program)?;
    let tool_exit =
        runner::stream_command(program, args, profile, |diagnostic| renderer.diagnostic(diagnostic))?;

    renderer.finish(tool_exit)?;

    // Mirror the tool's exit code so simp is transparent in CI.
    Ok(ExitCode::from(tool_exit.clamp(0, 255) as u8))
}

/// Run the tool transparently: original args, inherited stdio (so colors and
/// TTY detection survive), mirrored exit code, no parsing.
fn passthrough(cli: &Cli) -> Result<ExitCode> {
    let (program, args) = command_or_bail(cli)?;
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("failed to spawn `{program}`"))?;
    Ok(ExitCode::from(status.code().unwrap_or(1).clamp(0, 255) as u8))
}

fn command_or_bail(cli: &Cli) -> Result<(&String, &[String])> {
    cli.command.split_first().ok_or_else(|| {
        anyhow::anyhow!(
            "no command given. Use `simp <tool> <args>`.\nKnown tools: {}",
            profiles::known_names().join(", ")
        )
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
