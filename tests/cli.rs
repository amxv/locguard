use std::fs;
use std::path::Path;
use std::process::Command as ProcessCommand;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_locguard"))
}

fn git(root: &Path, args: &[&str]) {
    let status = ProcessCommand::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .expect("git should run");
    assert!(status.success(), "git {args:?} failed");
}

fn repo() -> TempDir {
    let temp = tempfile::tempdir().unwrap();
    git(temp.path(), &["init", "-q"]);
    git(
        temp.path(),
        &["config", "user.email", "locguard@example.test"],
    );
    git(temp.path(), &["config", "user.name", "locguard tests"]);
    temp
}

fn write_lines(path: &Path, lines: usize) {
    let mut body = String::new();
    for index in 0..lines {
        body.push_str(&format!("line {index}\n"));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, body).unwrap();
}

fn write_unterminated_lines(path: &Path, lines: usize) {
    let body = (0..lines)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, body).unwrap();
}

fn commit_all(root: &Path) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "fixture"]);
}

fn write_config(root: &Path, source: &str) {
    let path = root.join(".agents/.locguard.toml");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, source).unwrap();
}

#[test]
fn help_explains_zero_config_and_core_surface() {
    cli()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "locguard requires no configuration",
        ))
        .stdout(predicate::str::contains("--file <PATH>"))
        .stdout(predicate::str::contains("--dir <PATH>"))
        .stdout(predicate::str::contains("scan"))
        .stdout(predicate::str::contains("init"));
}

