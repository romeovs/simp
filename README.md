# simp

Normalize the output of common diagnostic tools (tsc, eslint, …) into one
consistent, token-efficient format that's easy for AI agents — and humans — to
consume.

## Install

Prebuilt binaries are attached to every [release](../../releases). The easiest
way to install and manage them is with [mise](https://mise.jdx.dev) via
[ubi](https://github.com/houseabsolute/ubi), which downloads the right binary
for your platform — no compile, no toolchain:

```sh
mise use ubi:romeovs/simp          # add to the current project
mise use -g ubi:romeovs/simp       # or install globally
```

ubi alone (without mise) works too:

```sh
ubi --project romeovs/simp --in ~/.local/bin
```

Or build from source (requires a Rust toolchain):

```sh
mise use cargo:simp                # compiles via cargo
# or
cargo install --git https://github.com/romeovs/simp simp
```

Releases ship static musl Linux binaries (x86_64, aarch64), macOS (Intel and
Apple Silicon), and Windows (x86_64).

## Usage

simp wraps the diagnostic tool: it injects the right machine-readable flags,
runs the tool, and captures its real exit code.

```sh
simp tsc --noEmit
simp eslint ./src
```

### When simp is active

simp only normalizes output when it's actually useful — inside an AI agent.
Otherwise it's a transparent pass-through, so a human running `simp tsc` in a
terminal still gets tsc's native, colored output.

Control this with `--enabled`:

| Value  | Behavior |
| ------ | -------- |
| `auto` | **Default.** Normalize if an AI agent is detected, else pass through. |
| `true` | Always normalize. |
| `false`| Always pass through (run the tool untouched, mirror its exit code). |

The flag is also backed by the `SIMP_ENABLED` env var (the flag wins if both are
set), which is handy when you can't change how an agent invokes the command but
can set its environment.

**Pass-through** is exact: simp runs the tool with your original args (no flag
injection), inherits stdout/stderr directly (colors and TTY detection survive),
and mirrors the exit code.

**Agent detection** (`auto`) is an allowlist of the environment markers known
agents set in the commands they run:

| Agent | Marker |
| ----- | ------ |
| Claude Code | `CLAUDECODE` |
| Cursor | `CURSOR_AGENT` |
| Gemini CLI | `GEMINI_CLI` |
| OpenAI Codex CLI | `CODEX_SANDBOX` |
| Augment | `AUGMENT_AGENT` |
| Cline | `CLINE_ACTIVE` |
| OpenCode | `OPENCODE_CLIENT` |
| Trae AI | `TRAE_AI_SHELL_ID` |
| Goose, Amp, … | `AGENT` (cross-tool convention) |
| Devin | `/opt/.devin` (filesystem) |
| any (generic opt-in) | `AI_AGENT` |

Detection is intentionally conservative: an unrecognized agent is recoverable
with `--enabled=true` (or `SIMP_ENABLED=true`), whereas wrongly normalizing a
human's or CI's output is not. To add an agent, extend the lists in
[`src/agent.rs`](src/agent.rs).

### Output

Default `flat` format — one self-contained diagnostic per line, plus a summary:

```
error src/api.ts:12:5 TS2304 Cannot find name 'foo'
error src/db.ts:8:3 TS2345 Argument type mismatch

2 errors, 2 files
```

`--format json` emits the normalized `Report` for downstream tooling.

### Streaming

In `flat` mode simp renders each diagnostic the moment it's parsed, rather than
waiting for the tool to finish — lower latency to first output and no need to
hold all diagnostics in memory. How much this helps depends on the tool's
format: line-oriented output (tsc) streams diagnostic-by-diagnostic, while
whole-document formats (eslint's JSON array) can only be parsed once complete,
so those are accumulated and flushed at the end. `--format json` is always
buffered, since a single JSON document can't be emitted incrementally.

### Exit codes

- simp mirrors the wrapped tool's exit code (transparent in CI).
- simp's own failures (bad usage, spawn error) exit `2`.

## Architecture

```
argv ─▶ Profile ─▶ Runner ─▶ Parser ─▶ Report ─▶ Formatter ─▶ stdout
        (flag?    (spawn +   (raw →               (Report →
         parser?)  capture)   Diagnostic)          text/json)
```

Everything funnels through one normalized `Diagnostic`
(`src/diagnostic.rs`); parsers map into it, formatters render out of it.

## Supported tools

| Tool     | Injected flags        | Parser                       |
| -------- | --------------------- | ---------------------------- |
| tsc      | `--pretty false`      | text                         |
| eslint   | `--format json`       | JSON                         |
| biome    | `--reporter=github`   | GitHub annotations           |
| prettier | `--list-different`    | file list (formatting)       |
| oxfmt    | `--list-different`    | file list (formatting)       |

Notes:

- **biome** uses the `github` reporter, not `json`: it's the only one that
  carries per-diagnostic severity *and* line/column. Works with biome's
  diagnostic commands (`lint`, `check`, `ci`).
- **prettier** and **oxfmt** report formatting as file-level `not formatted`
  warnings. Injecting `--list-different` also keeps simp from mutating files
  (oxfmt writes in place by default).

### Roadmap

1. **Declarative plugins**: a TOML profile (`command`, `inject`, `parser =
   "json"`, field mappings via JSON pointers) so JSON-emitting tools need no
   code.
2. **Code plugins** only if the declarative format proves insufficient.

## Development

```sh
mise install      # Rust toolchain (pinned in .tool-versions)
cargo test
cargo build --release
```
