use super::list_different::ListDifferent;
use super::{has_flag, Injection, Profile, StreamParser};

pub const PROFILE: Profile = Profile {
    name: "oxfmt",
    inject,
    parser,
};

// oxfmt writes files in place by default, so injecting `--list-different` is
// also what keeps simp from mutating the working tree — it prints unformatted
// paths (one per line, exit 1) instead. Refuse `--check` (human output).
fn inject(args: &[String]) -> Injection {
    if has_flag(args, &["--check"]) {
        return Injection::Unsupported(
            "oxfmt `--check` output isn't parseable; simp needs `--list-different`".to_string(),
        );
    }
    if has_flag(args, &["--list-different"]) {
        return Injection::Append(Vec::new());
    }
    Injection::Append(vec!["--list-different".to_string()])
}

fn parser() -> Box<dyn StreamParser> {
    Box::new(ListDifferent { source: "oxfmt" })
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
        assert_eq!(
            inject(&args(&["--list-different", "src/"])),
            Injection::Append(vec![])
        );
    }

    #[test]
    fn rejects_check() {
        assert!(matches!(
            inject(&args(&["--check", "src/"])),
            Injection::Unsupported(_)
        ));
    }
}