#[test]
fn shows_version() {
    cli()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn bare_git_check_reports_no_changed_source_files() {
    let repo = repo();
    write_lines(&repo.path().join("src/main.rs"), 10);
    commit_all(repo.path());

    cli()
        .current_dir(repo.path())
        .args(["--color", "never"])
        .assert()
        .success()
        .stdout("✓ no source files changed\n");
}

#[test]
fn bare_git_check_catches_new_oversized_source_file() {
    let repo = repo();
    write_lines(&repo.path().join("src/main.rs"), 10);
    commit_all(repo.path());
    write_lines(&repo.path().join("src/new.rs"), 1001);

    cli()
        .current_dir(repo.path())
        .args(["--color", "never"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL src/new.rs  >1000"))
        .stdout(predicate::str::contains(
            "1 file exceeds the 1000-line limit",
        ));
}

#[test]
fn full_scan_catches_committed_oversized_files() {
    let repo = repo();
    write_lines(&repo.path().join("src/legacy.rs"), 1001);
    commit_all(repo.path());

    cli()
        .current_dir(repo.path())
        .args(["scan", "--color", "never"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("src/legacy.rs"));
}

#[test]
fn vendor_and_generated_trees_are_skipped_by_default() {
    let repo = repo();
    write_lines(&repo.path().join("vendor/dependency.rs"), 1200);
    write_lines(&repo.path().join("generated/client.ts"), 1200);
    write_lines(&repo.path().join("src/main.rs"), 10);
    commit_all(repo.path());

    cli()
        .current_dir(repo.path())
        .args(["scan", "--color", "never"])
        .assert()
        .success()
        .stdout("✓ 1 file checked\n");
}

#[test]
fn explicit_dir_resurrects_its_builtin_excluded_root() {
    let repo = repo();
    write_lines(&repo.path().join("vendor/dependency.rs"), 1200);
    commit_all(repo.path());

    cli()
        .current_dir(repo.path())
        .args(["--dir", "vendor", "--color", "never"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("vendor/dependency.rs"));
}

#[test]
fn explicit_file_bypasses_default_source_type_detection() {
    let repo = repo();
    write_lines(&repo.path().join("Makefile"), 1001);
    commit_all(repo.path());

    cli()
        .current_dir(repo.path())
        .args(["--file", "Makefile", "--color", "never"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("FAIL Makefile  >1000"));
}

#[test]
fn config_include_adds_special_files_and_exclude_wins() {
    let repo = repo();
    write_lines(&repo.path().join("Makefile"), 1001);
    write_config(
        repo.path(),
        r#"
include = ["Makefile"]
exclude = ["Makefile"]
"#,
    );
    commit_all(repo.path());

    cli()
        .current_dir(repo.path())
        .args(["scan", "--color", "never"])
        .assert()
        .success()
        .stdout("✓ 0 files checked\n");
}

#[test]
fn exact_exemptions_are_silent_until_no_exempt_is_used() {
    let repo = repo();
    write_lines(&repo.path().join("src/legacy.rs"), 1200);
    write_config(
        repo.path(),
        r#"
[exempt]
files = ["src/legacy.rs"]
"#,
    );
    commit_all(repo.path());

    cli()
        .current_dir(repo.path())
        .args(["scan", "--color", "never"])
        .assert()
        .success()
        .stdout("✓ 0 files checked\n");

    cli()
        .current_dir(repo.path())
        .args(["scan", "--no-exempt", "--color", "never"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("src/legacy.rs"));
}

#[test]
fn per_path_override_applies_and_cli_limit_wins_over_it() {
    let repo = repo();
    write_lines(&repo.path().join("migrations/001.sql"), 1200);
    write_config(
        repo.path(),
        r#"
[[override]]
files = ["migrations/**/*.sql"]
limit = 1500
"#,
    );
    commit_all(repo.path());

    cli()
        .current_dir(repo.path())
        .args(["scan", "--color", "never"])
        .assert()
        .success();

    cli()
        .current_dir(repo.path())
        .args(["scan", "--limit", "1000", "--color", "never"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(">1000"));
}

#[test]
fn warnings_use_relative_threshold_and_can_be_suppressed() {
    let repo = repo();
    write_lines(&repo.path().join("src/near.rs"), 900);
    commit_all(repo.path());

    cli()
        .current_dir(repo.path())
        .args(["scan", "--color", "never"])
        .assert()
        .success()
        .stdout(predicate::str::contains("WARN src/near.rs  900 / 1000"))
        .stdout(predicate::str::contains("1 warning"));

    cli()
        .current_dir(repo.path())
        .args(["scan", "--no-warn", "--color", "never"])
        .assert()
        .success()
        .stdout("✓ 1 file checked\n");
}

#[test]
fn physical_lines_count_unterminated_final_line() {
    let repo = repo();
    write_unterminated_lines(&repo.path().join("src/exact.rs"), 1001);
    commit_all(repo.path());

    cli()
        .current_dir(repo.path())
        .args(["scan", "--color", "never"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(">1000"));
}

#[test]
fn gitignored_files_are_skipped_unless_no_ignore_is_set() {
    let repo = repo();
    fs::write(repo.path().join(".gitignore"), "ignored/\n").unwrap();
    write_lines(&repo.path().join("ignored/large.rs"), 1100);
    write_lines(&repo.path().join("src/main.rs"), 10);
    commit_all(repo.path());

    cli()
        .current_dir(repo.path())
        .args(["scan", "--color", "never"])
        .assert()
        .success();

    cli()
        .current_dir(repo.path())
        .args(["scan", "--no-ignore", "--color", "never"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("ignored/large.rs"));
}

#[test]
fn default_json_does_not_fake_exact_violation_count() {
    let repo = repo();
    write_lines(&repo.path().join("src/large.rs"), 1200);
    commit_all(repo.path());

    let output = cli()
        .current_dir(repo.path())
        .args(["scan", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["violations"][0]["lines"], Value::Null);
    assert_eq!(json["violations"][0]["greater_than"], 1000);
}

#[test]
fn exact_mode_reports_exact_violation_count() {
    let repo = repo();
    write_lines(&repo.path().join("src/large.rs"), 1200);
    commit_all(repo.path());

    let output = cli()
        .current_dir(repo.path())
        .args(["scan", "--exact", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["violations"][0]["lines"], 1200);
}

#[test]
fn binary_like_source_bytes_are_skipped() {
    let repo = repo();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(repo.path().join("src/utf16le.css"), b"a\0\n\0b\0\n\0").unwrap();
    commit_all(repo.path());

    cli()
        .current_dir(repo.path())
        .args(["scan", "--limit", "2", "--color", "never"])
        .assert()
        .success()
        .stdout("✓ 0 files checked\n");
}

#[cfg(unix)]
#[test]
fn small_unreadable_source_is_still_a_tool_error() {
    use std::os::unix::fs::PermissionsExt;

    let repo = repo();
    let path = repo.path().join("src/small.rs");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"fn x() {}\n").unwrap();
    commit_all(repo.path());

    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o000);
    fs::set_permissions(&path, permissions).unwrap();

    cli()
        .current_dir(repo.path())
        .args(["scan"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("failed to open source file"));
}

#[test]
fn unknown_config_key_is_a_tool_error() {
    let repo = repo();
    write_lines(&repo.path().join("src/main.rs"), 10);
    write_config(repo.path(), "warn_precent = 80\n");
    commit_all(repo.path());

    cli()
        .current_dir(repo.path())
        .args(["scan"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("unknown field"));
}

#[test]
fn init_is_optional_and_never_overwrites_existing_config() {
    let repo = repo();

    cli()
        .current_dir(repo.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains(".agents/.locguard.toml"));

    let config = fs::read_to_string(repo.path().join(".agents/.locguard.toml")).unwrap();
    assert!(config.contains("[exempt]"));
    assert!(config.contains("files = []"));

    cli()
        .current_dir(repo.path())
        .arg("init")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("config already exists"));
}

#[test]
fn non_git_bare_invocation_is_a_full_tree_scan() {
    let tree = tempfile::tempdir().unwrap();
    write_lines(&tree.path().join("src/large.rs"), 1001);

    cli()
        .current_dir(tree.path())
        .args(["--color", "never"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("src/large.rs"));
}

#[test]
fn generated_filename_can_be_resurrected_with_include() {
    let repo = repo();
    write_lines(&repo.path().join("src/api.generated.ts"), 1100);
    commit_all(repo.path());

    cli()
        .current_dir(repo.path())
        .args(["scan", "--color", "never"])
        .assert()
        .success()
        .stdout("✓ 0 files checked\n");

    cli()
        .current_dir(repo.path())
        .args(["scan", "--include", "**/*.generated.ts", "--color", "never"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("src/api.generated.ts"));
}

#[test]
fn only_replaces_builtin_source_recognition() {
    let repo = repo();
    write_lines(&repo.path().join("src/large.rs"), 1100);
    write_lines(&repo.path().join("src/large.py"), 1100);
    commit_all(repo.path());

    cli()
        .current_dir(repo.path())
        .args(["scan", "--only", "**/*.py", "--color", "never"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("src/large.py"))
        .stdout(predicate::str::contains("src/large.rs").not());
}

#[test]
fn no_default_excludes_scans_vendor_code() {
    let repo = repo();
    write_lines(&repo.path().join("vendor/dependency.rs"), 1100);
    commit_all(repo.path());

    cli()
        .current_dir(repo.path())
        .args(["scan", "--no-default-excludes", "--color", "never"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("vendor/dependency.rs"));
}

#[test]
fn bare_no_ignore_includes_ignored_untracked_source() {
    let repo = repo();
    fs::write(repo.path().join(".gitignore"), "ignored/\n").unwrap();
    write_lines(&repo.path().join("src/main.rs"), 10);
    commit_all(repo.path());
    write_lines(&repo.path().join("ignored/new.rs"), 1100);

    cli()
        .current_dir(repo.path())
        .args(["--color", "never"])
        .assert()
        .success()
        .stdout("✓ no source files changed\n");

    cli()
        .current_dir(repo.path())
        .args(["--no-ignore", "--color", "never"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("ignored/new.rs"));
}

#[test]
fn explicit_ignored_file_requires_no_ignore() {
    let repo = repo();
    fs::write(repo.path().join(".gitignore"), "ignored.rs\n").unwrap();
    commit_all(repo.path());
    write_lines(&repo.path().join("ignored.rs"), 1100);

    cli()
        .current_dir(repo.path())
        .args(["--file", "ignored.rs", "--color", "never"])
        .assert()
        .success()
        .stdout("✓ 0 files checked\n");

    cli()
        .current_dir(repo.path())
        .args(["--file", "ignored.rs", "--no-ignore", "--color", "never"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains("ignored.rs"));
}
