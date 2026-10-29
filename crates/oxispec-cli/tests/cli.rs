use std::fs;
use std::path::Path;
use std::process::Command;

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(dir: &Path, args: &[&str]) -> Out {
    let o = Command::new(env!("CARGO_BIN_EXE_oxispec"))
        .args(args)
        .current_dir(dir)
        // So that `oxispec tag` can create annotated tags without a global git identity.
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .output()
        .expect("binary runs");
    Out {
        code: o.status.code().expect("exit code"),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

#[test]
fn init_new_check_show_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();

    let out = run(p, &["init", "--preset", "standard"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(p.join(".oxispec/config.yaml").is_file());
    assert!(p.join("specs/adr").is_dir());

    let out = run(p, &["new", "requirement", "auth", "Admin session timeout"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("REQ-AUTH-0001"));
    let out = run(p, &["new", "requirement", "auth", "Password length"]);
    assert!(out.stdout.contains("REQ-AUTH-0002"));
    let out = run(p, &["new", "adr", "Use Postgres"]);
    assert!(out.stdout.contains("ADR-0001"));

    let out = run(p, &["check"]);
    assert_eq!(out.code, 0, "{}", out.stdout);
    assert!(out.stdout.contains("3 nodes checked: 0 errors"));

    // Link a requirement to the ADR, and show the reverse link.
    let file = p.join("specs/requirements/auth/admin-session-timeout.md");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        text.replace(
            "status: draft\n",
            "status: draft\nlinks:\n  decided-by: ADR-0001\n",
        ),
    )
    .unwrap();
    assert_eq!(run(p, &["check"]).code, 0);
    let out = run(p, &["show", "ADR-0001"]);
    assert!(
        out.stdout.contains("REQ-AUTH-0001 (decided-by)"),
        "{}",
        out.stdout
    );

    // A dangling link fails the check.
    let text = fs::read_to_string(&file).unwrap();
    fs::write(&file, text.replace("ADR-0001", "ADR-9999")).unwrap();
    let out = run(p, &["check"]);
    assert_eq!(out.code, 1);
    assert!(out.stdout.contains("E002"));
    let out = run(p, &["check", "--json"]);
    assert_eq!(out.code, 1);
    assert!(out.stdout.contains("\"errors\": 1"));
}

#[test]
fn modules_follow_dependency_rules() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init"]).code, 0);

    let out = run(p, &["modules", "add", "tests"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("enabled: tests"));

    let out = run(p, &["modules", "remove", "specs"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("needed by tests"), "{}", out.stderr);

    assert_eq!(run(p, &["modules", "remove", "tests"]).code, 0);
    assert_eq!(run(p, &["modules", "remove", "specs"]).code, 0);

    // `new requirement` needs the specs module.
    let out = run(p, &["new", "requirement", "auth", "Anything"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("module `specs` is not enabled"));
}

#[test]
fn init_rejects_a_preset_that_leaves_out_a_needed_module() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(
        dir.path(),
        &["init", "--preset", "specs-tests", "--without", "specs"],
    );
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("needs it"), "{}", out.stderr);
}

#[test]
fn commands_outside_a_project_explain_what_to_do() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(dir.path(), &["check"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("oxispec init"));
}

/// The ID in a line like `created REQ-ORD-7K3F (specs/...)`.
fn created_id(out: &Out) -> String {
    out.stdout
        .split_whitespace()
        .nth(1)
        .expect("an id after `created`")
        .to_string()
}

fn project_with_two_requirements_and_an_adr(p: &Path) {
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    assert_eq!(
        run(p, &["new", "requirement", "auth", "Admin session timeout"]).code,
        0
    );
    assert_eq!(
        run(p, &["new", "requirement", "auth", "Password length"]).code,
        0
    );
    assert_eq!(run(p, &["new", "adr", "Use Postgres"]).code, 0);
}

#[test]
fn link_command_adds_links_and_checks_the_target() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);

    let out = run(p, &["link", "REQ-AUTH-0001", "decided-by", "ADR-0001"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(
        out.stdout.contains("linked REQ-AUTH-0001"),
        "{}",
        out.stdout
    );

    let out = run(p, &["link", "REQ-AUTH-0001", "decided-by", "ADR-0001"]);
    assert_eq!(out.code, 0);
    assert!(out.stdout.contains("already has"), "{}", out.stdout);

    // New requirements carry a {#S1} anchor, so scenarios are linkable.
    let out = run(p, &["link", "REQ-AUTH-0002", "refines", "REQ-AUTH-0001.S1"]);
    assert_eq!(out.code, 0, "{}", out.stderr);

    let out = run(p, &["check"]);
    assert_eq!(out.code, 0, "{}", out.stdout);
    let out = run(p, &["show", "ADR-0001"]);
    assert!(
        out.stdout.contains("REQ-AUTH-0001 (decided-by)"),
        "{}",
        out.stdout
    );

    // The rest of the file is untouched.
    let file = p.join("specs/requirements/auth/admin-session-timeout.md");
    let text = fs::read_to_string(file).unwrap();
    assert!(text.contains("### Requirement: Admin session timeout"));
    assert!(text.contains("links:\n  decided-by:\n    - ADR-0001\n"));
}

#[test]
fn link_command_refuses_bad_input() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);

    let cases: [(&[&str], &str); 5] = [
        (
            &["link", "REQ-AUTH-0002", "refines", "REQ-AUTH-0001.S9"],
            "scenario",
        ),
        (
            &["link", "REQ-AUTH-0002", "refines", "REQ-NOPE-0001"],
            "does not exist",
        ),
        (&["link", "REQ-AUTH-0002", "bogus", "ADR-0001"], "known"),
        (
            &["link", "REQ-AUTH-0001", "refines", "REQ-AUTH-0001"],
            "itself",
        ),
        (&["link", "REQ-NOPE-0009", "refines", "ADR-0001"], "no node"),
    ];
    for (args, expected) in cases {
        let out = run(p, args);
        assert_eq!(out.code, 2, "{args:?}: {}", out.stdout);
        assert!(out.stderr.contains(expected), "{args:?}: {}", out.stderr);
    }
    // Nothing was written by any of the failed attempts.
    assert_eq!(run(p, &["check"]).code, 0);
    let text = fs::read_to_string(p.join("specs/requirements/auth/password-length.md")).unwrap();
    assert!(!text.contains("links:"), "{text}");
}

#[test]
fn check_reports_a_link_to_a_missing_scenario() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);

    let file = p.join("specs/requirements/auth/password-length.md");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        text.replace(
            "status: draft\n",
            "status: draft\nlinks:\n  refines: REQ-AUTH-0001.S9\n",
        ),
    )
    .unwrap();
    let out = run(p, &["check"]);
    assert_eq!(out.code, 1);
    assert!(out.stdout.contains("E009"), "{}", out.stdout);
    assert!(out.stdout.contains("{#S9}"), "{}", out.stdout);
}

