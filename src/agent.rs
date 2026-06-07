use std::env;
use std::ffi::OsString;
use std::path::Path;

/// Environment markers set by known AI coding agents in the shell commands they
/// run. We only check presence (non-empty), never the value, so the shared
/// `AGENT` convention covers every tool that adopts it regardless of the value
/// it picks.
///
/// Detection is deliberately an allowlist: a false negative (an unrecognized
/// agent) is recoverable with `--enabled=true` or `SIMP_ENABLED=true`, whereas a
/// false positive would mangle native output for a human — or a CI pipeline that
/// merely pipes stdout.
const AGENT_MARKERS: &[&str] = &[
    "CLAUDECODE",       // Claude Code
    "CURSOR_AGENT",     // Cursor
    "GEMINI_CLI",       // Gemini CLI
    "CODEX_SANDBOX",    // OpenAI Codex CLI (set on sandboxed tool calls)
    "AUGMENT_AGENT",    // Augment
    "CLINE_ACTIVE",     // Cline
    "OPENCODE_CLIENT",  // OpenCode
    "TRAE_AI_SHELL_ID", // Trae AI
    "AGENT",            // Goose, Amp, and the cross-tool convention
    "AI_AGENT",         // generic opt-in / Vercel's detection standard
];

/// Filesystem markers for agents that leave a path rather than an env var.
const AGENT_PATHS: &[&str] = &[
    "/opt/.devin", // Devin
];

/// Whether simp appears to be running inside an AI agent.
pub fn detected() -> bool {
    any_env_set(AGENT_MARKERS, |marker: &str| env::var_os(marker))
        || AGENT_PATHS.iter().any(|path| Path::new(path).exists())
}

/// Env detection split from the environment so it can be tested without
/// mutating process-global state.
fn any_env_set(markers: &[&str], lookup: impl Fn(&str) -> Option<OsString>) -> bool {
    markers
        .iter()
        .any(|marker| lookup(marker).is_some_and(|value| !value.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup<'a>(set: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            set.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        }
    }

    #[test]
    fn detects_known_marker() {
        assert!(any_env_set(AGENT_MARKERS, lookup(&[("CURSOR_AGENT", "1")])));
        assert!(any_env_set(AGENT_MARKERS, lookup(&[("AGENT", "goose")])));
        assert!(any_env_set(AGENT_MARKERS, lookup(&[("CODEX_SANDBOX", "seatbelt")])));
    }

    #[test]
    fn ignores_empty_marker() {
        assert!(!any_env_set(AGENT_MARKERS, lookup(&[("CLAUDECODE", "")])));
    }

    #[test]
    fn absent_when_unrelated_vars_set() {
        assert!(!any_env_set(AGENT_MARKERS, lookup(&[("PATH", "/usr/bin")])));
    }
}
