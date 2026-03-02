use super::helpers::replace_prefix;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    /// head -N file pattern
    static ref HEAD_DASH_N_RE: Regex =
        Regex::new(r"^head\s+-(\d+)\s+(.+)$").unwrap();

    /// head --lines=N file pattern
    static ref HEAD_LINES_RE: Regex =
        Regex::new(r"^head\s+--lines=(\d+)\s+(.+)$").unwrap();
}

pub fn try_rewrite_head(match_cmd: &str) -> Option<String> {
    // head -N file → rtk read file --max-lines N
    if let Some(caps) = HEAD_DASH_N_RE.captures(match_cmd) {
        let lines = &caps[1];
        let file = &caps[2];
        return Some(format!("rtk read {} --max-lines {}", file, lines));
    }
    // head --lines=N file
    if let Some(caps) = HEAD_LINES_RE.captures(match_cmd) {
        let lines = &caps[1];
        let file = &caps[2];
        return Some(format!("rtk read {} --max-lines {}", file, lines));
    }
    None
}

/// Grep/rg flags that take a value argument (next token is the value, not a positional).
const GREP_FLAGS_WITH_VALUE: &[&str] = &[
    "-A",
    "-B",
    "-C",
    "-e",
    "-f",
    "-m",
    "--max-count",
    "--include",
    "--exclude",
    "--exclude-dir",
    "--glob",
    "--type",
    "--type-not",
    "--type-add",
    "-g",
    "-t",
    "-T",
];

/// Reorder grep/rg arguments so positionals come before flags.
/// RTK's clap expects: `rtk grep PATTERN [PATH] [-- extra_flags...]`
/// but real usage is `grep -r -i PATTERN .` (flags first).
fn reorder_grep_args(cmd: &str) -> String {
    let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
    if parts.len() < 2 {
        return "rtk grep".to_string();
    }

    let tokens: Vec<&str> = parts[1].split_whitespace().collect();
    let mut flags: Vec<&str> = Vec::new();
    let mut positionals: Vec<&str> = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        let tok = tokens[i];
        if tok == "--" {
            positionals.extend_from_slice(&tokens[i + 1..]);
            break;
        } else if tok.starts_with('-') {
            flags.push(tok);
            // Consume next token if this flag takes a value
            if GREP_FLAGS_WITH_VALUE.contains(&tok) {
                i += 1;
                if i < tokens.len() {
                    flags.push(tokens[i]);
                }
            }
        } else {
            positionals.push(tok);
        }
        i += 1;
    }

    let mut result = String::from("rtk grep");
    for p in &positionals {
        result.push(' ');
        result.push_str(p);
    }
    if !flags.is_empty() {
        result.push_str(" --");
        for f in &flags {
            result.push(' ');
            result.push_str(f);
        }
    }
    result
}

