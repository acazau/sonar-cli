use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

#[derive(clap::Args)]
pub struct ScopeArgs {
    /// List all tracked files instead of just changed ones
    #[arg(long)]
    pub full: bool,
    /// File extension(s) to filter (without dot, comma-separated e.g. "rs" or "rs,toml")
    #[arg(long, default_value = "rs")]
    pub ext: String,
    /// Output as JSON array
    #[arg(long)]
    pub json: bool,
}

pub fn git_run(args: &[&str], dir: Option<&Path>) -> Result<Vec<String>, ()> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(d) = dir {
        cmd.current_dir(d);
    }
    let output = cmd.output().map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .trim()
        .split('\n')
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect())
}

pub fn scope(args: &ScopeArgs) {
    let extensions: Vec<&str> = args.ext.split(',').map(|s| s.trim()).collect();
    let mut all_files = BTreeSet::new();

    for ext in &extensions {
        let pattern = format!("*.{ext}");
        let files = if args.full {
            get_all_files(&pattern, None)
        } else {
            get_changed_files(&pattern, None)
        };
        all_files.extend(files);
    }

    if args.json {
        let json_arr: Vec<String> = all_files.iter().map(|f| format!("\"{}\"", f)).collect();
        println!("[{}]", json_arr.join(","));
    } else {
        for f in &all_files {
            println!("{}", f);
        }
    }
}

pub fn get_changed_files(pattern: &str, dir: Option<&Path>) -> Vec<String> {
    // Committed changes since origin/main (or main as fallback)
    let committed =
        git_run(&["diff", "--name-only", "origin/main...HEAD", "--", pattern], dir)
            .or_else(|_| git_run(&["diff", "--name-only", "main...HEAD", "--", pattern], dir))
            .unwrap_or_default();

    // Unstaged working tree changes
    let unstaged = git_run(&["diff", "--name-only", "--", pattern], dir).unwrap_or_default();

    // Staged changes
    let staged =
        git_run(&["diff", "--name-only", "--cached", "--", pattern], dir).unwrap_or_default();

    // Untracked files
    let untracked = git_run(
        &["ls-files", "--others", "--exclude-standard", pattern],
        dir,
    )
    .unwrap_or_default();

    let mut set = BTreeSet::new();
    for f in committed
        .into_iter()
        .chain(unstaged)
        .chain(staged)
        .chain(untracked)
    {
        set.insert(f);
    }
    set.into_iter().collect()
}

pub fn get_all_files(pattern: &str, dir: Option<&Path>) -> Vec<String> {
    git_run(&["ls-files", pattern], dir).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    fn init_temp_git_repo(suffix: &str) -> PathBuf {
        let tmp = env::temp_dir().join(format!(
            "xtask-{}-{}-{:?}",
            suffix,
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let git = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(&tmp)
                .output()
                .expect("git command failed")
        };

        git(&["init"]);
        git(&["config", "user.email", "test@test.com"]);
        git(&["config", "user.name", "Test"]);

        fs::write(tmp.join("main.rs"), "fn main() {}").unwrap();
        fs::write(tmp.join("lib.rs"), "pub fn lib() {}").unwrap();
        git(&["add", "."]);
        git(&["commit", "-m", "initial"]);

        fs::write(
            tmp.join("main.rs"),
            "fn main() { println!(\"hi\"); }",
        )
        .unwrap();
        fs::write(tmp.join("new.rs"), "fn new() {}").unwrap();
        fs::write(tmp.join("notes.txt"), "not rust").unwrap();

        tmp
    }

    #[test]
    fn scope_full_returns_all_tracked() {
        let repo = init_temp_git_repo("full");
        let files = get_all_files("*.rs", Some(&repo));
        assert!(files.contains(&"lib.rs".to_string()));
        assert!(files.contains(&"main.rs".to_string()));
        assert!(!files.contains(&"new.rs".to_string()));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn scope_default_returns_changed_and_untracked() {
        let repo = init_temp_git_repo("changed");
        let files = get_changed_files("*.rs", Some(&repo));
        assert!(files.contains(&"main.rs".to_string()));
        assert!(files.contains(&"new.rs".to_string()));
        assert!(!files.contains(&"lib.rs".to_string()));
        assert!(!files.contains(&"notes.txt".to_string()));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn scope_ext_filtering() {
        let repo = init_temp_git_repo("ext");
        Command::new("git")
            .args(["add", "notes.txt"])
            .current_dir(&repo)
            .output()
            .unwrap();
        let files = get_all_files("*.txt", Some(&repo));
        assert!(files.contains(&"notes.txt".to_string()));
        assert!(!files.contains(&"main.rs".to_string()));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn scope_multi_ext_returns_all_matching() {
        let repo = init_temp_git_repo("multi-ext");
        // Add notes.txt so it's tracked
        Command::new("git")
            .args(["add", "notes.txt"])
            .current_dir(&repo)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "add txt"])
            .current_dir(&repo)
            .output()
            .unwrap();
        // Now get all files for both rs and txt
        let mut all = BTreeSet::new();
        for ext in ["rs", "txt"] {
            let pattern = format!("*.{ext}");
            all.extend(get_all_files(&pattern, Some(&repo)));
        }
        assert!(all.contains("lib.rs"));
        assert!(all.contains("main.rs"));
        assert!(all.contains("notes.txt"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn scope_staged_changes_detected() {
        let repo = init_temp_git_repo("staged");
        // Stage the modified main.rs
        Command::new("git")
            .args(["add", "main.rs"])
            .current_dir(&repo)
            .output()
            .unwrap();
        let files = get_changed_files("*.rs", Some(&repo));
        assert!(files.contains(&"main.rs".to_string()));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn scope_graceful_fallback_no_parent() {
        let tmp = env::temp_dir().join(format!(
            "xtask-fallback-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let git = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(&tmp)
                .output()
                .unwrap()
        };
        git(&["init"]);
        git(&["config", "user.email", "t@t.com"]);
        git(&["config", "user.name", "T"]);
        fs::write(tmp.join("a.rs"), "").unwrap();
        git(&["add", "."]);
        git(&["commit", "-m", "only commit"]);

        let files = get_changed_files("*.rs", Some(&tmp));
        assert!(files.len() <= 1);
        let _ = fs::remove_dir_all(&tmp);
    }
}