#[test]
fn random_ids_do_not_collide() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    let out = run(p, &["init", "--id-scheme", "random"]);
    assert_eq!(out.code, 0, "{}", out.stderr);

    let first = created_id(&run(p, &["new", "requirement", "ord", "Place order"]));
    let second = created_id(&run(p, &["new", "requirement", "ord", "Cancel order"]));
    for id in [&first, &second] {
        let suffix = id
            .strip_prefix("REQ-ORD-")
            .unwrap_or_else(|| panic!("{id}"));
        assert_eq!(suffix.len(), 4, "{id}");
        assert!(
            suffix
                .chars()
                .all(|c| c.is_ascii_digit() || c.is_ascii_uppercase()),
            "{id}"
        );
    }
    assert_ne!(first, second);
    assert_eq!(run(p, &["check"]).code, 0);

    // The flag overrides the project setting.
    let out = run(
        p,
        &[
            "new",
            "requirement",
            "ord",
            "Refund order",
            "--id-scheme",
            "sequential",
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("REQ-ORD-0001"), "{}", out.stdout);
}

#[test]
fn json_output_carries_a_schema_version() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);
    for args in [
        &["check", "--json"][..],
        &["show", "REQ-AUTH-0001", "--json"][..],
    ] {
        let out = run(p, args);
        assert_eq!(out.code, 0, "{args:?}: {}", out.stderr);
        assert!(
            out.stdout.contains("\"schema_version\": 1"),
            "{args:?}: {}",
            out.stdout
        );
    }
}

#[test]
fn a_project_written_by_a_newer_oxispec_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init"]).code, 0);
    let config = p.join(".oxispec/config.yaml");
    let text = fs::read_to_string(&config).unwrap();
    assert!(text.contains("format: 1"), "{text}");
    fs::write(&config, text.replace("format: 1", "format: 99")).unwrap();

    for args in [&["check"][..], &["new", "requirement", "a", "B"][..]] {
        let out = run(p, args);
        assert_eq!(out.code, 2, "{args:?}");
        assert!(out.stderr.contains("format 99"), "{args:?}: {}", out.stderr);
    }
}

#[test]
fn a_project_from_iteration_one_without_format_still_works() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    fs::create_dir_all(p.join(".oxispec")).unwrap();
    fs::write(
        p.join(".oxispec/config.yaml"),
        "modules:\n- core\n- specs\ntopology: single\n",
    )
    .unwrap();
    let out = run(p, &["new", "requirement", "auth", "Login"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("REQ-AUTH-0001"));
    assert_eq!(run(p, &["check"]).code, 0);
}

// ---- local index ----

fn json(out: &Out) -> serde_json::Value {
    serde_json::from_str(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", out.stdout))
}

/// Makes every markdown file look like it was last modified an hour ago, so
/// the index trusts its size and time.
fn age_markdown_files(dir: &Path) {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(folder) = stack.pop() {
        for entry in fs::read_dir(folder).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "md") {
                let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
                file.set_modified(
                    std::time::SystemTime::now() - std::time::Duration::from_secs(3600),
                )
                .unwrap();
            }
        }
    }
}

#[test]
fn index_parses_files_once_and_then_reuses_them() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);
    age_markdown_files(p);

    let first = json(&run(p, &["index", "--rebuild", "--json"]));
    assert_eq!(first["schema_version"], 1);
    assert_eq!(first["files"], 3);
    assert_eq!(first["nodes"], 3);
    assert_eq!(first["parsed"], 3);
    assert_eq!(first["cache_written"], true);

    // Nothing changed: nothing is read or parsed.
    let second = json(&run(p, &["index", "--json"]));
    assert_eq!(second["parsed"], 0);
    assert_eq!(second["read"], 0);
    assert_eq!(second["reused"], 3);
    assert_eq!(second["cache_written"], false);

    // Change one file and delete another.
    let changed = p.join("specs/requirements/auth/password-length.md");
    let text = fs::read_to_string(&changed).unwrap();
    fs::write(&changed, format!("{text}\nMore detail.\n")).unwrap();
    fs::remove_file(p.join("specs/requirements/auth/admin-session-timeout.md")).unwrap();
    let third = json(&run(p, &["index", "--json"]));
    assert_eq!(third["parsed"], 1);
    assert_eq!(third["reused"], 1);
    assert_eq!(third["removed"], 1);
    assert_eq!(third["nodes"], 2);

    let human = run(p, &["index"]);
    assert_eq!(human.code, 0);
    assert!(
        human.stdout.contains("0 parsed, 2 reused, 0 removed"),
        "{}",
        human.stdout
    );
}

#[test]
fn commands_give_the_same_answers_with_and_without_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);
    let file = p.join("specs/requirements/auth/password-length.md");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        text.replace(
            "status: draft\n",
            "status: draft\nlinks:\n  refines: REQ-NOPE-0001\n",
        ),
    )
    .unwrap();

    let cold = run(p, &["check"]);
    let warm = run(p, &["check"]);
    let uncached = run(p, &["check", "--no-cache"]);
    assert_eq!(cold.code, 1);
    assert_eq!(cold.stdout, warm.stdout);
    assert_eq!(cold.stdout, uncached.stdout);
    assert!(cold.stdout.contains("E002"), "{}", cold.stdout);
}

#[test]
fn no_cache_leaves_no_cache_behind_and_index_refuses_it() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);
    let _ = fs::remove_dir_all(p.join(".oxispec/cache"));

    assert_eq!(run(p, &["check", "--no-cache"]).code, 0);
    assert!(!p.join(".oxispec/cache").exists());

    let out = run(p, &["index", "--no-cache"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("--no-cache"), "{}", out.stderr);
}

