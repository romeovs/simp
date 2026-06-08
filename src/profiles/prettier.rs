use super::list_different::ListDifferent;
use super::{has_flag, Injection, Profile, StreamParser};

pub const PROFILE: Profile = Profile {
    name: "prettier",
    inject,
    parser,
};

// `--list-different` prints the paths of unformatted files, one per line, which
// is exactly what we want. Refuse `--write` (it would modify files) and
// `--check` (human-formatted output we can't parse).
fn inject(args: &[String]) -> Injection {
    if has_flag(args, &["-w", "--write"]) {
        return Injection::Unsupported(
            "prettier `--write` would modify files; simp needs `--list-different`".to_string(),
        );
    }
    if has_flag(args, &["-c", "--check"]) {
        return Injection::Unsupported(
            "prettier `--check` output isn't parseable; simp needs `--list-different`".to_string(),
        );
    }
    if has_flag(args, &["-l", "--list-different"]) {
        return Injection::Append(Vec::new());
    }
    Injection::Append(vec!["--list-different".to_string()])
}

fn parser() -> Box<dyn StreamParser> {
    Box::new(ListDifferent { source: "prettier" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn injects_list_different_by_default() {
        assert_eq!(
            inject(&args(&["src/"])),
            Injection::Append(args(&["--list-different"]))
        );
    }

    #[test]
    fn respects_existing_list_different() {
        assert_eq!(inject(&args(&["-l", "src/"])), Injection::Append(vec![]));
    }

    #[test]
    fn rejects_write_and_check() {
        assert!(matches!(
            inject(&args(&["--write", "src/"])),
            Injection::Unsupported(_)
        ));
        assert!(matches!(
            inject(&args(&["--check", "src/"])),
            Injection::Unsupported(_)
        ));
    }
}