/// Try to rewrite file operation commands (cat, grep, ls, tree, find, diff, curl, wget).
/// Returns Some(rewritten) if matched, None otherwise.
pub fn try_rewrite_file_cmd(match_cmd: &str, cmd_body: &str) -> Option<String> {
    if match_cmd.starts_with("cat ") {
        // Count non-flag arguments: skip tokens starting with '-'
        let args: Vec<&str> = match_cmd
            .strip_prefix("cat ")
            .unwrap_or("")
            .split_whitespace()
            .filter(|a| !a.starts_with('-'))
            .collect();
        // rtk read only accepts a single file — skip multi-file cat and stdin (cat -)
        if args.len() != 1 || args[0] == "-" {
            return None;
        }
        return Some(replace_prefix(cmd_body, "cat ", "rtk read "));
    }
    if match_cmd.starts_with("rg ") || match_cmd.starts_with("grep ") {
        return Some(reorder_grep_args(match_cmd));
    }
    if match_cmd == "ls" || match_cmd.starts_with("ls ") {
        return Some(replace_prefix(cmd_body, "ls", "rtk ls"));
    }
    if match_cmd == "tree" || match_cmd.starts_with("tree ") {
        return Some(replace_prefix(cmd_body, "tree", "rtk tree"));
    }
    if match_cmd.starts_with("find ") {
        return Some(replace_prefix(cmd_body, "find ", "rtk find "));
    }
    if match_cmd.starts_with("diff ") {
        return Some(replace_prefix(cmd_body, "diff ", "rtk diff "));
    }
    if match_cmd.starts_with("head ") {
        return try_rewrite_head(match_cmd);
    }
    // Network commands
    if match_cmd.starts_with("curl ") {
        return Some(replace_prefix(cmd_body, "curl ", "rtk curl "));
    }
    if match_cmd.starts_with("wget ") {
        return Some(replace_prefix(cmd_body, "wget ", "rtk wget "));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::super::rewrite_chain;

    fn rewrite(cmd: &str) -> Option<String> {
        if cmd.contains("<<") {
            return None;
        }
        rewrite_chain(cmd)
    }

    #[test]
    fn test_cat_to_read() {
        assert_eq!(
            rewrite("cat src/main.rs"),
            Some("rtk read src/main.rs".into())
        );
    }

    #[test]
    fn test_rg_to_grep() {
        assert_eq!(
            rewrite("rg pattern src/"),
            Some("rtk grep pattern src/".into())
        );
    }

    #[test]
    fn test_grep_no_flags() {
        assert_eq!(rewrite("grep TODO ."), Some("rtk grep TODO .".into()));
    }

    #[test]
    fn test_ls() {
        assert_eq!(rewrite("ls -la"), Some("rtk ls -la".into()));
    }

    #[test]
    fn test_ls_bare() {
        assert_eq!(rewrite("ls"), Some("rtk ls".into()));
    }

    #[test]
    fn test_find() {
        assert_eq!(
            rewrite("find . -name '*.rs'"),
            Some("rtk find . -name '*.rs'".into())
        );
    }

    #[test]
    fn test_head_dash_n() {
        assert_eq!(
            rewrite("head -20 src/main.rs"),
            Some("rtk read src/main.rs --max-lines 20".into())
        );
    }

    #[test]
    fn test_head_lines_eq() {
        assert_eq!(
            rewrite("head --lines=50 README.md"),
            Some("rtk read README.md --max-lines 50".into())
        );
    }

    #[test]
    fn test_curl() {
        assert_eq!(
            rewrite("curl https://api.example.com"),
            Some("rtk curl https://api.example.com".into())
        );
    }

    #[test]
    fn test_wget() {
        assert_eq!(
            rewrite("wget https://example.com/file"),
            Some("rtk wget https://example.com/file".into())
        );
    }

    // --- P0-3: cat with multiple files should NOT be rewritten ---
    #[test]
    fn test_cat_multi_file_no_rewrite() {
        assert_eq!(rewrite("cat file1.txt file2.txt"), None);
    }

    #[test]
    fn test_cat_three_files_no_rewrite() {
        assert_eq!(rewrite("cat a.txt b.txt c.txt"), None);
    }

    #[test]
    fn test_cat_single_file_still_works() {
        assert_eq!(
            rewrite("cat src/main.rs"),
            Some("rtk read src/main.rs".into())
        );
    }

    #[test]
    fn test_cat_with_flags_single_file() {
        // cat -n file → has flag, still single file target
        assert_eq!(
            rewrite("cat -n src/main.rs"),
            Some("rtk read -n src/main.rs".into())
        );
    }

    #[test]
    fn test_cat_stdin_no_rewrite() {
        // cat - reads from stdin, rtk read doesn't support it
        assert_eq!(rewrite("cat -"), None);
    }

    // --- P1-2: grep/rg flags reordered for clap compatibility ---
    // rtk grep expects: PATTERN [PATH] [-- extra_flags...]
    // Real usage: grep -r TODO . (flags first) → reorder to: rtk grep TODO . -- -r
    #[test]
    fn test_grep_flags_reordered() {
        assert_eq!(
            rewrite("grep -r TODO ."),
            Some("rtk grep TODO . -- -r".into())
        );
    }

    #[test]
    fn test_grep_multiple_flags_reordered() {
        assert_eq!(
            rewrite("grep -r -i pattern src/"),
            Some("rtk grep pattern src/ -- -r -i".into())
        );
    }

    #[test]
    fn test_grep_combined_flags_reordered() {
        assert_eq!(
            rewrite("grep -rn pattern src/"),
            Some("rtk grep pattern src/ -- -rn".into())
        );
    }

    #[test]
    fn test_rg_flags_reordered() {
        assert_eq!(
            rewrite("rg -i pattern ."),
            Some("rtk grep pattern . -- -i".into())
        );
    }

    #[test]
    fn test_grep_flag_with_value() {
        // -A 3 consumes next token as value
        assert_eq!(
            rewrite("grep -A 3 pattern ."),
            Some("rtk grep pattern . -- -A 3".into())
        );
    }

    #[test]
    fn test_grep_no_flags_no_separator() {
        assert_eq!(rewrite("grep TODO src/"), Some("rtk grep TODO src/".into()));
    }

    #[test]
    fn test_rg_no_flags_no_separator() {
        assert_eq!(
            rewrite("rg pattern src/"),
            Some("rtk grep pattern src/".into())
        );
    }
}