#[test]
fn a_damaged_cache_is_replaced_without_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);
    assert_eq!(run(p, &["check"]).code, 0);
    let cache = p.join(".oxispec/cache/index.json");
    assert!(cache.is_file());

    fs::write(&cache, "{ this is not json").unwrap();
    let out = run(p, &["check"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let repaired = fs::read_to_string(&cache).unwrap();
    assert!(serde_json::from_str::<serde_json::Value>(&repaired).is_ok());
}

// ---- check --changed ----

fn git(dir: &Path, args: &[&str]) {
    git_env(dir, args, &[]);
}

fn git_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) {
    let out = Command::new("git")
        .envs(env.iter().copied())
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.autocrlf=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn auth_path(p: &Path, slug: &str) -> std::path::PathBuf {
    p.join("specs/requirements/auth").join(format!("{slug}.md"))
}

/// Four nodes, committed:
/// - REQ-AUTH-0001 `a`, decided by ADR-0001
/// - REQ-AUTH-0002 `b`, refines scenario S1 of `a`
/// - REQ-AUTH-0003 `broken`, which has no scenario (an error nobody touches)
/// - ADR-0001
fn committed_project_with_an_old_error(p: &Path) {
    git(p, &["init", "-q"]);
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    for title in ["a", "b", "broken"] {
        assert_eq!(run(p, &["new", "requirement", "auth", title]).code, 0);
    }
    assert_eq!(run(p, &["new", "adr", "Use Postgres"]).code, 0);
    assert_eq!(
        run(p, &["link", "REQ-AUTH-0002", "refines", "REQ-AUTH-0001.S1"]).code,
        0
    );
    assert_eq!(
        run(p, &["link", "REQ-AUTH-0001", "decided-by", "ADR-0001"]).code,
        0
    );
    let broken = auth_path(p, "broken");
    let text = fs::read_to_string(&broken).unwrap();
    let without_scenario = text.split("#### Scenario:").next().unwrap().to_string();
    fs::write(&broken, without_scenario).unwrap();
    git(p, &["add", "-A"]);
    git(p, &["commit", "-q", "-m", "base"]);
}

#[test]
fn check_changed_reports_only_what_the_change_touches() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    committed_project_with_an_old_error(p);

    // The old error fails a full check, but nothing has changed yet.
    assert_eq!(run(p, &["check"]).code, 1);
    let out = run(p, &["check", "--changed"]);
    assert_eq!(out.code, 0, "{}", out.stdout);
    assert!(
        out.stdout.contains("0 of 4 nodes checked"),
        "{}",
        out.stdout
    );

    // A harmless edit to `a` checks `a` and `b`, which links to it.
    let a = auth_path(p, "a");
    let original = fs::read_to_string(&a).unwrap();
    fs::write(&a, format!("{original}\nMore detail.\n")).unwrap();
    let out = run(p, &["check", "--changed"]);
    assert_eq!(out.code, 0, "{}", out.stdout);
    assert!(
        out.stdout
            .contains("2 of 4 nodes checked (1 changed files since HEAD"),
        "{}",
        out.stdout
    );

    // Renaming the scenario anchor breaks `b`, which did not change itself.
    fs::write(&a, original.replace("{#S1}", "{#S2}")).unwrap();
    let out = run(p, &["check", "--changed"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(out.stdout.contains("E009"), "{}", out.stdout);
    assert!(out.stdout.contains("b.md"), "{}", out.stdout);
    assert!(
        !out.stdout.contains("E006"),
        "the old error is not reported: {}",
        out.stdout
    );

    // Deleting ADR-0001 breaks `a`, which links to it. The old ID comes from git.
    fs::write(&a, &original).unwrap();
    let adr = fs::read_dir(p.join("specs/adr"))
        .unwrap()
        .flatten()
        .next()
        .unwrap()
        .path();
    fs::remove_file(adr).unwrap();
    let out = run(p, &["check", "--changed"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(out.stdout.contains("E002"), "{}", out.stdout);
    assert!(out.stdout.contains("ADR-0001"), "{}", out.stdout);
    assert!(!out.stdout.contains("E006"), "{}", out.stdout);

    let out = run(p, &["check", "--changed", "--json"]);
    let report = json(&out);
    assert_eq!(report["scope"]["base"], "HEAD");
    assert_eq!(report["scope"]["changed_files"], 1);
    assert_eq!(report["errors"], 1);
}

#[test]
fn check_changed_can_compare_with_an_older_commit() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    committed_project_with_an_old_error(p);
    let a = auth_path(p, "a");
    let original = fs::read_to_string(&a).unwrap();
    fs::write(&a, format!("{original}\nMore detail.\n")).unwrap();
    git(p, &["commit", "-q", "-am", "edit a"]);

    let out = run(p, &["check", "--changed"]);
    assert!(
        out.stdout.contains("0 of 4 nodes checked"),
        "{}",
        out.stdout
    );
    let out = run(p, &["check", "--changed", "--base", "HEAD~1"]);
    assert_eq!(out.code, 0, "{}", out.stdout);
    assert!(
        out.stdout.contains("2 of 4 nodes checked"),
        "{}",
        out.stdout
    );

    let out = run(p, &["check", "--changed", "--base", "no-such-ref"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("unknown git ref"), "{}", out.stderr);
}

#[test]
fn check_changed_in_a_repository_without_commits_checks_everything() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q"]);
    assert_eq!(run(p, &["init"]).code, 0);
    assert_eq!(run(p, &["new", "requirement", "auth", "Login"]).code, 0);
    let out = run(p, &["check", "--changed"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("1 of 1 nodes checked"),
        "{}",
        out.stdout
    );
}

#[test]
fn check_changed_outside_a_git_repository_explains_itself() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init"]).code, 0);
    let out = run(p, &["check", "--changed"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("git repository"), "{}", out.stderr);
}

// ---- history, diff and --at ----

fn commit_all(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
}

/// A repository with four commits:
/// 1. REQ-AUTH-0001 and ADR-0001 are created
/// 2. REQ-AUTH-0001 is accepted and linked to the ADR
/// 3. REQ-AUTH-0002 is added
/// 4. the file of REQ-AUTH-0001 is renamed
fn project_with_history(p: &Path) {
    git(p, &["init", "-q"]);
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    run(p, &["new", "requirement", "auth", "Admin session timeout"]);
    run(p, &["new", "adr", "Use Postgres"]);
    commit_all(p, "add the auth requirement and an adr");

    run(p, &["link", "REQ-AUTH-0001", "decided-by", "ADR-0001"]);
    let file = auth_path(p, "admin-session-timeout");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(&file, text.replace("status: draft", "status: accepted")).unwrap();
    commit_all(p, "accept the timeout requirement");

    run(p, &["new", "requirement", "auth", "Password length"]);
    commit_all(p, "add password length");

    git(
        p,
        &[
            "mv",
            "specs/requirements/auth/admin-session-timeout.md",
            "specs/requirements/auth/session-timeout.md",
        ],
    );
    git(p, &["commit", "-q", "-m", "rename the file"]);
}

#[test]
fn history_lists_each_change_and_follows_a_renamed_file() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);

    let out = run(p, &["history", "REQ-AUTH-0001"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let text = out.stdout;
    assert!(
        text.contains("REQ-AUTH-0001  specs/requirements/auth/session-timeout.md"),
        "{text}"
    );
    assert!(text.contains("rename the file"), "{text}");
    assert!(
        text.contains("moved: specs/requirements/auth/admin-session-timeout.md"),
        "{text}"
    );
    assert!(text.contains("accept the timeout requirement"), "{text}");
    assert!(
        text.contains("status: draft -> accepted; link added: decided-by ADR-0001"),
        "{text}"
    );
    assert!(
        text.contains("add the auth requirement and an adr"),
        "{text}"
    );
    assert!(text.contains("created"), "{text}");
    assert!(!text.contains("uncommitted"), "{text}");
    // Newest first.
    let newest = text.find("rename the file").unwrap();
    let oldest = text.find("add the auth requirement").unwrap();
    assert!(newest < oldest, "{text}");

    // An edit that is not committed shows at the top.
    let file = auth_path(p, "session-timeout");
    let current = fs::read_to_string(&file).unwrap();
    fs::write(&file, format!("{current}\nAn extra paragraph.\n")).unwrap();
    let out = run(p, &["history", "REQ-AUTH-0001"]);
    let first_entry = out.stdout.lines().nth(1).unwrap();
    assert!(first_entry.contains("uncommitted"), "{}", out.stdout);
    assert!(out.stdout.contains("text changed"), "{}", out.stdout);
}

#[test]
fn history_json_and_limit() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);

    let out = run(p, &["history", "REQ-AUTH-0001", "--json"]);
    let report = json(&out);
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["truncated"], false);
    let entries = report["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0]["event"], "changed");
    assert_eq!(entries[0]["changes"][0]["change"], "moved");
    assert_eq!(entries[2]["event"], "created");
    assert_eq!(
        entries[2]["commit"]["subject"],
        "add the auth requirement and an adr"
    );

    let out = run(p, &["history", "REQ-AUTH-0001", "--limit", "1", "--json"]);
    let report = json(&out);
    assert_eq!(report["entries"].as_array().unwrap().len(), 1);
    assert_eq!(report["truncated"], true);
    let human = run(p, &["history", "REQ-AUTH-0001", "--limit", "1"]);
    assert!(
        human.stdout.contains("more history exists"),
        "{}",
        human.stdout
    );
}

#[test]
fn history_of_a_deleted_node_is_found_with_at() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);
    fs::remove_file(auth_path(p, "password-length")).unwrap();
    commit_all(p, "remove password length");

    let out = run(p, &["history", "REQ-AUTH-0002"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("--at"), "{}", out.stderr);

    let out = run(p, &["history", "REQ-AUTH-0002", "--at", "HEAD~1"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("add password length"), "{}", out.stdout);

    // The deletion itself shows when the history is read from the last commit.
    let out = run(p, &["history", "REQ-AUTH-0002", "--at", "HEAD"]);
    assert_eq!(out.code, 2, "the node does not exist at HEAD");
}

#[test]
fn show_and_check_can_look_at_an_older_revision() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);
    // Break something after the history above.
    let file = auth_path(p, "password-length");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        text.replace(
            "status: draft\n",
            "status: draft\nlinks:\n  refines: REQ-NOPE-0001\n",
        ),
    )
    .unwrap();
    commit_all(p, "link to something missing");

    let out = run(p, &["show", "REQ-AUTH-0001", "--at", "HEAD~4"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("[draft]"), "{}", out.stdout);
    assert!(
        out.stdout.contains("admin-session-timeout.md"),
        "{}",
        out.stdout
    );
    let out = run(p, &["show", "REQ-AUTH-0001"]);
    assert!(out.stdout.contains("[accepted]"), "{}", out.stdout);
    assert!(out.stdout.contains("session-timeout.md"), "{}", out.stdout);

    // The old revision was clean. The current one is not.
    let out = run(p, &["check", "--at", "HEAD~1"]);
    assert_eq!(out.code, 0, "{}", out.stdout);
    assert!(out.stdout.starts_with("checking "), "{}", out.stdout);
    assert_eq!(run(p, &["check"]).code, 1);

    let out = run(p, &["check", "--at", "HEAD~1", "--json"]);
    let report = json(&out);
    assert_eq!(report["at"]["subject"], "rename the file");
    assert!(report["at"]["commit"].as_str().unwrap().len() >= 40);
}

