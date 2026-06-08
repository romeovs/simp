mod biome;
mod eslint;
mod list_different;
mod oxfmt;
mod prettier;
mod tsc;

use crate::diagnostic::Diagnostic;

/// Incrementally turns a tool's output into diagnostics, one line at a time, so
/// callers can render results before the tool finishes. Line-oriented tools
/// (tsc) emit from `push_line`; whole-document tools (eslint JSON) buffer in
/// `push_line` and emit everything from `finish`.
pub trait StreamParser {
    /// Consume one line of output (the trailing newline already stripped).
    /// Returns any diagnostics that are complete as of this line.
    fn push_line(&mut self, line: &str) -> Vec<Diagnostic>;

    /// Input is exhausted; return any diagnostics still buffered. Line parsers
    /// have nothing left, so the default is empty.
    fn finish(&mut self) -> Vec<Diagnostic> {
        Vec::new()
    }
}

/// What to do about machine-readable flags, given the args the user already
/// passed. Profiles decide this per-invocation rather than declaring a fixed
/// flag list, because the user may have set a conflicting (or already-correct)
/// output mode themselves.
#[derive(Debug, PartialEq, Eq)]
pub enum Injection {
    /// Append these flags to the wrapped command. Empty means the args already
    /// request parseable output, so nothing needs adding.
    Append(Vec<String>),
    /// The args force an output mode simp can't parse (e.g. `--pretty true`).
    /// The string explains why, for a warning.
    Unsupported(String),
}

/// Everything simp knows about one supported tool, in one place: how to coax
/// machine-readable output out of it, and how to parse that output. Each tool's
/// module owns a `pub const PROFILE: Profile`; add a tool by adding a module and
/// listing it in `PROFILES`.
pub struct Profile {
    /// Matched against the wrapped command's program name (e.g. the `tsc` in
    /// `simp tsc --noEmit`).
    pub name: &'static str,
    /// Decide which flags (if any) to inject for parseable output, given the
    /// args the user already passed. Only consulted in wrapper mode.
    pub inject: fn(args: &[String]) -> Injection,
    /// Construct a fresh streaming parser for one invocation.
    pub parser: fn() -> Box<dyn StreamParser>,
}

static PROFILES: &[Profile] = &[
    tsc::PROFILE,
    eslint::PROFILE,
    biome::PROFILE,
    prettier::PROFILE,
    oxfmt::PROFILE,
];

/// Resolve a profile by the wrapped command's program name (e.g. the `tsc` in
/// `simp tsc --noEmit`).
pub fn resolve(name: &str) -> Option<&'static Profile> {
    let stem = program_stem(name);
    PROFILES.iter().find(|profile| profile.name == stem)
}

/// Strip directory and wrapper prefixes so `./node_modules/.bin/tsc` resolves.
fn program_stem(name: &str) -> &str {
    name.rsplit(['/', '\\']).next().unwrap_or(name)
}

pub fn known_names() -> Vec<&'static str> {
    PROFILES.iter().map(|profile| profile.name).collect()
}

/// Read the value of a flag from args, handling both `--flag value` and
/// `--flag=value` forms. Returns the first match across the given aliases.
fn flag_value<'a>(args: &'a [String], names: &[&str]) -> Option<&'a str> {
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        for name in names {
            if arg == name {
                return iter.peek().map(|value| value.as_str());
            }
            if let Some(value) = arg.strip_prefix(&format!("{name}=")) {
                return Some(value);
            }
        }
    }
    None
}

/// Whether any of the given flag aliases is present, as a bare flag or with an
/// `=value`. For boolean/switch flags where only presence matters.
fn has_flag(args: &[String], names: &[&str]) -> bool {
    args.iter().any(|arg| {
        names
            .iter()
            .any(|name| arg == name || arg.starts_with(&format!("{name}=")))
    })
}