#[test]
fn diff_reports_nodes_and_findings() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);

    let out = run(p, &["diff", "HEAD~3"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let text = out.stdout;
    assert!(text.contains("to the working tree"), "{text}");
    assert!(text.contains("+ REQ-AUTH-0002"), "{text}");
    assert!(text.contains("~ REQ-AUTH-0001"), "{text}");
    assert!(text.contains("status: draft -> accepted"), "{text}");
    assert!(text.contains("findings: 0 new, 0 fixed"), "{text}");

    // Two commits that introduce, then fix, a dangling link.
    let file = auth_path(p, "password-length");
    let original = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        original.replace(
            "status: draft\n",
            "status: draft\nlinks:\n  refines: REQ-NOPE-0001\n",
        ),
    )
    .unwrap();
    commit_all(p, "break a link");
    fs::write(&file, &original).unwrap();
    commit_all(p, "fix the link");

    let out = run(p, &["diff", "HEAD~2", "HEAD~1"]);
    assert!(
        out.stdout.contains("findings: 1 new, 0 fixed"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("new error[E002]"), "{}", out.stdout);
    let out = run(p, &["diff", "HEAD~1", "HEAD"]);
    assert!(
        out.stdout.contains("findings: 0 new, 1 fixed"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("fixed error[E002]"), "{}", out.stdout);
    let out = run(p, &["diff", "HEAD~2", "HEAD"]);
    assert!(
        out.stdout.contains("no changes to any node"),
        "{}",
        out.stdout
    );

    let out = run(p, &["diff", "HEAD~2", "HEAD~1", "--json"]);
    let report = json(&out);
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["new_findings"].as_array().unwrap().len(), 1);
    assert_eq!(report["changed"][0]["id"], "REQ-AUTH-0002");
    assert_eq!(report["changed"][0]["changes"][0]["change"], "link_added");
}

#[test]
fn revisions_can_be_branches_tags_hashes_or_dates() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q"]);
    assert_eq!(run(p, &["init"]).code, 0);
    run(p, &["new", "requirement", "auth", "Login"]);
    git_env(p, &["add", "-A"], &[]);
    git_env(
        p,
        &["commit", "-q", "-m", "first"],
        &[
            ("GIT_AUTHOR_DATE", "2026-01-10T12:00:00Z"),
            ("GIT_COMMITTER_DATE", "2026-01-10T12:00:00Z"),
        ],
    );
    git(p, &["tag", "specs-v1"]);
    let file = p.join("specs/requirements/auth/login.md");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(&file, text.replace("status: draft", "status: accepted")).unwrap();
    git_env(p, &["add", "-A"], &[]);
    git_env(
        p,
        &["commit", "-q", "-m", "second"],
        &[
            ("GIT_AUTHOR_DATE", "2026-02-20T12:00:00Z"),
            ("GIT_COMMITTER_DATE", "2026-02-20T12:00:00Z"),
        ],
    );

    let status_at = |rev: &str| -> String {
        let out = run(p, &["show", "REQ-AUTH-0001", "--at", rev]);
        assert_eq!(out.code, 0, "{rev}: {}{}", out.stdout, out.stderr);
        out.stdout
    };
    assert!(status_at("HEAD").contains("[accepted]"));
    assert!(status_at("HEAD~1").contains("[draft]"));
    assert!(status_at("specs-v1").contains("[draft]"));
    // A spec release tag works without its `specs-` prefix.
    assert!(status_at("v1").contains("[draft]"));
    assert!(status_at("date:2026-01-31").contains("[draft]"));
    assert!(status_at("date:2026-02-20").contains("[accepted]"));
    // Git cannot read dates after 2099, but any date up to then works.
    assert!(status_at("date:2099-12-31").contains("[accepted]"));

    for (rev, expected) in [
        ("date:2025-12-31", "no commit on or before"),
        ("date:yesterday", "date:2026-01-31"),
        ("no-such-rev", "unknown revision"),
    ] {
        let out = run(p, &["show", "REQ-AUTH-0001", "--at", rev]);
        assert_eq!(out.code, 2, "{rev}");
        assert!(out.stderr.contains(expected), "{rev}: {}", out.stderr);
    }
}

#[test]
fn history_commands_need_a_git_repository_and_at_excludes_changed() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init"]).code, 0);
    run(p, &["new", "requirement", "auth", "Login"]);
    for args in [
        &["history", "REQ-AUTH-0001"][..],
        &["diff", "HEAD"][..],
        &["show", "REQ-AUTH-0001", "--at", "HEAD"][..],
    ] {
        let out = run(p, args);
        assert_eq!(out.code, 2, "{args:?}");
        assert!(
            out.stderr.contains("git repository"),
            "{args:?}: {}",
            out.stderr
        );
    }
    let out = run(p, &["check", "--at", "HEAD", "--changed"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("cannot be used with"), "{}", out.stderr);
}

#[test]
fn history_in_a_repository_without_commits_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q"]);
    assert_eq!(run(p, &["init"]).code, 0);
    run(p, &["new", "requirement", "auth", "Login"]);
    let out = run(p, &["history", "REQ-AUTH-0001"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("created (not committed)"),
        "{}",
        out.stdout
    );
}

// ---- rollback and tag ----

fn head_subject(p: &Path) -> String {
    let out = Command::new("git")
        .args(["log", "-1", "--format=%s"])
        .current_dir(p)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn git_status(p: &Path) -> String {
    let out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(p)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn rollback_restores_a_node_and_its_old_path_without_committing() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);

    // Commit 1 is HEAD~3: the node was a draft, in its first file.
    let out = run(p, &["rollback", "HEAD~3", "REQ-AUTH-0001"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("restore REQ-AUTH-0001"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("status: accepted -> draft"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("moved: "), "{}", out.stdout);

    assert!(auth_path(p, "admin-session-timeout").is_file());
    assert!(!auth_path(p, "session-timeout").exists());
    assert!(
        run(p, &["show", "REQ-AUTH-0001"])
            .stdout
            .contains("[draft]")
    );
    assert_eq!(run(p, &["check"]).code, 0);

    // Nothing was committed. The change waits for review.
    assert_eq!(head_subject(p), "rename the file");
    assert!(!git_status(p).is_empty());

    // Committing it is the new revision, and it shows in the history.
    commit_all(p, "roll the timeout back to a draft");
    let out = run(p, &["history", "REQ-AUTH-0001"]);
    let first = out.stdout.lines().nth(1).unwrap();
    assert!(
        first.contains("roll the timeout back to a draft"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("status: accepted -> draft"),
        "{}",
        out.stdout
    );
    // History is not rewritten: the older commits are all still there.
    assert!(
        out.stdout.contains("accept the timeout requirement"),
        "{}",
        out.stdout
    );
}

#[test]
fn rollback_dry_run_and_json_change_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);

    let out = run(p, &["rollback", "HEAD~3", "REQ-AUTH-0001", "--dry-run"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("dry run: nothing was changed"),
        "{}",
        out.stdout
    );
    assert!(git_status(p).is_empty());

    let out = run(
        p,
        &["rollback", "HEAD~3", "REQ-AUTH-0001", "--dry-run", "--json"],
    );
    let report = json(&out);
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["actions"][0]["action"], "restore");
    assert_eq!(report["actions"][0]["id"], "REQ-AUTH-0001");
    assert!(git_status(p).is_empty());
}

#[test]
fn rollback_refuses_to_break_links_unless_forced() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);
    assert_eq!(
        run(p, &["link", "REQ-AUTH-0001", "refines", "REQ-AUTH-0002"]).code,
        0
    );
    commit_all(p, "link to the password requirement");

    // REQ-AUTH-0002 did not exist in the first commit, so this deletes it.
    let out = run(p, &["rollback", "HEAD~4", "REQ-AUTH-0002"]);
    assert_eq!(out.code, 2, "{}", out.stdout);
    assert!(
        out.stdout.contains("remove REQ-AUTH-0002"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("would break"), "{}", out.stdout);
    assert!(out.stderr.contains("new error"), "{}", out.stderr);
    assert!(
        auth_path(p, "password-length").is_file(),
        "nothing was touched"
    );
    assert!(git_status(p).is_empty());

    let out = run(p, &["rollback", "HEAD~4", "REQ-AUTH-0002", "--force"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(!auth_path(p, "password-length").exists());
    assert_eq!(
        run(p, &["check"]).code,
        1,
        "the link now dangles, as warned"
    );
}

#[test]
fn rollback_refuses_to_lose_uncommitted_work_unless_forced() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);
    let file = auth_path(p, "session-timeout");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(&file, format!("{text}\nWork in progress.\n")).unwrap();

    let out = run(p, &["rollback", "HEAD~3", "REQ-AUTH-0001"]);
    assert_eq!(out.code, 2, "{}", out.stdout);
    assert!(out.stderr.contains("not committed"), "{}", out.stderr);
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("Work in progress.")
    );

    let out = run(p, &["rollback", "HEAD~3", "REQ-AUTH-0001", "--force"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(!file.exists());
}

#[test]
fn rollback_recreates_deleted_nodes_and_all_restores_everything() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);
    fs::remove_file(auth_path(p, "password-length")).unwrap();
    commit_all(p, "delete the password requirement");

    let out = run(p, &["rollback", "HEAD~1", "REQ-AUTH-0002"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("recreate REQ-AUTH-0002"),
        "{}",
        out.stdout
    );
    assert!(auth_path(p, "password-length").is_file());
    commit_all(p, "bring it back");

    // Back to the very first commit: one node is restored, one is removed.
    let out = run(p, &["rollback", "HEAD~4", "--all"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let out = run(p, &["diff", "HEAD~4"]);
    assert!(
        out.stdout.contains("no changes to any node"),
        "{}",
        out.stdout
    );
}

#[test]
fn rollback_explains_bad_input() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);
    let cases: [(&[&str], &str); 5] = [
        (&["rollback", "HEAD~1"], "name the nodes"),
        (
            &["rollback", "HEAD~1", "REQ-AUTH-0001", "--all"],
            "not both",
        ),
        (
            &["rollback", "HEAD~1", "REQ-NOPE-0001"],
            "no node REQ-NOPE-0001",
        ),
        (
            &["rollback", "no-such-rev", "REQ-AUTH-0001"],
            "unknown revision",
        ),
        (
            &[
                "rollback",
                "HEAD~1",
                "REQ-AUTH-0001",
                "--dry-run",
                "--force",
                "--all",
            ],
            "not both",
        ),
    ];
    for (args, expected) in cases {
        let out = run(p, args);
        assert_eq!(out.code, 2, "{args:?}");
        assert!(out.stderr.contains(expected), "{args:?}: {}", out.stderr);
    }
    assert!(git_status(p).is_empty());

    let plain = tempfile::tempdir().unwrap();
    assert_eq!(run(plain.path(), &["init"]).code, 0);
    let out = run(plain.path(), &["rollback", "HEAD", "REQ-AUTH-0001"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("git repository"), "{}", out.stderr);
}

#[test]
fn tags_name_revisions_and_work_with_at() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);

    let out = run(p, &["tag"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("no spec tags yet"), "{}", out.stdout);

    // Tag the first commit, then the latest one.
    let out = run(p, &["tag", "v1", "--at", "HEAD~3", "-m", "first draft"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("as specs-v1"), "{}", out.stdout);
    assert_eq!(run(p, &["tag", "v2"]).code, 0);

    let out = run(p, &["tag"]);
    assert!(out.stdout.contains("v1"), "{}", out.stdout);
    assert!(out.stdout.contains("first draft"), "{}", out.stdout);
    assert!(out.stdout.contains("v2"), "{}", out.stdout);
    let tags = json(&run(p, &["tag", "--json"]));
    assert_eq!(tags["schema_version"], 1);
    assert_eq!(tags["tags"].as_array().unwrap().len(), 2);

    // The tag is a revision for every history command.
    assert!(
        run(p, &["show", "REQ-AUTH-0001", "--at", "v1"])
            .stdout
            .contains("[draft]")
    );
    assert!(
        run(p, &["show", "REQ-AUTH-0001", "--at", "v2"])
            .stdout
            .contains("[accepted]")
    );
    let out = run(p, &["diff", "v1", "v2"]);
    assert!(out.stdout.contains("+ REQ-AUTH-0002"), "{}", out.stdout);
    assert_eq!(
        run(p, &["rollback", "v1", "REQ-AUTH-0001", "--dry-run"]).code,
        0
    );

    // Tags are never moved, and names must be valid.
    let out = run(p, &["tag", "v1"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("already exists"), "{}", out.stderr);
    let out = run(p, &["tag", "bad name"]);
    assert_eq!(out.code, 2);
    assert!(
        out.stderr.contains("not a valid tag name"),
        "{}",
        out.stderr
    );
    let out = run(p, &["tag", "--at", "HEAD"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("give it a name"), "{}", out.stderr);
}

#[test]
fn a_tag_says_when_spec_changes_are_left_out() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);
    let file = auth_path(p, "session-timeout");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(&file, format!("{text}\nNot committed.\n")).unwrap();

    let out = run(p, &["tag", "v1"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout
            .contains("1 spec file(s) have changes that are not committed"),
        "{}",
        out.stdout
    );
    let out = run(p, &["tag", "v0", "--at", "HEAD~1"]);
    assert!(!out.stdout.contains("not committed"), "{}", out.stdout);
}

// ---- change sets ----

fn change_file(p: &Path) -> std::path::PathBuf {
    p.join("specs/changes/0001-tighten-timeouts.md")
}

fn project_with_a_change_set(p: &Path) {
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    assert_eq!(
        run(p, &["new", "requirement", "auth", "Admin session timeout"]).code,
        0
    );
    let out = run(p, &["new", "change", "Tighten timeouts"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("CHG-0001"), "{}", out.stdout);
    assert!(change_file(p).is_file());
    assert_eq!(
        run(p, &["link", "CHG-0001", "changes", "REQ-AUTH-0001"]).code,
        0
    );
}

#[test]
fn a_change_set_moves_through_its_lifecycle_and_records_the_approval() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_a_change_set(p);
    assert_eq!(run(p, &["check"]).code, 0);

    let out = run(p, &["change", "list"]);
    assert!(out.stdout.contains("CHG-0001"), "{}", out.stdout);
    assert!(out.stdout.contains("draft"), "{}", out.stdout);
    assert!(out.stdout.contains("Tighten timeouts"), "{}", out.stdout);

    // Steps cannot be skipped.
    let out = run(p, &["change", "status", "CHG-0001", "approved"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("in-review"), "{}", out.stderr);
    assert!(out.stderr.contains("--force"), "{}", out.stderr);

    assert_eq!(
        run(p, &["change", "status", "CHG-0001", "in-review"]).code,
        0
    );
    let out = run(
        p,
        &[
            "change", "status", "CHG-0001", "approved", "--by", "Ann Lee",
        ],
    );
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("approved by Ann Lee on 20"),
        "{}",
        out.stdout
    );
    let text = fs::read_to_string(change_file(p)).unwrap();
    assert!(text.contains("status: approved"), "{text}");
    assert!(text.contains("approved_by: \"Ann Lee\""), "{text}");
    assert!(text.contains("approved_on: \"20"), "{text}");
    assert!(
        text.contains("links:\n  changes:\n    - REQ-AUTH-0001\n"),
        "{text}"
    );
    assert!(text.contains("# CHG-0001: Tighten timeouts"), "{text}");
    assert_eq!(run(p, &["check"]).code, 0);

    // Merging keeps who approved it.
    assert_eq!(run(p, &["change", "status", "CHG-0001", "merged"]).code, 0);
    let out = run(p, &["change", "list", "--status", "merged"]);
    assert!(out.stdout.contains("approved by Ann Lee"), "{}", out.stdout);
    let out = run(p, &["change", "list", "--status", "draft"]);
    assert!(
        out.stdout.contains("no change sets that are draft"),
        "{}",
        out.stdout
    );

    // A merged change set can be reverted, and nothing else comes after that.
    assert_eq!(
        run(p, &["change", "status", "CHG-0001", "reverted"]).code,
        0
    );
    let out = run(p, &["change", "status", "CHG-0001", "draft"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("--force"), "{}", out.stderr);
    let out = run(p, &["change", "status", "CHG-0001", "draft", "--force"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("withdrawn"), "{}", out.stdout);
    let text = fs::read_to_string(change_file(p)).unwrap();
    assert!(
        !text.contains("approved_by") && !text.contains("approved_on"),
        "{text}"
    );

    let out = run(p, &["change", "status", "CHG-0001", "draft"]);
    assert!(out.stdout.contains("already draft"), "{}", out.stdout);
}

#[test]
fn approving_needs_a_name_from_git_or_from_the_command() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_a_change_set(p);
    assert_eq!(
        run(p, &["change", "status", "CHG-0001", "in-review"]).code,
        0
    );

    // A git configuration with no user name at all.
    let empty = p.join("empty-gitconfig");
    fs::write(&empty, "").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_oxispec"))
        .args(["change", "status", "CHG-0001", "approved"])
        .current_dir(p)
        .env("GIT_CONFIG_GLOBAL", &empty)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--by NAME"), "{stderr}");
    assert!(
        !fs::read_to_string(change_file(p))
            .unwrap()
            .contains("approved")
    );
}

#[test]
fn hand_edited_change_sets_are_checked() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_a_change_set(p);
    let file = change_file(p);
    let original = fs::read_to_string(&file).unwrap();

    fs::write(&file, original.replace("status: draft", "status: approved")).unwrap();
    let out = run(p, &["check"]);
    assert_eq!(out.code, 1);
    assert!(out.stdout.contains("E013"), "{}", out.stdout);

    fs::write(&file, original.replace("status: draft", "status: finished")).unwrap();
    let out = run(p, &["check"]);
    assert_eq!(out.code, 1);
    assert!(out.stdout.contains("E012"), "{}", out.stdout);
}

#[test]
fn change_commands_refuse_nodes_and_statuses_that_do_not_fit() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_a_change_set(p);
    let cases: [(&[&str], &str); 4] = [
        (
            &["change", "status", "REQ-AUTH-0001", "approved"],
            "not a change set",
        ),
        (&["change", "status", "CHG-9999", "approved"], "no node"),
        (
            &["change", "status", "CHG-0001", "finished"],
            "unknown status",
        ),
        (
            &["change", "list", "--status", "finished"],
            "unknown status",
        ),
    ];
    for (args, expected) in cases {
        let out = run(p, args);
        assert_eq!(out.code, 2, "{args:?}");
        assert!(out.stderr.contains(expected), "{args:?}: {}", out.stderr);
    }
}

#[test]
fn change_list_json_has_what_a_release_needs() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_a_change_set(p);
    run(p, &["change", "status", "CHG-0001", "in-review"]);
    run(
        p,
        &[
            "change", "status", "CHG-0001", "approved", "--by", "Ann Lee",
        ],
    );

    let report = json(&run(p, &["change", "list", "--json"]));
    assert_eq!(report["schema_version"], 1);
    let first = &report["change_sets"][0];
    assert_eq!(first["id"], "CHG-0001");
    assert_eq!(first["status"], "approved");
    assert_eq!(first["title"], "Tighten timeouts");
    assert_eq!(first["approved_by"], "Ann Lee");
    assert_eq!(first["changes"][0], "REQ-AUTH-0001");
}

fn release_project(p: &Path) {
    git(p, &["init", "-q"]);
    let out = run(p, &["init", "--preset", "standard", "--with", "releases"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    run(p, &["new", "requirement", "auth", "Admin session timeout"]);
    commit_all(p, "first requirement");
}

fn release_tags(p: &Path) -> String {
    let out = Command::new("git")
        .args(["tag", "--list", "specs-v*"])
        .current_dir(p)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn a_first_release_is_prepared_checked_and_published() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    release_project(p);

    let out = run(p, &["release", "prepare", "--json"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let v = json(&out);
    assert_eq!(v["version"], "0.1.0");
    assert_eq!(v["base"], serde_json::Value::Null);
    assert_eq!(v["id"], "REL-0001");
    let manifest = p.join("specs/releases/0.1.0.md");
    let text = fs::read_to_string(&manifest).unwrap();
    assert!(text.contains("type: release") && text.contains("version: \"0.1.0\""));
    assert!(text.contains("The first release."), "{text}");
    assert!(text.contains("requirement REQ-AUTH-0001: Admin session timeout"));

    // A second prepare for the same version does not overwrite the notes.
    let again = run(p, &["release", "prepare", "0.1.0"]);
    assert_eq!(again.code, 2);
    assert!(again.stderr.contains("--force"), "{}", again.stderr);

    // The manifest has to be committed before publishing.
    let out = run(p, &["release", "publish", "0.1.0"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("not committed"), "{}", out.stderr);
    assert_eq!(release_tags(p), "");

    let out = run(p, &["release", "check", "0.1.0"]);
    assert_eq!(out.code, 0, "{}", out.stdout);
    assert!(out.stdout.contains("ready to publish"));
    commit_all(p, "prepare release 0.1.0");

    let out = run(p, &["release", "publish", "0.1.0"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("specs-v0.1.0"));
    assert_eq!(release_tags(p).trim(), "specs-v0.1.0");

    // It is released once, and the tag works with --at.
    let out = run(p, &["release", "publish", "0.1.0"]);
    assert_eq!(out.code, 1);
    assert!(out.stdout.contains("already released"), "{}", out.stdout);
    let out = run(p, &["show", "REQ-AUTH-0001", "--at", "v0.1.0"]);
    assert_eq!(out.code, 0, "{}", out.stderr);

    let out = run(p, &["release", "list", "--json"]);
    let v = json(&out);
    assert_eq!(v["releases"][0]["version"], "0.1.0");
    assert_eq!(v["releases"][0]["state"], "released");
    assert_eq!(v["releases"][0]["tag"], "specs-v0.1.0");

    // Nothing changed since, so there is nothing to release.
    let out = run(p, &["release", "prepare"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("nothing has changed"), "{}", out.stderr);
}

#[test]
fn a_release_waits_for_its_change_sets_to_be_approved() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    release_project(p);
    assert_eq!(run(p, &["release", "prepare", "0.1.0"]).code, 0);
    commit_all(p, "release 0.1.0");
    assert_eq!(run(p, &["release", "publish", "0.1.0"]).code, 0);

    // A new requirement, with a change set that is still a draft.
    run(p, &["new", "requirement", "auth", "Password length"]);
    run(p, &["new", "change", "Add a password rule"]);
    run(p, &["link", "CHG-0001", "changes", "REQ-AUTH-0002"]);
    let out = run(p, &["release", "prepare", "--json"]);
    let v = json(&out);
    assert_eq!(v["version"], "0.2.0", "{}", out.stdout);
    assert_eq!(v["base"], "0.1.0");
    assert_eq!(v["bump"], "minor");
    assert_eq!(v["change_sets"][0]["id"], "CHG-0001");
    let text = fs::read_to_string(p.join("specs/releases/0.2.0.md")).unwrap();
    assert!(text.contains("includes: [CHG-0001]"), "{text}");
    assert!(text.contains("## Added") && text.contains("## Change sets"));
    commit_all(p, "prepare 0.2.0");

    let out = run(p, &["release", "check", "0.2.0"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(
        out.stdout.contains("not approved yet: CHG-0001"),
        "{}",
        out.stdout
    );
    let out = run(p, &["release", "publish", "0.2.0"]);
    assert_eq!(out.code, 1);
    assert_eq!(release_tags(p).trim(), "specs-v0.1.0");

    run(p, &["change", "status", "CHG-0001", "in-review"]);
    run(
        p,
        &[
            "change", "status", "CHG-0001", "approved", "--by", "Ann Lee",
        ],
    );
    commit_all(p, "approve the change set");
    let out = run(p, &["release", "check", "0.2.0", "--json"]);
    assert_eq!(out.code, 0, "{}", out.stdout);
    assert_eq!(json(&out)["can_release"], true);
    assert_eq!(run(p, &["release", "publish", "0.2.0"]).code, 0);
    assert!(release_tags(p).contains("specs-v0.2.0"));
}

#[test]
fn removing_a_requirement_is_breaking_and_a_low_version_only_warns() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    release_project(p);
    run(p, &["new", "requirement", "auth", "Password length"]);
    commit_all(p, "second requirement");
    assert_eq!(run(p, &["release", "prepare", "0.1.0"]).code, 0);
    commit_all(p, "release 0.1.0");
    assert_eq!(run(p, &["release", "publish", "0.1.0"]).code, 0);

    fs::remove_file(auth_path(p, "password-length")).unwrap();
    let out = run(p, &["release", "prepare", "--dry-run"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("suggested version: 0.2.0"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("## Breaking"), "{}", out.stdout);
    assert!(out.stdout.contains("removed requirement REQ-AUTH-0002"));
    assert!(!p.join("specs/releases/0.2.0.md").exists());

    // A patch version for a breaking change is allowed, with a warning.
    let out = run(p, &["release", "prepare", "0.1.1"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(
        out.stdout.contains("warning: the changes suggest 0.2.0"),
        "{}",
        out.stdout
    );
    commit_all(p, "prepare 0.1.1");
    let out = run(p, &["release", "check", "0.1.1"]);
    assert_eq!(out.code, 0, "{}", out.stdout);
    assert!(out.stdout.contains("warn "), "{}", out.stdout);

    // Versions have to look like versions, and go up.
    assert_eq!(run(p, &["release", "check", "one"]).code, 2);
    assert_eq!(run(p, &["release", "prepare", "0.1.0"]).code, 2);
}

#[test]
fn releases_need_the_module_and_a_repository() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q"]);
    assert_eq!(run(p, &["init", "--preset", "minimal"]).code, 0);
    let out = run(p, &["release", "list"]);
    assert_eq!(out.code, 2);
    assert!(
        out.stderr.contains("modules add releases"),
        "{}",
        out.stderr
    );
    assert_eq!(run(p, &["modules", "add", "releases"]).code, 0);
    let out = run(p, &["release", "list"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("no releases yet"));

    let plain = tempfile::tempdir().unwrap();
    assert_eq!(
        run(
            plain.path(),
            &["init", "--preset", "minimal", "--with", "releases"]
        )
        .code,
        0
    );
    let out = run(plain.path(), &["release", "list"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("git repository"), "{}", out.stderr);
}

/// `main` has one good requirement and one that links to a node that does not
/// exist, an old problem. A branch `feature` starts from there.
fn ci_project(p: &Path) {
    git(p, &["init", "-q", "-b", "main"]);
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    run(p, &["new", "requirement", "auth", "Good"]);
    run(p, &["new", "requirement", "auth", "Old problem"]);
    run(p, &["link", "REQ-AUTH-0002", "refines", "REQ-AUTH-0001"]);
    let file = auth_path(p, "old-problem");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(&file, text.replace("REQ-AUTH-0001", "REQ-NOPE-0001")).unwrap();
    commit_all(p, "main with an old error");
    git(p, &["checkout", "-q", "-b", "feature"]);
}

#[test]
fn ci_reports_only_what_the_change_introduced() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    ci_project(p);
    assert_eq!(run(p, &["check"]).code, 1, "the old error is there");

    // A clean change passes, although `check` still fails on the old error.
    run(p, &["new", "requirement", "auth", "Clean"]);
    let out = run(p, &["ci", "--base", "main"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("0 new errors"), "{}", out.stdout);
    assert!(
        out.stdout.contains("1 existing not shown"),
        "{}",
        out.stdout
    );

    // A change that adds an error fails, and only that error is reported.
    run(p, &["new", "requirement", "auth", "Broken"]);
    let broken = auth_path(p, "broken");
    let text = fs::read_to_string(&broken).unwrap();
    fs::write(
        &broken,
        text.replace(
            "status: draft\n",
            "status: draft\nlinks:\n  refines: REQ-GONE-0001\n",
        ),
    )
    .unwrap();
    let out = run(p, &["ci", "--base", "main", "--json"]);
    assert_eq!(out.code, 1, "{}{}", out.stdout, out.stderr);
    let v = json(&out);
    assert_eq!(v["errors"], 1);
    assert_eq!(v["failed"], true);
    assert_eq!(v["existing_findings"], 1);
    assert_eq!(v["new_findings"][0]["id"], "REQ-AUTH-0004");
    assert_eq!(v["bump"], "minor");
    assert!(
        v["new_findings"][0]["message"]
            .as_str()
            .unwrap()
            .contains("REQ-GONE-0001"),
        "{v}"
    );
}

#[test]
fn ci_writes_sarif_junit_and_a_pull_request_comment() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    ci_project(p);
    run(p, &["new", "requirement", "auth", "Broken"]);
    let broken = auth_path(p, "broken");
    let text = fs::read_to_string(&broken).unwrap();
    fs::write(
        &broken,
        text.replace(
            "status: draft\n",
            "status: draft\nlinks:\n  refines: REQ-GONE-0001\n",
        ),
    )
    .unwrap();

    let out = run(
        p,
        &[
            "ci",
            "--base",
            "main",
            "--sarif",
            "out/oxispec.sarif",
            "--junit",
            "out/junit.xml",
            "--markdown",
            "out/comment.md",
        ],
    );
    assert_eq!(out.code, 1, "{}{}", out.stdout, out.stderr);

    let sarif: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(p.join("out/oxispec.sarif")).unwrap()).unwrap();
    assert_eq!(sarif["version"], "2.1.0");
    let results = sarif["runs"][0]["results"].as_array().unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["ruleId"], "E002");
    assert_eq!(
        results[0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "specs/requirements/auth/broken.md"
    );

    let junit = fs::read_to_string(p.join("out/junit.xml")).unwrap();
    assert!(junit.contains("<testsuite name=\"oxispec ci\""), "{junit}");
    assert!(junit.contains("name=\"REQ-AUTH-0003\""), "{junit}");
    assert!(junit.contains("<failure"), "{junit}");

    let comment = fs::read_to_string(p.join("out/comment.md")).unwrap();
    assert!(comment.starts_with("<!-- oxispec-ci -->"), "{comment}");
    assert!(
        comment.contains("❌ New findings: **1 error**"),
        "{comment}"
    );
    assert!(
        comment.contains("specs/requirements/auth/broken.md"),
        "{comment}"
    );
}

#[test]
fn ci_warnings_fail_only_with_strict() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    ci_project(p);
    run(p, &["new", "requirement", "auth", "Odd link"]);
    let file = auth_path(p, "odd-link");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        text.replace(
            "status: draft\n",
            "status: draft\nlinks:\n  wibble: REQ-AUTH-0001\n",
        ),
    )
    .unwrap();
    let out = run(p, &["ci", "--base", "main"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("1 new warnings"), "{}", out.stdout);
    assert_eq!(run(p, &["ci", "--base", "main", "--strict"]).code, 1);
}

#[test]
fn ci_compares_with_where_the_branch_started_not_with_newer_work_on_the_base() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    ci_project(p);
    run(p, &["new", "requirement", "auth", "On the branch"]);
    commit_all(p, "branch work");

    // `main` moves on while the branch is open.
    git(p, &["checkout", "-q", "main"]);
    run(p, &["new", "requirement", "billing", "Only on main"]);
    commit_all(p, "main moves on");
    git(p, &["checkout", "-q", "feature"]);

    let out = run(p, &["ci", "--base", "main", "--json"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let v = json(&out);
    assert_eq!(v["removed"].as_array().unwrap().len(), 0, "{v}");
    // Only the branch's own requirement is new, even though it is committed.
    let added = v["added"].as_array().unwrap();
    assert_eq!(added.len(), 1, "{v}");
    assert_eq!(added[0]["id"], "REQ-AUTH-0003");
}

#[test]
fn ci_explains_a_missing_base() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    ci_project(p);
    let out = run(p, &["ci"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("origin/main"), "{}", out.stderr);
    assert!(out.stderr.contains("fetch-depth"), "{}", out.stderr);
    let plain = tempfile::tempdir().unwrap();
    assert_eq!(run(plain.path(), &["init", "--preset", "minimal"]).code, 0);
    let out = run(plain.path(), &["ci"]);
    assert_eq!(out.code, 2);
}
