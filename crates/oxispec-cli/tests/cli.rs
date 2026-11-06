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

#[test]
fn search_finds_nodes_and_keeps_its_index_up_to_date() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);
    fs::write(
        auth_path(p, "admin-session-timeout"),
        "---\nid: REQ-AUTH-0001\ntype: requirement\nstatus: draft\n---\n### Requirement: Admin session timeout\nThe system SHALL end an idle administrator session.\n\n#### Scenario: Idle {#S1}\n- **WHEN** idle\n- **THEN** ended\n",
    )
    .unwrap();

    let out = run(p, &["search", "idle", "administrators", "--json"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let v = json(&out);
    assert_eq!(v["hits"][0]["id"], "REQ-AUTH-0001");
    assert!(
        v["hits"][0]["snippet"]
            .as_str()
            .unwrap()
            .contains("**idle**"),
        "{v}"
    );
    assert_eq!(v["index"]["added"], 3);
    assert!(p.join(".oxispec/cache/search/manifest.json").is_file());

    // The second search reads nothing again, and an edit is picked up.
    let v = json(&run(p, &["search", "idle", "--json"]));
    assert_eq!(v["index"]["unchanged"], 3);
    let text = fs::read_to_string(auth_path(p, "admin-session-timeout")).unwrap();
    fs::write(
        auth_path(p, "admin-session-timeout"),
        text.replace("idle administrator", "inactive operator")
            .replace("Idle", "Quiet")
            .replace("idle", "quiet"),
    )
    .unwrap();
    let out = run(p, &["search", "idle"]);
    assert!(out.stdout.contains("no matches"), "{}", out.stdout);
    assert!(out.stderr.contains("1 changed"), "{}", out.stderr);
    let out = run(p, &["search", "inactive", "--type", "requirement"]);
    assert!(out.stdout.contains("REQ-AUTH-0001"), "{}", out.stdout);
    let out = run(p, &["search", "inactive", "--type", "adr"]);
    assert!(out.stdout.contains("no matches"), "{}", out.stdout);
    assert_eq!(run(p, &["search", "inactive", "--rebuild"]).code, 0);
}

#[test]
fn search_without_a_cache_leaves_nothing_behind() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);
    let _ = fs::remove_dir_all(p.join(".oxispec/cache"));
    let out = run(p, &["--no-cache", "search", "postgres"]);
    assert_eq!(out.code, 0, "{}{}", out.stderr, out.stdout);
    assert!(out.stdout.contains("ADR-0001"), "{}", out.stdout);
    assert!(!p.join(".oxispec/cache").exists());
    assert_eq!(run(p, &["search", "   "]).code, 2);
}

#[test]
fn site_builds_a_page_per_node_linked_by_id() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);
    run(p, &["link", "REQ-AUTH-0001", "decided-by", "ADR-0001"]);

    let out = run(p, &["site", "--out", "public", "--title", "Demo"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("wrote 4 pages"), "{}", out.stdout);
    let site = p.join("public");
    for page in [
        "index.html",
        "REQ-AUTH-0001.html",
        "REQ-AUTH-0002.html",
        "ADR-0001.html",
        "style.css",
    ] {
        assert!(site.join(page).is_file(), "{page}");
    }
    let req = fs::read_to_string(site.join("REQ-AUTH-0001.html")).unwrap();
    assert!(
        req.contains("<a href=\"ADR-0001.html\">ADR-0001</a>"),
        "{req}"
    );
    assert!(req.contains("id=\"S1\""), "scenario anchors: {req}");
    assert!(
        req.contains("<title>REQ-AUTH-0001 Admin session timeout · Demo</title>"),
        "{req}"
    );
    let adr = fs::read_to_string(site.join("ADR-0001.html")).unwrap();
    assert!(adr.contains("What links here"), "{adr}");
    assert!(adr.contains("href=\"REQ-AUTH-0001.html\""), "{adr}");

    // A rebuild removes the page of a node that is gone, and keeps other files.
    fs::write(site.join("CNAME"), "example.com").unwrap();
    fs::remove_file(auth_path(p, "password-length")).unwrap();
    assert_eq!(run(p, &["site", "--out", "public"]).code, 0);
    assert!(!site.join("REQ-AUTH-0002.html").exists());
    assert!(site.join("CNAME").is_file());

    // The output folder must not be the project itself.
    let out = run(p, &["site", "--out", "."]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("folder of its own"), "{}", out.stderr);
}

#[test]
fn site_at_a_revision_shows_the_specs_as_they_were() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);
    git(p, &["tag", "-a", "-m", "first", "specs-first", "HEAD~3"]);

    assert_eq!(run(p, &["site", "--out", "old", "--at", "first"]).code, 0);
    assert_eq!(run(p, &["site", "--out", "new"]).code, 0);
    let old = fs::read_to_string(p.join("old/index.html")).unwrap();
    let new = fs::read_to_string(p.join("new/index.html")).unwrap();
    assert!(old.contains("2 nodes"), "{old}");
    assert!(new.contains("3 nodes"), "{new}");
    assert!(!p.join("old/REQ-AUTH-0002.html").exists());
    assert!(p.join("new/REQ-AUTH-0002.html").exists());
    let page = fs::read_to_string(p.join("old/REQ-AUTH-0001.html")).unwrap();
    assert!(page.contains("<article>"), "text comes from git: {page}");
    assert!(page.contains("status-draft"), "{page}");
}

#[test]
fn site_releases_builds_every_version_with_a_picker_and_changes() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q"]);
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    run(p, &["new", "requirement", "auth", "Admin session timeout"]);
    commit_all(p, "first");
    assert_eq!(run(p, &["tag", "v1"]).code, 0);
    run(p, &["new", "requirement", "auth", "Password length"]);
    commit_all(p, "second");
    assert_eq!(run(p, &["tag", "v2"]).code, 0);
    run(p, &["new", "adr", "Use Postgres"]);

    let out = run(p, &["site", "--out", "public", "--releases"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("3 versions"), "{}", out.stdout);
    let site = p.join("public");
    for version in ["latest", "v1", "v2"] {
        assert!(site.join(version).join("index.html").is_file(), "{version}");
        assert!(site.join(version).join("style.css").is_file(), "{version}");
    }
    assert!(!site.join("v1/REQ-AUTH-0002.html").exists());
    assert!(site.join("v2/REQ-AUTH-0002.html").is_file());
    assert!(site.join("latest/ADR-0001.html").is_file());
    assert!(!site.join("v2/ADR-0001.html").exists());

    let root = fs::read_to_string(site.join("index.html")).unwrap();
    assert!(root.contains("url=latest/index.html"), "{root}");
    assert!(
        root.contains("v1/index.html") && root.contains("v2/index.html"),
        "{root}"
    );

    let page = fs::read_to_string(site.join("v2/REQ-AUTH-0002.html")).unwrap();
    assert!(
        page.contains("<option value=\"v2\" selected>v2</option>"),
        "{page}"
    );
    assert!(page.contains("<option value=\"v1\">v1</option>"), "{page}");
    assert!(
        page.contains("Since v1") && page.contains("new in this version"),
        "{page}"
    );
    let first = fs::read_to_string(site.join("v1/REQ-AUTH-0001.html")).unwrap();
    assert!(
        !first.contains("Since"),
        "the oldest version has nothing to compare with: {first}"
    );
    let latest = fs::read_to_string(site.join("latest/ADR-0001.html")).unwrap();
    assert!(latest.contains("Since v2"), "{latest}");

    // The two ways of choosing a version do not mix.
    assert_eq!(run(p, &["site", "--releases", "--at", "v1"]).code, 2);
}

fn trace_project(p: &Path) {
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    run(p, &["new", "requirement", "auth", "Admin session timeout"]);
    let file = auth_path(p, "admin-session-timeout");
    let mut text = fs::read_to_string(&file).unwrap();
    text.push_str("\n#### Scenario: Active admin {#S2}\n- **WHEN** active\n- **THEN** open\n");
    fs::write(&file, text).unwrap();
    fs::create_dir_all(p.join("src")).unwrap();
    fs::create_dir_all(p.join("tests")).unwrap();
    fs::write(
        p.join("src/auth.rs"),
        "fn other() {}\n\n/// spec: REQ-AUTH-0001\nfn expire_admin_session() {}\n",
    )
    .unwrap();
    fs::write(
        p.join("tests/auth.rs"),
        "// verifies: REQ-AUTH-0001.S1\n#[test]\nfn idle() {}\n",
    )
    .unwrap();
}

#[test]
fn coverage_and_trace_follow_spec_and_verifies_comments() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    trace_project(p);

    let out = run(p, &["coverage"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout
            .contains("requirements: 1 implemented of 1 (100%)"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("scenarios:    1 verified of 2 (50%)"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("REQ-AUTH-0001.S2"), "{}", out.stdout);

    let v = json(&run(p, &["coverage", "--json"]));
    assert_eq!(v["unverified"][0], "REQ-AUTH-0001.S2");
    assert_eq!(v["verified_percent"], 50.0);
    assert_eq!(v["annotations"], 2);

    // A threshold turns the report into a gate.
    assert_eq!(run(p, &["coverage", "--fail-under", "50"]).code, 0);
    let out = run(p, &["coverage", "--fail-under", "100"]);
    assert_eq!(out.code, 1);
    assert!(
        out.stdout.contains("below the --fail-under"),
        "{}",
        out.stdout
    );

    let out = run(p, &["trace", "REQ-AUTH-0001"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("src/auth.rs:3"), "{}", out.stdout);
    assert!(out.stdout.contains("S1"), "{}", out.stdout);
    assert!(
        out.stdout.contains("verified by tests/auth.rs:1"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("Active admin: NOT VERIFIED"),
        "{}",
        out.stdout
    );
    let v = json(&run(p, &["trace", "REQ-AUTH-0001", "--json"]));
    assert_eq!(v["implemented_by"][0]["path"], "src/auth.rs");
    assert_eq!(
        v["scenarios"][1]["verified_by"].as_array().unwrap().len(),
        0
    );

    let out = run(p, &["trace"]);
    assert!(
        out.stdout.contains("2 annotations in 2 files"),
        "{}",
        out.stdout
    );
    assert_eq!(run(p, &["trace", "REQ-NOPE-0001"]).code, 2);

    // A test-case node's link counts as well.
    run(p, &["new", "requirement", "auth", "Other"]);
    let out = run(p, &["coverage", "--status", "accepted"]);
    assert!(
        out.stdout
            .contains("requirements: 0 implemented of 0 (100%)"),
        "{}",
        out.stdout
    );
}

#[test]
fn coverage_reports_annotations_that_point_at_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    trace_project(p);
    fs::write(
        p.join("src/more.rs"),
        "// spec: REQ-NOPE-0001\n// verifies: REQ-AUTH-0001.S9\n",
    )
    .unwrap();
    let out = run(p, &["coverage", "--json"]);
    assert_eq!(out.code, 1, "{}{}", out.stdout, out.stderr);
    let v = json(&out);
    let findings = v["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 2, "{v}");
    assert_eq!(findings[0]["code"], "E014");
    assert_eq!(findings[0]["path"], "src/more.rs");
    assert_eq!(findings[0]["line"], 1);
    assert_eq!(findings[1]["code"], "E015");
    let out = run(p, &["coverage"]);
    assert!(out.stdout.contains("error[E014]"), "{}", out.stdout);
    assert!(out.stdout.contains("--> src/more.rs:1"), "{}", out.stdout);
}

#[test]
fn trace_leaves_out_excluded_paths_markdown_and_ignored_files() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    trace_project(p);
    fs::create_dir_all(p.join("vendor")).unwrap();
    fs::create_dir_all(p.join("build")).unwrap();
    fs::write(p.join("vendor/lib.rs"), "// spec: REQ-NOPE-0001\n").unwrap();
    fs::write(p.join("build/gen.rs"), "// spec: REQ-NOPE-0002\n").unwrap();
    fs::write(p.join("NOTES.md"), "// spec: REQ-NOPE-0003\n").unwrap();
    fs::write(p.join(".gitignore"), "build/\n").unwrap();
    assert_eq!(
        run(p, &["coverage"]).code,
        1,
        "vendor is scanned until excluded"
    );

    let config = fs::read_to_string(p.join(".oxispec/config.yaml")).unwrap();
    fs::write(
        p.join(".oxispec/config.yaml"),
        format!("{config}trace:\n  exclude: [\"vendor/**\"]\n"),
    )
    .unwrap();
    let out = run(p, &["coverage"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("2 annotations in"), "{}", out.stdout);
}

#[test]
fn trace_needs_the_tests_module() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "minimal"]).code, 0);
    for command in ["trace", "coverage"] {
        let out = run(p, &[command]);
        assert_eq!(out.code, 2);
        assert!(out.stderr.contains("modules add tests"), "{}", out.stderr);
    }
}

#[test]
fn links_go_suspect_when_the_code_or_the_spec_changes_until_confirmed() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    trace_project(p);
    fs::write(
        p.join("src/auth.rs"),
        "// spec: REQ-AUTH-0001\nfn expire_admin_session() {\n    end_after(15);\n}\n",
    )
    .unwrap();

    // Nothing is confirmed yet: reported, not failed, unless asked.
    let out = run(p, &["suspect"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("2 ok") || out.stdout.contains("0 ok, 0 suspect, 2 unconfirmed"),
        "{}",
        out.stdout
    );
    assert_eq!(run(p, &["suspect", "--strict"]).code, 1);
    assert_eq!(
        run(p, &["confirm"]).code,
        2,
        "confirming needs ids or --all"
    );
    assert_eq!(run(p, &["confirm", "REQ-NOPE-0001"]).code, 2);

    let out = run(p, &["confirm", "--all", "--dry-run"]);
    assert!(out.stdout.contains("would confirm 2"), "{}", out.stdout);
    assert!(!p.join(".oxispec/trace.lock.json").exists());
    let out = run(p, &["confirm", "--all"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("confirmed 2 link(s)"), "{}", out.stdout);
    let lock = fs::read_to_string(p.join(".oxispec/trace.lock.json")).unwrap();
    assert!(
        lock.contains("spec REQ-AUTH-0001 src/auth.rs expire_admin_session"),
        "{lock}"
    );
    let out = run(p, &["suspect", "--strict"]);
    assert_eq!(out.code, 0, "{}", out.stdout);
    assert!(out.stdout.contains("2 ok, 0 suspect"), "{}", out.stdout);

    // Formatting and comments are not a change.
    fs::write(
        p.join("src/auth.rs"),
        concat!("// spe", "c: REQ-AUTH-0001 (reviewed)\nfn expire_admin_session() {\n\n        end_after(15); // minutes\n}\n"),
    )
    .unwrap();
    assert_eq!(run(p, &["suspect", "--strict"]).code, 0);

    // The code changes.
    fs::write(
        p.join("src/auth.rs"),
        "// spec: REQ-AUTH-0001\nfn expire_admin_session() {\n    end_after(30);\n}\n",
    )
    .unwrap();
    let out = run(p, &["suspect"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(out.stdout.contains("error[E016]"), "{}", out.stdout);
    assert!(out.stdout.contains("the code changed"), "{}", out.stdout);
    assert!(out.stdout.contains("--> src/auth.rs:1"), "{}", out.stdout);
    let v = json(&run(p, &["suspect", "--json"]));
    assert_eq!(v["suspect"], 1);
    assert_eq!(v["links"][0]["state"], "suspect_code");
    assert_eq!(v["links"][0]["symbol"], "expire_admin_session");

    assert_eq!(run(p, &["confirm", "REQ-AUTH-0001"]).code, 0);
    assert_eq!(run(p, &["suspect"]).code, 0);

    // The spec changes: every link to it is suspect.
    let file = auth_path(p, "admin-session-timeout");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        text.replace("<describe the behavior>", "end idle admin sessions"),
    )
    .unwrap();
    let out = run(p, &["suspect"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(out.stdout.contains("the spec changed"), "{}", out.stdout);
    assert_eq!(json(&run(p, &["suspect", "--json"]))["suspect"], 2);
    assert_eq!(run(p, &["confirm", "--all"]).code, 0);
    assert_eq!(run(p, &["suspect", "--strict"]).code, 0);

    // The annotation goes away: an orphan, dropped by confirm.
    fs::write(p.join("src/auth.rs"), "fn expire_admin_session() {}\n").unwrap();
    let out = run(p, &["suspect", "--strict"]);
    assert_eq!(out.code, 1);
    assert!(out.stdout.contains("orphaned"), "{}", out.stdout);
    let out = run(p, &["confirm", "--all"]);
    assert!(
        out.stdout.contains("dropped 1 confirmation"),
        "{}",
        out.stdout
    );
    assert_eq!(run(p, &["suspect", "--strict"]).code, 0);
}

#[test]
fn ci_reports_suspect_links_until_they_are_confirmed() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    trace_project(p);
    fs::write(
        p.join("src/auth.rs"),
        "// spec: REQ-AUTH-0001\nfn expire_admin_session() {\n    end_after(15);\n}\n",
    )
    .unwrap();
    assert_eq!(run(p, &["confirm", "--all"]).code, 0);
    commit_all(p, "main, with confirmed links");
    git(p, &["checkout", "-q", "-b", "feature"]);

    assert_eq!(run(p, &["ci", "--base", "main"]).code, 0);

    fs::write(
        p.join("src/auth.rs"),
        "// spec: REQ-AUTH-0001\nfn expire_admin_session() {\n    end_after(30);\n}\n",
    )
    .unwrap();
    let out = run(p, &["ci", "--base", "main"]);
    assert_eq!(out.code, 1, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("error[E016]"), "{}", out.stdout);
    let v = json(&run(p, &["ci", "--base", "main", "--json"]));
    assert_eq!(v["new_findings"][0]["code"], "E016");
    assert_eq!(v["new_findings"][0]["path"], "src/auth.rs");

    assert_eq!(run(p, &["confirm", "REQ-AUTH-0001"]).code, 0);
    assert_eq!(run(p, &["ci", "--base", "main"]).code, 0);
}

const JUNIT_PASS: &str = r#"<?xml version="1.0"?>
<testsuites><testsuite name="s" tests="1">
<testcase classname="auth" name="idle"/>
</testsuite></testsuites>"#;

const JUNIT_FAIL: &str = r#"<?xml version="1.0"?>
<testsuites><testsuite name="s" tests="1">
<testcase classname="auth" name="idle"><failure message="boom"/></testcase>
</testsuite></testsuites>"#;

#[test]
fn imported_test_results_say_which_scenarios_pass() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    trace_project(p);
    fs::write(
        p.join("tests/auth.rs"),
        "// verifies: REQ-AUTH-0001.S1\n#[test]\nfn idle() {}\n",
    )
    .unwrap();
    fs::write(p.join("pass.xml"), JUNIT_PASS).unwrap();
    fs::write(p.join("fail.xml"), JUNIT_FAIL).unwrap();

    // `--require-pass` needs results first.
    let out = run(p, &["coverage", "--require-pass"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("results import"), "{}", out.stderr);
    assert_eq!(run(p, &["results", "show"]).code, 2);

    let out = run(p, &["results", "import", "pass.xml"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("1 passed, 0 failed"), "{}", out.stdout);
    let out = run(p, &["coverage", "--require-pass"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("passing:      1 of 2 (50%)"),
        "{}",
        out.stdout
    );
    let v = json(&run(p, &["coverage", "--json"]));
    assert_eq!(v["results"]["passing"], 1);
    assert_eq!(v["results"]["failing"].as_array().unwrap().len(), 0);
    // The percentage that counts is the passing one.
    assert_eq!(
        run(p, &["coverage", "--require-pass", "--fail-under", "60"]).code,
        1
    );
    assert_eq!(run(p, &["coverage", "--fail-under", "60"]).code, 1);
    assert_eq!(run(p, &["coverage", "--fail-under", "50"]).code, 0);

    // A failing test fails the run with --require-pass only.
    assert_eq!(run(p, &["results", "import", "fail.xml"]).code, 0);
    let out = run(p, &["coverage", "--require-pass"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(out.stdout.contains("failing:"), "{}", out.stdout);
    assert!(out.stdout.contains("REQ-AUTH-0001.S1"), "{}", out.stdout);
    assert_eq!(run(p, &["coverage"]).code, 0);
    let out = run(p, &["results", "show"]);
    assert!(out.stdout.contains("FAIL auth::idle"), "{}", out.stdout);

    // A test can name its scenario in its own name, and go test -json works.
    fs::write(
        p.join("go.json"),
        concat!(
            "{\"Action\":\"pass\",\"Package\":\"p\",\"Test\":\"TestX [REQ-AUTH-0001",
            ".S2]\"}\n"
        ),
    )
    .unwrap();
    assert_eq!(
        run(p, &["results", "import", "go.json", "--replace"]).code,
        0
    );
    let v = json(&run(p, &["coverage", "--json"]));
    assert_eq!(v["results"]["passing"], 1);
    assert_eq!(v["results"]["tests"], 1);

    assert_eq!(run(p, &["results", "clear"]).code, 0);
    assert_eq!(run(p, &["results", "show"]).code, 2);
    fs::write(p.join("bad.txt"), "hello").unwrap();
    assert_eq!(run(p, &["results", "import", "bad.txt"]).code, 2);
    assert_eq!(run(p, &["results", "import", "missing.xml"]).code, 2);
}

#[test]
fn a_release_waits_for_confirmed_links_and_passing_tests() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q"]);
    trace_project(p);
    assert_eq!(run(p, &["modules", "add", "releases"]).code, 0);
    fs::write(
        p.join("tests/auth.rs"),
        "// verifies: REQ-AUTH-0001.S1\n#[test]\nfn idle() {}\n",
    )
    .unwrap();
    fs::write(p.join("pass.xml"), JUNIT_PASS).unwrap();
    fs::write(p.join("fail.xml"), JUNIT_FAIL).unwrap();
    fs::write(p.join(".gitignore"), ".oxispec/cache/\n*.xml\n").unwrap();
    assert_eq!(run(p, &["confirm", "--all"]).code, 0);
    assert_eq!(run(p, &["release", "prepare", "0.1.0"]).code, 0);
    commit_all(p, "release 0.1.0");
    assert_eq!(run(p, &["results", "import", "pass.xml"]).code, 0);

    let out = run(p, &["release", "check", "0.1.0"]);
    assert_eq!(out.code, 0, "{}", out.stdout);
    for gate in ["no-suspect-links", "tests-passing", "results-current"] {
        assert!(out.stdout.contains(gate), "{gate}: {}", out.stdout);
    }
    assert_eq!(run(p, &["release", "publish", "0.1.0"]).code, 0);

    // Code changes without being confirmed: the link is suspect.
    fs::write(
        p.join("src/auth.rs"),
        "/// spec: REQ-AUTH-0001\nfn expire_admin_session() {\n    end_after(30);\n}\n",
    )
    .unwrap();
    run(p, &["new", "requirement", "auth", "Password length"]);
    assert_eq!(run(p, &["release", "prepare", "0.2.0"]).code, 0);
    commit_all(p, "release 0.2.0");
    let out = run(p, &["release", "check", "0.2.0"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(
        out.stdout.contains("FAIL no-suspect-links"),
        "{}",
        out.stdout
    );
    assert_eq!(run(p, &["release", "publish", "0.2.0"]).code, 1);

    // Confirmed, but the tests are from another commit: a warning only.
    assert_eq!(run(p, &["confirm", "--all"]).code, 0);
    commit_all(p, "confirm");
    let out = run(p, &["release", "check", "0.2.0"]);
    assert_eq!(out.code, 0, "{}", out.stdout);
    assert!(
        out.stdout.contains("warn results-current"),
        "{}",
        out.stdout
    );

    // A failing test blocks it.
    assert_eq!(run(p, &["results", "import", "fail.xml"]).code, 0);
    let out = run(p, &["release", "check", "0.2.0"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(out.stdout.contains("FAIL tests-passing"), "{}", out.stdout);
    assert!(out.stdout.contains("REQ-AUTH-0001.S1"), "{}", out.stdout);
    assert_eq!(run(p, &["results", "import", "pass.xml"]).code, 0);

    // Publishing reads the committed code, not the working tree.
    fs::write(
        p.join("src/auth.rs"),
        "/// spec: REQ-AUTH-0001\nfn expire_admin_session() {\n    end_after(99);\n}\n",
    )
    .unwrap();
    assert_eq!(
        run(p, &["release", "check", "0.2.0"]).code,
        1,
        "the working tree is suspect"
    );
    let out = run(p, &["release", "publish", "0.2.0"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(release_tags(p).contains("specs-v0.2.0"));
}

#[test]
fn a_project_without_trace_annotations_has_no_trace_gates() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    release_project(p);
    assert_eq!(run(p, &["release", "prepare", "0.1.0"]).code, 0);
    commit_all(p, "release");
    let out = run(p, &["release", "check", "0.1.0"]);
    assert_eq!(out.code, 0, "{}", out.stdout);
    assert!(!out.stdout.contains("no-suspect-links"), "{}", out.stdout);
}

fn skill_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|n| n == "SKILL.md") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

#[test]
fn init_writes_skills_for_the_four_clients() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    let out = run(p, &["init", "--preset", "standard"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    for client in [".claude", ".cursor", ".github", ".gemini"] {
        let skill = p.join(client).join("skills/oxispec-explore/SKILL.md");
        let text = fs::read_to_string(&skill).unwrap_or_else(|_| panic!("{}", skill.display()));
        assert!(text.starts_with("---\nname: oxispec-explore\n"), "{text}");
    }
    assert!(!p.join(".agents").exists(), "not part of the default");
    // The standard preset has no releases module, so no release skill.
    assert!(!p.join(".claude/skills/oxispec-release").exists());
    assert!(
        p.join(".claude/skills/oxispec-sync-code/SKILL.md")
            .is_file()
    );
    let config = fs::read_to_string(p.join(".oxispec/config.yaml")).unwrap();
    assert!(
        config.contains("agents:\n- claude\n- cursor\n- copilot\n- gemini"),
        "{config}"
    );
    let out = run(p, &["agents", "check"]);
    assert_eq!(out.code, 0, "{}", out.stdout);
}

#[test]
fn init_can_choose_the_clients_or_none() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    let out = run(
        p,
        &["init", "--preset", "standard", "--agents", "claude,agents"],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(p.join(".claude/skills/oxispec-explore/SKILL.md").is_file());
    assert!(p.join(".agents/skills/oxispec-explore/SKILL.md").is_file());
    assert!(!p.join(".cursor").exists());

    let dir = tempfile::tempdir().unwrap();
    let out = run(
        dir.path(),
        &["init", "--preset", "standard", "--agents", "none"],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(!dir.path().join(".claude").exists());
    // Choosing none still leaves the commands to use later.
    let out = run(dir.path(), &["agents", "update", "--client", "cursor"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(
        dir.path()
            .join(".cursor/skills/oxispec-explore/SKILL.md")
            .is_file()
    );

    let dir = tempfile::tempdir().unwrap();
    let out = run(dir.path(), &["init", "--agents", "vim", "--with", "agents"]);
    assert_eq!(out.code, 2);
    assert!(
        out.stderr.contains("unknown client `vim`"),
        "{}",
        out.stderr
    );
    let out = run(dir.path(), &["init", "--agents", "claude"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("`agents` module"), "{}", out.stderr);
}

#[test]
fn a_project_without_the_agents_module_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init"]).code, 0);
    assert!(!p.join(".claude").exists());
    let out = run(p, &["agents", "update"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("modules add agents"), "{}", out.stderr);
    let out = run(p, &["modules", "add", "agents"]);
    assert!(out.stdout.contains("agents update"), "{}", out.stdout);
    assert_eq!(run(p, &["agents", "update"]).code, 0);
    // Only the skills for enabled modules: specs and core.
    assert!(p.join(".claude/skills/oxispec-propose/SKILL.md").is_file());
    assert!(!p.join(".claude/skills/oxispec-write-adr").exists());
}

#[test]
fn update_keeps_hand_edits_and_check_reports_drift() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(
        run(p, &["init", "--preset", "standard", "--agents", "claude"]).code,
        0
    );
    let skill = p.join(".claude/skills/oxispec-verify/SKILL.md");

    // Nothing changed: nothing is written.
    let out = run(p, &["agents", "update"]);
    assert_eq!(out.code, 0);
    assert!(out.stdout.contains("0 written"), "{}", out.stdout);

    // A missing file is drift, and update restores it.
    fs::remove_file(&skill).unwrap();
    let out = run(p, &["agents", "check"]);
    assert_eq!(out.code, 1);
    assert!(
        out.stdout.contains("oxispec-verify/SKILL.md: missing"),
        "{}",
        out.stdout
    );
    assert_eq!(run(p, &["agents", "update"]).code, 0);
    assert!(skill.is_file());

    // A hand edit is kept, reported, and replaced only with --force.
    let mine = format!(
        "{}\nOur team rule: ask before importing results.\n",
        fs::read_to_string(&skill).unwrap()
    );
    fs::write(&skill, &mine).unwrap();
    let out = run(p, &["agents", "check", "--json"]);
    assert_eq!(out.code, 1);
    let v = json(&out);
    assert_eq!(v["ok"], false);
    assert!(
        v["files"].as_array().unwrap().iter().any(
            |f| f["state"] == "edited" && f["path"] == ".claude/skills/oxispec-verify/SKILL.md"
        ),
        "{v}"
    );
    let out = run(p, &["agents", "update"]);
    assert_eq!(out.code, 1, "a skipped file is not a success");
    assert!(out.stdout.contains("edited by hand"), "{}", out.stdout);
    assert_eq!(fs::read_to_string(&skill).unwrap(), mine);
    let out = run(p, &["agents", "update", "--dry-run", "--force"]);
    assert!(
        out.stdout
            .contains("would write .claude/skills/oxispec-verify/SKILL.md"),
        "{}",
        out.stdout
    );
    assert_eq!(
        fs::read_to_string(&skill).unwrap(),
        mine,
        "dry run writes nothing"
    );
    assert_eq!(run(p, &["agents", "update", "--force"]).code, 0);
    assert_ne!(fs::read_to_string(&skill).unwrap(), mine);
    assert_eq!(run(p, &["agents", "check"]).code, 0);
}

#[test]
fn project_notes_are_added_and_refresh_unedited_files() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(
        run(
            p,
            &["init", "--preset", "standard", "--agents", "claude,cursor"]
        )
        .code,
        0
    );
    fs::create_dir_all(p.join(".oxispec/agents")).unwrap();
    fs::write(
        p.join(".oxispec/agents/_all.md"),
        "Domains are auth, billing and audit.\n",
    )
    .unwrap();
    fs::write(
        p.join(".oxispec/agents/cursor.md"),
        "Run checks with the project task runner.\n",
    )
    .unwrap();

    // The files are untouched, so a new note is a safe refresh, but it is drift.
    assert_eq!(run(p, &["agents", "check"]).code, 1);
    assert_eq!(run(p, &["agents", "update"]).code, 0);
    let claude = fs::read_to_string(p.join(".claude/skills/oxispec-explore/SKILL.md")).unwrap();
    let cursor = fs::read_to_string(p.join(".cursor/skills/oxispec-explore/SKILL.md")).unwrap();
    assert!(
        claude.contains("## Project notes\n\nDomains are auth, billing and audit.\n"),
        "{claude}"
    );
    assert!(!claude.contains("task runner"));
    assert!(
        cursor.contains(
            "Domains are auth, billing and audit.\n\nRun checks with the project task runner."
        ),
        "{cursor}"
    );
    assert_eq!(run(p, &["agents", "check"]).code, 0);
}

#[test]
fn export_writes_the_layouts_other_tools_read() {
    let dir = tempfile::tempdir().unwrap();
    let out_dir = tempfile::tempdir().unwrap();
    let o = out_dir.path().to_str().unwrap();

    // Without --all it needs a project.
    let out = run(
        dir.path(),
        &["agents", "export", "--to", "ruler", "--out", o],
    );
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("--all"), "{}", out.stderr);

    let out = run(
        dir.path(),
        &["agents", "export", "--to", "ruler", "--all", "--out", o],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(
        out_dir
            .path()
            .join(".ruler/skills/oxispec-release/SKILL.md")
            .is_file()
    );

    // In a project, only the enabled modules' skills.
    let p = dir.path();
    assert_eq!(run(p, &["init"]).code, 0);
    let other = tempfile::tempdir().unwrap();
    let out = run(
        p,
        &[
            "agents",
            "export",
            "--to",
            "skills-repo",
            "--out",
            other.path().to_str().unwrap(),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(
        other
            .path()
            .join("skills/oxispec-explore/SKILL.md")
            .is_file()
    );
    assert!(!other.path().join("skills/oxispec-release").exists());
}

#[test]
fn the_published_skills_folder_matches_the_generator() {
    let generated = tempfile::tempdir().unwrap();
    let out = run(
        generated.path(),
        &[
            "agents",
            "export",
            "--to",
            "skills-repo",
            "--all",
            "--out",
            generated.path().to_str().unwrap(),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    let committed = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../skills");
    let relative = |root: &Path| -> Vec<(String, String)> {
        skill_files(root)
            .into_iter()
            .map(|f| {
                let rel = f
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = fs::read_to_string(&f).unwrap().replace("\r\n", "\n");
                (rel, text)
            })
            .collect()
    };
    let want = relative(&generated.path().join("skills"));
    let have = relative(&committed);
    assert_eq!(
        have.iter().map(|(p, _)| p).collect::<Vec<_>>(),
        want.iter().map(|(p, _)| p).collect::<Vec<_>>(),
        "skills/ lists other skills than the generator. Run: cargo run -- agents export --to skills-repo --all"
    );
    for ((path, have), (_, want)) in have.iter().zip(&want) {
        assert_eq!(
            have, want,
            "skills/{path} is out of date. Run: cargo run -- agents export --to skills-repo --all"
        );
    }
}

/// The commands that skills tell an agent to run, as written in code spans and fences.
fn invocations(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with("oxispec ") {
            found.push(line.trim().to_string());
        }
        for (i, span) in line.split('`').enumerate() {
            if i % 2 == 1 && span.starts_with("oxispec ") {
                found.push(span.to_string());
            }
        }
    }
    found
}

#[test]
fn every_command_a_skill_names_exists() {
    let dir = tempfile::tempdir().unwrap();
    let out = run(
        dir.path(),
        &[
            "agents",
            "export",
            "--to",
            "skills-repo",
            "--all",
            "--out",
            dir.path().to_str().unwrap(),
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    let mut checked = 0;
    for file in skill_files(dir.path()) {
        let text = fs::read_to_string(&file).unwrap();
        for command in invocations(&text) {
            let words: Vec<&str> = command.split_whitespace().skip(1).collect();
            let mut path: Vec<&str> = words
                .iter()
                .copied()
                .take_while(|w| {
                    w.starts_with(|c: char| c.is_ascii_lowercase())
                        && w.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                })
                .take(3)
                .collect();
            assert!(
                !path.is_empty(),
                "{}: `{command}` names no command",
                file.display()
            );
            // The longest run of words that is a command: the rest are arguments.
            let help = loop {
                let mut args = path.clone();
                args.push("--help");
                let help = run(dir.path(), &args);
                if help.code == 0 || path.len() == 1 {
                    break help;
                }
                path.pop();
            };
            assert_eq!(
                help.code,
                0,
                "{}: `{command}` is not a command\n{}",
                file.display(),
                help.stderr
            );
            for flag in words.iter().filter(|w| w.starts_with("--")) {
                let flag = flag.split('=').next().unwrap();
                assert!(
                    help.stdout.contains(flag),
                    "{}: `{command}` uses {flag}, which `{}` does not have",
                    file.display(),
                    path.join(" ")
                );
            }
            checked += 1;
        }
    }
    assert!(
        checked >= 40,
        "only {checked} commands found; is the scanner broken?"
    );
}

/// Runs `oxispec mcp` with these requests on stdin, and returns what it printed, one value per line.
fn mcp(dir: &Path, flags: &[&str], requests: &[serde_json::Value]) -> Vec<serde_json::Value> {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(env!("CARGO_BIN_EXE_oxispec"))
        .arg("mcp")
        .args(flags)
        .current_dir(dir)
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("binary runs");
    {
        let mut stdin = child.stdin.take().unwrap();
        for r in requests {
            writeln!(stdin, "{r}").unwrap();
        }
        // Dropping stdin ends the input, and the server stops.
    }
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("not JSON: {l}: {e}")))
        .collect()
}

fn rpc(id: u32, method: &str, params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

/// Calls one tool, and returns whether it failed and its text.
fn call_tool(dir: &Path, name: &str, args: serde_json::Value) -> (bool, String) {
    let replies = mcp(
        dir,
        &[],
        &[rpc(
            1,
            "tools/call",
            serde_json::json!({"name": name, "arguments": args}),
        )],
    );
    let r = &replies[0];
    assert!(r.get("error").is_none(), "{r}");
    (
        r["result"]["isError"].as_bool().unwrap(),
        r["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string(),
    )
}

fn tool_names(replies: &[serde_json::Value]) -> Vec<String> {
    replies.iter().find(|r| r["id"] == 2).unwrap()["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

// verifies: REQ-MCP-0002.S1, REQ-MCP-0002.S2
#[test]
fn mcp_speaks_the_protocol_and_lists_tools_by_module() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);

    let replies = mcp(
        p,
        &[],
        &[
            rpc(
                1,
                "initialize",
                serde_json::json!({"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}),
            ),
            serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            rpc(2, "tools/list", serde_json::json!({})),
            rpc(3, "ping", serde_json::json!({})),
        ],
    );
    // The notification gets no answer.
    assert_eq!(replies.len(), 3, "{replies:?}");
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(replies[0]["result"]["serverInfo"]["name"], "oxispec");
    assert!(
        replies[0]["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("person approves")
    );
    let names = tool_names(&replies);
    for want in [
        "spec_search",
        "spec_get",
        "spec_neighbors",
        "spec_context",
        "spec_check",
        "spec_history",
        "spec_diff",
        "spec_trace",
        "spec_propose_change",
    ] {
        assert!(names.contains(&want.to_string()), "{want} in {names:?}");
    }
    // A version the server does not know gets its newest one.
    let replies = mcp(
        p,
        &[],
        &[rpc(
            1,
            "initialize",
            serde_json::json!({"protocolVersion": "1999-01-01"}),
        )],
    );
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-06-18");

    // Read-only leaves out the tool that writes.
    let replies = mcp(
        p,
        &["--read-only"],
        &[rpc(2, "tools/list", serde_json::json!({}))],
    );
    let names = tool_names(&replies);
    assert!(!names.contains(&"spec_propose_change".to_string()));
    assert!(names.contains(&"spec_get".to_string()));

    // No tests module, no trace tool.
    let other = tempfile::tempdir().unwrap();
    assert_eq!(run(other.path(), &["init", "--with", "mcp"]).code, 0);
    let replies = mcp(
        other.path(),
        &[],
        &[rpc(2, "tools/list", serde_json::json!({}))],
    );
    assert!(!tool_names(&replies).contains(&"spec_trace".to_string()));
}

#[test]
fn mcp_needs_its_module_and_answers_bad_messages_with_errors() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init"]).code, 0);
    let out = run(p, &["mcp"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("modules add mcp"), "{}", out.stderr);

    assert_eq!(run(p, &["modules", "add", "mcp"]).code, 0);
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(env!("CARGO_BIN_EXE_oxispec"))
        .arg("mcp")
        .current_dir(p)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        writeln!(stdin, "this is not json").unwrap();
        writeln!(
            stdin,
            "{}",
            rpc(7, "resources/subscribe", serde_json::json!({}))
        )
        .unwrap();
        writeln!(
            stdin,
            "{}",
            rpc(8, "tools/call", serde_json::json!({"name": "spec_nope"}))
        )
        .unwrap();
    }
    let out = child.wait_with_output().unwrap();
    let replies: Vec<serde_json::Value> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(replies[0]["error"]["code"], -32700);
    assert_eq!(replies[1]["id"], 7);
    assert_eq!(replies[1]["error"]["code"], -32601);
    assert_eq!(replies[2]["error"]["code"], -32602);
}

// verifies: REQ-MCP-0001.S1
#[test]
fn mcp_read_tools_return_the_same_answers_as_the_commands() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);

    let (failed, text) = call_tool(p, "spec_get", serde_json::json!({"id": "REQ-AUTH-0001"}));
    assert!(!failed, "{text}");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["node"]["id"], "REQ-AUTH-0001");
    assert!(
        v["text"].as_str().unwrap().contains("### Requirement:"),
        "the file text is included"
    );
    // A scenario ID reads its node.
    let (failed, _) = call_tool(p, "spec_get", serde_json::json!({"id": "REQ-AUTH-0001.S1"}));
    assert!(!failed);
    let (failed, text) = call_tool(p, "spec_get", serde_json::json!({"id": "REQ-NOPE-0001"}));
    assert!(failed);
    assert!(text.contains("no node with id"), "{text}");
    // At an older revision.
    let (failed, text) = call_tool(
        p,
        "spec_get",
        serde_json::json!({"id": "REQ-AUTH-0001", "at": "HEAD~3"}),
    );
    assert!(!failed, "{text}");
    assert!(text.contains("\"status\": \"draft\""), "{text}");

    let (failed, text) = call_tool(
        p,
        "spec_search",
        serde_json::json!({"query": "Postgres", "type": "adr"}),
    );
    assert!(!failed, "{text}");
    assert!(text.contains("ADR-0001"), "{text}");

    let (failed, text) = call_tool(p, "spec_neighbors", serde_json::json!({"id": "ADR-0001"}));
    assert!(!failed, "{text}");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["neighbors"][0]["id"], "REQ-AUTH-0001");
    assert_eq!(v["neighbors"][0]["link"], "decided-by");
    assert_eq!(v["neighbors"][0]["incoming"], true);

    let (failed, text) = call_tool(p, "spec_check", serde_json::json!({}));
    assert!(!failed, "{text}");
    assert!(text.contains("\"errors\": 0"), "{text}");

    let (failed, text) = call_tool(
        p,
        "spec_history",
        serde_json::json!({"id": "REQ-AUTH-0001", "limit": 2}),
    );
    assert!(!failed, "{text}");
    assert!(
        text.contains("commits") || text.contains("subject"),
        "{text}"
    );

    let (failed, text) = call_tool(p, "spec_diff", serde_json::json!({"from": "HEAD~2"}));
    assert!(!failed, "{text}");
    assert!(text.contains("REQ-AUTH-0002"), "{text}");

    let (failed, text) = call_tool(p, "spec_trace", serde_json::json!({}));
    assert!(!failed, "{text}");
}

// verifies: REQ-MCP-0003.S1, REQ-MCP-0003.S2
#[test]
fn mcp_refuses_arguments_that_could_be_read_as_options() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);
    for (tool, args) in [
        ("spec_diff", serde_json::json!({"from": "--output=x"})),
        ("spec_check", serde_json::json!({"at": "-x"})),
        ("spec_get", serde_json::json!({"id": "--help"})),
        (
            "spec_history",
            serde_json::json!({"id": "REQ-AUTH-0001 --all"}),
        ),
        (
            "spec_search",
            serde_json::json!({"query": "x", "type": "--json"}),
        ),
        ("spec_context", serde_json::json!({"ids": []})),
        ("spec_search", serde_json::json!({"limit": 5})),
        (
            "spec_neighbors",
            serde_json::json!({"id": "REQ-AUTH-0001", "depth": 99}),
        ),
    ] {
        let (failed, text) = call_tool(p, tool, args.clone());
        assert!(failed, "{tool} {args}: {text}");
    }
    // A search for words that look like an option is a search, not an option.
    let (failed, text) = call_tool(p, "spec_search", serde_json::json!({"query": "--help"}));
    assert!(!failed, "{text}");
}

// verifies: REQ-MCP-0004.S1, REQ-MCP-0004.S2, REQ-MCP-0004.S3
#[test]
fn mcp_proposes_a_draft_change_set_and_never_approves() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);

    let (failed, text) = call_tool(
        p,
        "spec_propose_change",
        serde_json::json!({"title": "Tighten timeouts", "why": "Admins stay signed in too long.", "changes": ["REQ-AUTH-0001"]}),
    );
    assert!(!failed, "{text}");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["status"], "draft");
    let id = v["id"].as_str().unwrap();
    let file = fs::read_to_string(p.join(v["path"].as_str().unwrap())).unwrap();
    assert!(file.contains("status: draft"), "{file}");
    assert!(
        file.contains("## Why\n\nAdmins stay signed in too long."),
        "{file}"
    );
    assert!(
        file.contains("changes:") && file.contains("REQ-AUTH-0001"),
        "{file}"
    );
    assert!(!file.contains("approved_by"));
    assert_eq!(run(p, &["check"]).code, 0);
    let out = run(p, &["change", "list", "--json"]);
    assert!(out.stdout.contains(id), "{}", out.stdout);

    // An unknown target writes nothing at all.
    let before = git_status(p);
    let (failed, text) = call_tool(
        p,
        "spec_propose_change",
        serde_json::json!({"title": "Other", "changes": ["REQ-AUTH-0001", "REQ-NOPE-0009"]}),
    );
    assert!(failed);
    assert!(text.contains("nothing was written"), "{text}");
    assert_eq!(git_status(p), before);

    for bad in ["", "-flag", "two\nlines"] {
        let (failed, _) = call_tool(p, "spec_propose_change", serde_json::json!({"title": bad}));
        assert!(failed, "{bad:?}");
    }

    // Read-only refuses the call itself.
    let replies = mcp(
        p,
        &["--read-only"],
        &[rpc(
            1,
            "tools/call",
            serde_json::json!({"name": "spec_propose_change", "arguments": {"title": "X"}}),
        )],
    );
    assert_eq!(replies[0]["error"]["code"], -32602);
}

#[test]
fn context_fills_a_budget_nearest_first_and_lists_what_did_not_fit() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_history(p);

    let out = run(p, &["context", "REQ-AUTH-0001"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.starts_with("# Context pack:"), "{}", out.stdout);
    let first = out
        .stdout
        .find("## REQ-AUTH-0001")
        .expect("the node asked for");
    let adr = out
        .stdout
        .find("## ADR-0001")
        .expect("the decision behind it");
    assert!(first < adr);
    assert!(out.stdout.contains("asked for"));
    assert!(
        out.stdout.contains("REQ-AUTH-0001 decided-by ADR-0001"),
        "{}",
        out.stdout
    );

    let out = run(p, &["context", "REQ-AUTH-0001", "--json", "--budget", "60"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let v = json(&out);
    assert_eq!(v["schema_version"], 1);
    assert!(v["used"].as_u64().unwrap() <= 60, "{v}");
    assert_eq!(v["items"][0]["id"], "REQ-AUTH-0001");
    assert!(
        !v["pointers"].as_array().unwrap().is_empty(),
        "what does not fit is listed: {v}"
    );

    // At an older revision, and with a depth of 0 only what was asked for.
    let out = run(
        p,
        &[
            "context",
            "REQ-AUTH-0001",
            "--depth",
            "0",
            "--at",
            "HEAD~3",
            "--json",
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(json(&out)["items"].as_array().unwrap().len(), 1);

    let out = run(p, &["context", "REQ-NOPE-0001"]);
    assert_eq!(out.code, 2);
    assert!(
        out.stderr.contains("no node with id REQ-NOPE-0001"),
        "{}",
        out.stderr
    );
}

/// A repository with a vocabulary and one accepted claim on `main`, and a branch.
fn claims_project(p: &Path) {
    git(p, &["init", "-q", "-b", "main"]);
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    let out = run(p, &["new", "vocabulary", "Auth words"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("VOC-0001"), "{}", out.stdout);
    let vocab = p.join("specs/vocabulary/0001-auth-words.md");
    let text = fs::read_to_string(&vocab).unwrap();
    fs::write(
        &vocab,
        text.replace("# session.idle_timeout", "session.idle_timeout")
            .replace("# role", "role"),
    )
    .unwrap();
    run(p, &["new", "requirement", "auth", "Default timeout"]);
    add_claims(p, "default-timeout", "for *: session.idle_timeout == 30m\n");
    commit_all(p, "a vocabulary and one claim");
    git(p, &["checkout", "-q", "-b", "feature"]);
}

/// Accepts the requirement in `slug.md` and puts a `claims` block at its end.
fn add_claims(p: &Path, slug: &str, claims: &str) {
    let file = auth_path(p, slug);
    let text = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        format!(
            "{}\n```claims\n{claims}```\n",
            text.replace("status: draft", "status: accepted")
        ),
    )
    .unwrap();
}

#[test]
fn check_finds_a_conflict_between_claims_and_an_overrides_link_settles_it() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    claims_project(p);
    let out = run(p, &["check"]);
    assert_eq!(out.code, 0, "{}", out.stdout);

    run(p, &["new", "requirement", "auth", "Admin timeout"]);
    add_claims(
        p,
        "admin-timeout",
        "for role = admin: session.idle_timeout <= 15m\n",
    );
    let out = run(p, &["check"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(out.stdout.contains("E019"), "{}", out.stdout);
    assert!(out.stdout.contains("when role = admin"), "{}", out.stdout);
    assert!(out.stdout.contains("REQ-AUTH-0001"), "{}", out.stdout);
    assert!(out.stdout.contains("REQ-AUTH-0002"), "{}", out.stdout);

    let out = run(p, &["check", "--json"]);
    let v = json(&out);
    let conflict = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["code"] == "E019")
        .expect("a conflict");
    assert_eq!(conflict["severity"], "error");
    assert_eq!(conflict["id"], "REQ-AUTH-0001");

    // The narrow requirement wins where it applies, so the conflict is intended.
    let out = run(p, &["link", "REQ-AUTH-0002", "overrides", "REQ-AUTH-0001"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let out = run(p, &["check"]);
    assert_eq!(out.code, 0, "{}", out.stdout);
}

#[test]
fn a_claim_that_cannot_be_read_is_an_error_with_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    claims_project(p);
    run(p, &["new", "requirement", "auth", "Loose"]);
    add_claims(p, "loose", "for role = admin: session.idle_timeout <= 15\n");
    let out = run(p, &["check"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(out.stdout.contains("E017"), "{}", out.stdout);
    assert!(out.stdout.contains("write a unit"), "{}", out.stdout);
    assert!(out.stdout.contains("auth/loose.md:14"), "{}", out.stdout);
}

#[test]
fn claims_lists_the_vocabulary_with_its_claims_and_the_share_of_requirements() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    claims_project(p);
    run(p, &["new", "requirement", "auth", "No claims yet"]);

    let out = run(p, &["claims"]);
    assert_eq!(out.code, 0, "{}{}", out.stderr, out.stdout);
    let text = out.stdout;
    assert!(
        text.contains("session.idle_timeout (duration), declared in VOC-0001"),
        "{text}"
    );
    assert!(text.contains("role (enum: admin, user, guest)"), "{text}");
    assert!(
        text.contains("for *: session.idle_timeout == 30m"),
        "{text}"
    );
    assert!(text.contains("no claims"), "{text}");
    assert!(
        text.contains("1 of 2 requirements have claims (50%)"),
        "{text}"
    );

    let v = json(&run(p, &["claims", "--json"]));
    assert_eq!(v["variables"].as_array().unwrap().len(), 2);
    assert_eq!(v["claims_percent"], 50.0);
    let timeout = &v["variables"][1];
    assert_eq!(timeout["name"], "session.idle_timeout");
    assert_eq!(timeout["type"], "duration");
    assert_eq!(timeout["claims"][0]["node"], "REQ-AUTH-0001");
    assert_eq!(timeout["claims"][0]["modality"], "must");
    assert_eq!(timeout["claims"][0]["active"], true);

    let v = json(&run(p, &["claims", "--json", "--var", "role"]));
    assert_eq!(v["variables"].as_array().unwrap().len(), 1);
    let out = run(p, &["claims", "--var", "nope"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("no variable `nope`"), "{}", out.stderr);
}

#[test]
fn ci_reports_a_new_conflict_once_and_not_again_after_it_is_merged() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    claims_project(p);
    run(p, &["new", "requirement", "auth", "Admin timeout"]);
    add_claims(
        p,
        "admin-timeout",
        "for role = admin: session.idle_timeout <= 15m\n",
    );
    let out = run(p, &["ci", "--base", "main", "--json"]);
    assert_eq!(out.code, 1, "{}{}", out.stdout, out.stderr);
    let v = json(&out);
    assert_eq!(v["errors"], 1);
    assert_eq!(v["new_findings"][0]["code"], "E019");

    // Merged, the conflict is there but it is not new.
    commit_all(p, "the admin timeout");
    git(p, &["checkout", "-q", "-b", "next"]);
    run(p, &["new", "requirement", "auth", "Something else"]);
    let out = run(p, &["ci", "--base", "feature", "--json"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let v = json(&out);
    assert_eq!(v["errors"], 0);
    assert_eq!(v["existing_findings"], 1);

    // Moving the claim down the file does not make it new either.
    let file = auth_path(p, "admin-timeout");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        text.replace("### Requirement", "A paragraph above.\n\n### Requirement"),
    )
    .unwrap();
    let out = run(p, &["ci", "--base", "feature", "--json"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
}

#[test]
fn claims_need_their_module() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "minimal"]).code, 0);
    for args in [&["new", "vocabulary", "Words"][..], &["claims"][..]] {
        let out = run(p, args);
        assert_eq!(out.code, 2, "{args:?}");
        assert!(out.stderr.contains("consistency"), "{}", out.stderr);
    }
    assert_eq!(run(p, &["modules", "add", "consistency"]).code, 0);
    assert_eq!(run(p, &["new", "vocabulary", "Words"]).code, 0);
    assert!(p.join("specs/vocabulary").is_dir());
}

#[test]
fn mcp_offers_the_claims_tool_with_its_module() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    claims_project(p);
    let listed = |p: &Path| {
        tool_names(&mcp(
            p,
            &[],
            &[
                rpc(
                    1,
                    "initialize",
                    serde_json::json!({"protocolVersion": "2025-06-18"}),
                ),
                rpc(2, "tools/list", serde_json::json!({})),
            ],
        ))
    };
    assert!(listed(p).contains(&"spec_claims".to_string()));

    let (is_error, text) = call_tool(
        p,
        "spec_claims",
        serde_json::json!({"variable": "session.idle_timeout"}),
    );
    assert!(!is_error, "{text}");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["variables"][0]["name"], "session.idle_timeout");
    assert_eq!(v["variables"][0]["claims"][0]["node"], "REQ-AUTH-0001");

    // An argument that could be read as an option is refused.
    let (is_error, text) = call_tool(p, "spec_claims", serde_json::json!({"variable": "--json"}));
    assert!(is_error, "{text}");

    let other = tempfile::tempdir().unwrap();
    assert_eq!(run(other.path(), &["init", "--preset", "minimal"]).code, 0);
    assert_eq!(run(other.path(), &["modules", "add", "mcp"]).code, 0);
    assert!(!listed(other.path()).contains(&"spec_claims".to_string()));
}

const ORDER_MACHINE: &str = "initial: created
states: [created, paid, shipped, delivered, cancelled]
final: [delivered, cancelled]
transitions:
  - {from: created, event: pay, to: paid, implements: [REQ-AUTH-0001]}
  - {from: paid, event: ship, to: shipped}
  - {from: shipped, event: deliver, to: delivered}
  - {from: created, event: cancel, to: cancelled}
  - {from: paid, event: cancel, to: cancelled}
";

/// A repository on `main` with a requirement and the machine SM-0001 as a new file,
/// whose block is `ORDER_MACHINE` followed by `properties`.
fn machine_project(p: &Path, properties: &str) {
    git(p, &["init", "-q", "-b", "main"]);
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    run(p, &["new", "requirement", "auth", "Login"]);
    let out = run(p, &["new", "machine", "Order"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("SM-0001"), "{}", out.stdout);
    let file = p.join("specs/machines/0001-order.md");
    fs::write(
        &file,
        format!(
            "---\nid: SM-0001\ntype: state-machine\nstatus: accepted\n---\n# SM-0001: Order\n\n```statemachine\n{ORDER_MACHINE}properties:\n{properties}```\n"
        ),
    )
    .unwrap();
}

#[test]
fn a_new_machine_passes_check_and_a_broken_property_comes_with_its_path() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    assert_eq!(run(p, &["new", "machine", "Order"]).code, 0);
    let out = run(p, &["check"]);
    assert!(!out.stdout.contains("SM-0001"), "{}", out.stdout);

    let file = p.join("specs/machines/0001-order.md");
    fs::write(
        &file,
        format!(
            "---\nid: SM-0001\ntype: state-machine\nstatus: accepted\n---\n# SM-0001: Order\n\n```statemachine\n{ORDER_MACHINE}properties:\n  - never cancel after shipped\n  - {{rule: never cancel after paid, from: REQ-AUTH-9999}}\n```\n"
        ),
    )
    .unwrap();
    let out = run(p, &["check", "--json"]);
    assert_eq!(out.code, 1, "{}{}", out.stdout, out.stderr);
    let v = json(&out);
    let findings = v["findings"].as_array().unwrap();
    let codes: Vec<_> = findings
        .iter()
        .map(|f| f["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"E023"), "{codes:?}");
    assert!(codes.contains(&"E024"), "{codes:?}");
    let e023 = findings.iter().find(|f| f["code"] == "E023").unwrap();
    assert!(
        e023["message"]
            .as_str()
            .unwrap()
            .contains("created -pay-> paid -cancel-> cancelled"),
        "{e023}"
    );
}

#[test]
fn machine_lists_shows_draws_and_writes_tests() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    machine_project(
        p,
        "  - never cancel after shipped\n  - never cancel after paid\n",
    );

    let out = run(p, &["machine"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(
        out.stdout
            .contains("SM-0001        5 states, 5 transitions, 2 properties (1 violated)"),
        "{}",
        out.stdout
    );

    let out = run(p, &["machine", "SM-0001"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("created -pay-> paid"), "{}", out.stdout);
    assert!(out.stdout.contains("holds     never cancel after shipped"));
    assert!(
        out.stdout
            .contains("violated  never cancel after paid: created -pay-> paid -cancel-> cancelled"),
        "{}",
        out.stdout
    );

    let v = json(&run(p, &["machine", "SM-0001", "--json"]));
    let m = &v["machines"][0];
    assert_eq!(m["id"], "SM-0001");
    assert_eq!(m["properties"][0]["holds"], true);
    assert_eq!(m["properties"][1]["holds"], false);
    assert_eq!(
        m["properties"][1]["counterexample"]["steps"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let out = run(p, &["machine", "SM-0001", "--diagram"]);
    assert!(
        out.stdout.starts_with("stateDiagram-v2\n"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("paid --> shipped: ship"));

    let v = json(&run(p, &["machine", "SM-0001", "--tests", "--json"]));
    assert_eq!(v["tests"].as_array().unwrap().len(), 5);
    assert_eq!(v["total"], 5);
    assert_eq!(v["truncated"], false);
    assert_eq!(v["tests"][0]["implements"][0], "REQ-AUTH-0001");

    let v = json(&run(
        p,
        &["machine", "SM-0001", "--tests", "--limit", "2", "--json"],
    ));
    assert_eq!(v["tests"].as_array().unwrap().len(), 2);
    assert_eq!(v["total"], 5);
    assert_eq!(v["truncated"], true);
    let out = run(p, &["machine", "SM-0001", "--tests", "--limit", "2"]);
    assert!(out.stdout.contains("2 of 5 tests shown"), "{}", out.stdout);

    let out = run(p, &["machine", "SM-9999"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("no state machine `SM-9999`"));
    let out = run(p, &["machine", "--diagram"]);
    assert_eq!(out.code, 2);
}

#[test]
fn ci_reports_a_new_broken_property_once() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    machine_project(p, "  - never cancel after shipped\n");
    commit_all(p, "an order machine");
    git(p, &["checkout", "-q", "-b", "feature"]);
    let file = p.join("specs/machines/0001-order.md");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        text.replace("never cancel after shipped", "never cancel after paid"),
    )
    .unwrap();
    let out = run(p, &["ci", "--base", "main", "--json"]);
    assert_eq!(out.code, 1, "{}{}", out.stdout, out.stderr);
    assert_eq!(json(&out)["new_findings"][0]["code"], "E023");

    // Merged, the violation is there but it is not new, even when the block moves down.
    commit_all(p, "break the property");
    git(p, &["checkout", "-q", "-b", "next"]);
    let moved = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        moved.replace("# SM-0001: Order", "# SM-0001: Order\n\nA paragraph above."),
    )
    .unwrap();
    let out = run(p, &["ci", "--base", "feature", "--json"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert_eq!(json(&out)["existing_findings"], 1);
}

#[test]
fn machines_need_their_module() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "minimal"]).code, 0);
    for args in [&["new", "machine", "Order"][..], &["machine"][..]] {
        let out = run(p, args);
        assert_eq!(out.code, 2, "{args:?}");
        assert!(out.stderr.contains("statemachines"), "{}", out.stderr);
    }
    assert_eq!(run(p, &["modules", "add", "statemachines"]).code, 0);
    assert_eq!(run(p, &["new", "machine", "Order"]).code, 0);
    assert!(p.join("specs/machines").is_dir());
}

#[test]
fn mcp_offers_the_machine_tool_with_its_module() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    machine_project(p, "  - never cancel after paid\n");
    let (is_error, text) = call_tool(p, "spec_machine", serde_json::json!({}));
    assert!(!is_error, "{text}");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["machines"][0]["id"], "SM-0001");

    let (is_error, text) = call_tool(
        p,
        "spec_machine",
        serde_json::json!({"id": "SM-0001", "view": "diagram"}),
    );
    assert!(!is_error, "{text}");
    assert!(text.starts_with("stateDiagram-v2"), "{text}");

    let (is_error, text) = call_tool(p, "spec_machine", serde_json::json!({"view": "tests"}));
    assert!(is_error, "{text}");

    let other = tempfile::tempdir().unwrap();
    assert_eq!(run(other.path(), &["init", "--preset", "minimal"]).code, 0);
    assert_eq!(run(other.path(), &["modules", "add", "mcp"]).code, 0);
    let names = tool_names(&mcp(
        other.path(),
        &[],
        &[
            rpc(
                1,
                "initialize",
                serde_json::json!({"protocolVersion": "2025-06-18"}),
            ),
            rpc(2, "tools/list", serde_json::json!({})),
        ],
    ));
    assert!(!names.contains(&"spec_machine".to_string()));
}

/// Writes an architecture element or environment as a node.
fn arch_node(p: &Path, file: &str, id: &str, ty: &str, block: &str) {
    let path = p.join("architecture/model").join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!(
            "---\nid: {id}\ntype: {ty}\nstatus: accepted\n---\n# {id}\n\n```architecture\n{block}```\n"
        ),
    )
    .unwrap();
}

/// A shop: web and API containers and a database in one system, a customer and a payment system.
fn architecture_project(p: &Path) {
    git(p, &["init", "-q", "-b", "main"]);
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    arch_node(p, "shop.md", "C4-0001", "system", "name: Shop\n");
    arch_node(
        p,
        "web.md",
        "C4-0002",
        "container",
        "name: Web\ntechnology: React\nowner: web\nparent: C4-0001\nuses:\n  - {to: C4-0003, description: calls, technology: HTTPS}\n",
    );
    arch_node(
        p,
        "api.md",
        "C4-0003",
        "container",
        "name: Orders API\ntechnology: Go\nowner: orders\nparent: C4-0001\nuses:\n  - {to: C4-0004, description: reads and writes}\n  - {to: C4-0006, description: charges cards}\n",
    );
    arch_node(
        p,
        "db.md",
        "C4-0004",
        "container",
        "name: Orders DB\ntechnology: Postgres\nowner: orders\nparent: C4-0001\n",
    );
    arch_node(
        p,
        "customer.md",
        "C4-0005",
        "person",
        "name: Customer\nuses:\n  - {to: C4-0002, description: browses}\n",
    );
    arch_node(p, "payments.md", "C4-0006", "system", "name: Payments\n");
    arch_node(
        p,
        "prod.md",
        "DEP-0001",
        "deployment-environment",
        "name: Production\nnodes:\n  - {id: eks, kind: kubernetes, region: eu-west-1, hosts: [C4-0002, C4-0003]}\n  - {id: rds, hosts: [C4-0004]}\n",
    );
}

#[test]
fn new_architecture_writes_elements_and_a_container_needs_its_parent() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    let out = run(p, &["new", "architecture", "system", "Shop"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("C4-0001"), "{}", out.stdout);
    assert!(p.join("architecture/model/0001-shop.md").is_file());
    let out = run(p, &["new", "architecture", "environment", "Production"]);
    assert!(out.stdout.contains("DEP-0001"), "{}", out.stdout);
    assert_eq!(run(p, &["check"]).code, 0);

    assert_eq!(run(p, &["new", "architecture", "container", "Web"]).code, 0);
    let out = run(p, &["check", "--json"]);
    let v = json(&out);
    let messages: Vec<_> = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["message"].as_str().unwrap().to_string())
        .collect();
    assert!(
        messages
            .iter()
            .any(|m| m == "container C4-0002: has no `parent`, which must be a system"),
        "{messages:?}"
    );
}

#[test]
fn architecture_lists_and_draws_views_from_one_model() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    architecture_project(p);
    assert_eq!(run(p, &["check"]).code, 0);

    let out = run(p, &["architecture"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("container"), "{}", out.stdout);
    assert!(
        out.stdout.contains("Orders API [Go] in C4-0001"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("environment DEP-0001"),
        "{}",
        out.stdout
    );

    let v = json(&run(p, &["architecture", "--json"]));
    assert_eq!(v["elements"].as_array().unwrap().len(), 6);
    assert_eq!(v["environments"][0]["nodes"][0]["hosts"][0], "C4-0002");

    let out = run(p, &["architecture", "view", "context", "C4-0001"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("title: context view of Shop"));
    assert!(
        out.stdout.contains("C4_0005 -->|\"browses\"| C4_0001"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout
            .contains("C4_0001 -->|\"charges cards\"| C4_0006")
    );

    let out = run(p, &["architecture", "view", "container", "C4-0001"]);
    assert!(
        out.stdout.contains("subgraph C4_0001[\"Shop\"]"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout
            .contains("C4_0002 -->|\"calls [HTTPS]\"| C4_0003")
    );

    let v = json(&run(
        p,
        &["architecture", "view", "deployment", "DEP-0001", "--json"],
    ));
    assert_eq!(v["nodes"].as_array().unwrap().len(), 3);
    assert!(v["mermaid"].as_str().unwrap().contains("Production"));

    let out = run(p, &["architecture", "view", "component", "C4-0001"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("is of a container"), "{}", out.stderr);
}

#[test]
fn a_broken_architecture_rule_names_the_path_and_ci_reports_it_once() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    architecture_project(p);
    commit_all(p, "a shop");
    git(p, &["checkout", "-q", "-b", "feature"]);
    arch_node(
        p,
        "shop.md",
        "C4-0001",
        "system",
        "name: Shop\nrules:\n  - forbid C4-0002 ->* C4-0004\n",
    );
    let out = run(p, &["ci", "--base", "main", "--json"]);
    assert_eq!(out.code, 1, "{}{}", out.stdout, out.stderr);
    let v = json(&out);
    assert_eq!(v["new_findings"][0]["code"], "E027");
    assert!(
        v["new_findings"][0]["message"]
            .as_str()
            .unwrap()
            .ends_with("is broken by C4-0002 -> C4-0003 -> C4-0004"),
        "{v}"
    );

    commit_all(p, "forbid it");
    git(p, &["checkout", "-q", "-b", "next"]);
    run(p, &["new", "requirement", "auth", "Something else"]);
    let out = run(p, &["ci", "--base", "feature", "--json"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert_eq!(json(&out)["existing_findings"], 1);
}

#[test]
fn architecture_needs_its_module() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "minimal"]).code, 0);
    for args in [
        &["new", "architecture", "system", "Shop"][..],
        &["architecture"][..],
    ] {
        let out = run(p, args);
        assert_eq!(out.code, 2, "{args:?}");
        assert!(out.stderr.contains("architecture"), "{}", out.stderr);
    }
    assert_eq!(run(p, &["modules", "add", "architecture"]).code, 0);
    assert_eq!(run(p, &["new", "architecture", "system", "Shop"]).code, 0);
    assert!(p.join("architecture/model").is_dir());
}

#[test]
fn mcp_offers_the_architecture_tool_with_its_module() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    architecture_project(p);
    let (is_error, text) = call_tool(p, "spec_architecture", serde_json::json!({}));
    assert!(!is_error, "{text}");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["elements"].as_array().unwrap().len(), 6);

    let (is_error, text) = call_tool(
        p,
        "spec_architecture",
        serde_json::json!({"view": "container", "id": "C4-0001"}),
    );
    assert!(!is_error, "{text}");
    assert!(text.contains("flowchart LR"), "{text}");

    let (is_error, _) = call_tool(
        p,
        "spec_architecture",
        serde_json::json!({"view": "container"}),
    );
    assert!(is_error);
    let (is_error, _) = call_tool(
        p,
        "spec_architecture",
        serde_json::json!({"view": "--json", "id": "C4-0001"}),
    );
    assert!(is_error);

    let other = tempfile::tempdir().unwrap();
    assert_eq!(run(other.path(), &["init", "--preset", "minimal"]).code, 0);
    assert_eq!(run(other.path(), &["modules", "add", "mcp"]).code, 0);
    let names = tool_names(&mcp(
        other.path(),
        &[],
        &[
            rpc(
                1,
                "initialize",
                serde_json::json!({"protocolVersion": "2025-06-18"}),
            ),
            rpc(2, "tools/list", serde_json::json!({})),
        ],
    ));
    assert!(!names.contains(&"spec_architecture".to_string()));
}

#[test]
fn the_library_dependency_version_matches_the_release_version() {
    let manifest =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml")).unwrap();
    let want = format!("version = \"={}\"", env!("CARGO_PKG_VERSION"));
    assert!(
        manifest
            .lines()
            .any(|l| l.starts_with("oxispec-core") && l.contains(&want)),
        "`oxispec-core` in [workspace.dependencies] must say {want}; see docs/releasing.md"
    );
}

#[test]
fn new_commands_running_at_once_get_different_ids() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    assert_eq!(run(p, &["init", "--preset", "minimal"]).code, 0);
    let handles: Vec<_> = (0..8)
        .map(|n| {
            let p = p.to_path_buf();
            std::thread::spawn(move || {
                for k in 0..4 {
                    let out = run(
                        &p,
                        &["new", "requirement", "race", &format!("writer {n} {k}")],
                    );
                    assert_eq!(out.code, 0, "{}", out.stderr);
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let v = json(&run(p, &["check", "--json"]));
    let duplicates = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["code"] == "E001")
        .count();
    assert_eq!(duplicates, 0, "{v}");
    assert_eq!(v["nodes"], 32, "{v}");
}

#[test]
fn a_closed_pipe_ends_the_program_quietly() {
    use std::process::Stdio;
    let dir = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_oxispec"))
        .args(["init", "--preset", "standard"])
        .current_dir(dir.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Like `oxispec init | true`: nobody reads the output.
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn hostile_files_never_make_a_command_panic() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    assert_eq!(
        run(p, &["init", "--preset", "standard", "--agents", "none"]).code,
        0
    );
    let h = p.join("specs/h");
    fs::create_dir_all(&h).unwrap();
    let files: &[(&str, &[u8])] = &[
        ("empty.md", b""),
        ("dashes.md", b"---\n"),
        ("nothing.md", b"---\n---\n"),
        ("binary.md", b"\xff\xfe\x00bad"),
        ("nulbyte.md", b"---\nid: REQ-H-0006\ntype: requirement\n---\nNUL \x00 here ### Requirement: x\n"),
        ("bom.md", b"\xef\xbb\xbf---\nid: REQ-H-0002\ntype: requirement\n---\n### Requirement: Bom\nThe system SHALL.\n"),
        ("nested.md", b"---\nid: REQ-H-0004\ntype: requirement\nlinks:\n  satisfies:\n    - a: {b: [c]}\n---\ntext\n"),
        ("self.md", b"---\nid: REQ-H-0005\ntype: requirement\nlinks:\n  supersedes: REQ-H-0005\n  refines: REQ-H-0005\n---\n### Requirement: Self\nThe system SHALL.\n\n#### Scenario: S\n- a\n"),
        ("open.md", b"---\nid: SM-H-0001\ntype: state-machine\n---\n```statemachine\ninitial: a\nstates: [a\n"),
        ("crlf.md", b"---\r\nid: SM-H-0002\r\ntype: state-machine\r\n---\r\n```statemachine\r\ninitial: a\r\nstates: [a, b]\r\ntransitions:\r\n  - {from: a, event: go, to: b}\r\nproperties:\r\n  - never reach b\r\n```\r\n"),
        ("arch.md", b"---\r\nid: C4-H-0001\r\ntype: system\r\n---\r\n```architecture\r\nname: X\r\n```\r\n"),
    ];
    for (name, bytes) in files {
        fs::write(h.join(name), bytes).unwrap();
    }
    let long = format!(
        "---\nid: REQ-H-0003\ntype: requirement\n---\n### Requirement: Long\nThe system SHALL {}.\n",
        "x".repeat(300_000)
    );
    fs::write(h.join("long.md"), long).unwrap();
    commit_all(p, "hostile");

    for args in [
        &["check", "--json"][..],
        &["check", "--changed"],
        &["search", "system"],
        &["trace", "--json"],
        &["coverage", "--json"],
        &["claims", "--json"],
        &["machine", "--json"],
        &["machine", "SM-H-0002", "--tests"],
        &["machine", "SM-H-0002", "--diagram"],
        &["architecture", "--json"],
        &["context", "REQ-H-0002"],
        &["show", "REQ-H-0002"],
        &["history", "REQ-H-0002"],
        &["diff", "HEAD"],
        &["ci", "--base", "HEAD"],
        &["suspect"],
        &["index"],
    ] {
        let out = run(p, args);
        assert!(!out.stderr.contains("panicked"), "{args:?}: {}", out.stderr);
        assert!(
            matches!(out.code, 0 | 1),
            "{args:?} exited {}: {}",
            out.code,
            out.stderr
        );
    }
    // The byte order mark did not hide the node, and CRLF did not hide the machine's problem.
    assert_eq!(run(p, &["show", "REQ-H-0002"]).code, 0);
    let check = run(p, &["check"]);
    assert!(check.stdout.contains("E023"), "{}", check.stdout);
}

#[test]
fn links_added_at_the_same_time_to_one_file_are_all_kept() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    assert_eq!(
        run(p, &["init", "--preset", "standard", "--agents", "none"]).code,
        0
    );
    run(p, &["new", "requirement", "race", "Source"]);
    for n in 0..8 {
        run(p, &["new", "requirement", "race", &format!("Target {n}")]);
    }
    let handles: Vec<_> = (2..=9)
        .map(|n| {
            let p = p.to_path_buf();
            std::thread::spawn(move || {
                let to = format!("REQ-RACE-{n:04}");
                let out = run(&p, &["link", "REQ-RACE-0001", "refines", &to]);
                assert_eq!(out.code, 0, "{}", out.stderr);
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    let v = json(&run(p, &["show", "REQ-RACE-0001", "--json"]));
    let links = v["node"]["links"]["refines"].as_array().unwrap();
    assert_eq!(links.len(), 8, "{v}");
}

#[test]
fn agents_list_names_clients_and_skills_and_bad_client_names_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    assert_eq!(
        run(p, &["init", "--preset", "standard", "--agents", "claude"]).code,
        0
    );

    let out = run(p, &["agents", "list"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("* claude"), "{}", out.stdout);
    assert!(out.stdout.contains("  cursor"), "{}", out.stdout);
    assert!(out.stdout.contains("oxispec-explore"), "{}", out.stdout);

    let v = json(&run(p, &["agents", "list", "--json"]));
    assert_eq!(v["module_enabled"], true);
    assert_eq!(v["clients"][0]["selected"], true);
    assert!(
        v["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["name"] == "oxispec-model-architecture" && s["enabled"] == true)
    );

    let out = run(p, &["agents", "update", "--client", "nope"]);
    assert_eq!(out.code, 2);
    assert!(
        out.stderr.contains("unknown client `nope`"),
        "{}",
        out.stderr
    );
    let out = run(p, &["agents", "check", "--client", "nope"]);
    assert_eq!(out.code, 2);

    // A skill edited by hand is reported, and a dry run writes nothing.
    let skill = p.join(".claude/skills/oxispec-explore/SKILL.md");
    fs::write(&skill, "my own text\n").unwrap();
    let out = run(p, &["agents", "check", "--json"]);
    assert_eq!(out.code, 1);
    let v = json(&out);
    assert_eq!(v["ok"], false);
    assert!(
        v["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["state"] == "edited")
    );
    let out = run(p, &["agents", "update", "--dry-run", "--force"]);
    assert!(out.stdout.contains("would write"), "{}", out.stdout);
    assert_eq!(fs::read_to_string(&skill).unwrap(), "my own text\n");
}

#[test]
fn init_refuses_unknown_clients_and_none_combined_with_others() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    let out = run(p, &["init", "--preset", "standard", "--agents", "bogus"]);
    assert_eq!(out.code, 2);
    assert!(
        out.stderr.contains("unknown client `bogus`"),
        "{}",
        out.stderr
    );
    let out = run(
        p,
        &["init", "--preset", "standard", "--agents", "none,claude"],
    );
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("none"), "{}", out.stderr);
    assert!(!p.join(".oxispec").exists());
    let out = run(p, &["init", "--preset", "standard", "--agents", "none"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(!p.join(".claude").exists());
}

#[test]
fn a_small_budget_leaves_related_nodes_as_pointers() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    assert_eq!(
        run(p, &["init", "--preset", "standard", "--agents", "none"]).code,
        0
    );
    for n in 1..=4 {
        run(p, &["new", "requirement", "a", &format!("R{n}")]);
    }
    for n in 2..=4 {
        let to = format!("REQ-A-000{n}");
        assert_eq!(run(p, &["link", "REQ-A-0001", "refines", &to]).code, 0);
    }
    let out = run(p, &["context", "REQ-A-0001", "--budget", "120"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(
        out.stdout.contains("## Related, not included"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("- REQ-A-0004 (requirement)"),
        "{}",
        out.stdout
    );
    let out = run(p, &["context", "REQ-A-9999"]);
    assert_eq!(out.code, 2);
}

#[test]
fn connect_writes_the_mcp_entry_and_keeps_other_settings() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(run(dir.path(), &["init"]).code, 0);
    std::fs::create_dir_all(dir.path().join(".gemini")).unwrap();
    std::fs::write(
        dir.path().join(".gemini/settings.json"),
        r#"{"theme":"dark"}"#,
    )
    .unwrap();

    let dry = run(dir.path(), &["connect", "--dry-run"]);
    assert_eq!(dry.code, 0, "{}", dry.stderr);
    assert!(dry.stdout.contains("would create .mcp.json"));
    assert!(!dir.path().join(".mcp.json").exists());

    let out = run(dir.path(), &["connect", "--read-only"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let claude = std::fs::read_to_string(dir.path().join(".mcp.json")).unwrap();
    assert!(claude.contains("--read-only"));
    let vscode = std::fs::read_to_string(dir.path().join(".vscode/mcp.json")).unwrap();
    assert!(vscode.contains("\"servers\"") && vscode.contains("stdio"));
    let gemini = std::fs::read_to_string(dir.path().join(".gemini/settings.json")).unwrap();
    assert!(gemini.contains("\"theme\": \"dark\"") && gemini.contains("oxispec"));

    let again = run(dir.path(), &["connect", "--read-only"]);
    assert!(
        again.stdout.contains("unchanged .mcp.json"),
        "{}",
        again.stdout
    );
}

#[test]
fn connect_refuses_unknown_clients_and_files_it_cannot_merge() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(run(dir.path(), &["init"]).code, 0);
    let bad = run(dir.path(), &["connect", "vim"]);
    assert_eq!(bad.code, 2);
    assert!(bad.stderr.contains("unknown client `vim`"));

    std::fs::write(dir.path().join(".mcp.json"), "{ // mine\n}").unwrap();
    let out = run(dir.path(), &["connect", "claude", "cursor"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("by hand"), "{}", out.stderr);
    // Nothing else was written either.
    assert!(!dir.path().join(".cursor/mcp.json").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join(".mcp.json")).unwrap(),
        "{ // mine\n}"
    );
}

#[test]
fn status_summarizes_the_project_in_text_and_json() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);

    let text = run(p, &["status"]);
    assert_eq!(text.code, 0, "{}", text.stderr);
    assert!(text.stdout.contains("3 nodes"), "{}", text.stdout);
    assert!(text.stdout.contains("requirement"), "{}", text.stdout);

    let json = run(p, &["status", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["nodes"], 3);
    assert_eq!(v["types"]["requirement"], 2);

    let outside = tempfile::tempdir().unwrap();
    assert_eq!(run(outside.path(), &["status"]).code, 2);
}

#[test]
fn mcp_offers_nodes_as_resources_and_a_few_prompts() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);
    let id = {
        let out = run(p, &["new", "requirement", "auth", "Third one"]);
        created_id(&out)
    };
    let uri = format!("oxispec://node/{id}");
    let replies = mcp(
        p,
        &[],
        &[
            rpc(
                1,
                "initialize",
                serde_json::json!({"protocolVersion": "2025-06-18"}),
            ),
            rpc(2, "resources/list", serde_json::json!({})),
            rpc(3, "resources/templates/list", serde_json::json!({})),
            rpc(4, "resources/read", serde_json::json!({"uri": uri})),
            rpc(
                5,
                "resources/read",
                serde_json::json!({"uri": "file:///etc/passwd"}),
            ),
            rpc(
                6,
                "resources/read",
                serde_json::json!({"uri": "oxispec://node/--bad"}),
            ),
            rpc(7, "resources/list", serde_json::json!({"cursor": "x"})),
            rpc(8, "prompts/list", serde_json::json!({})),
            rpc(
                9,
                "prompts/get",
                serde_json::json!({"name": "assess-impact", "arguments": {"id": id}}),
            ),
            rpc(
                10,
                "prompts/get",
                serde_json::json!({"name": "nope", "arguments": {"id": id}}),
            ),
            rpc(
                11,
                "prompts/get",
                serde_json::json!({"name": "assess-impact", "arguments": {"id": "--x"}}),
            ),
        ],
    );
    let by = |n: u64| replies.iter().find(|r| r["id"] == n).unwrap();
    assert!(by(1)["result"]["capabilities"]["resources"].is_object());
    assert!(by(1)["result"]["capabilities"]["prompts"].is_object());
    let listed = by(2)["result"]["resources"].as_array().unwrap();
    assert_eq!(listed.len(), 4);
    assert!(listed.iter().any(|r| r["uri"] == uri.as_str()));
    assert!(by(2)["result"].get("nextCursor").is_none());
    assert_eq!(
        by(3)["result"]["resourceTemplates"][0]["uriTemplate"],
        "oxispec://node/{id}"
    );
    let text = by(4)["result"]["contents"][0]["text"].as_str().unwrap();
    assert!(text.contains("Third one"), "{text}");
    for n in [5, 6] {
        assert!(by(n).get("error").is_some(), "{}", by(n));
    }
    assert!(by(7).get("error").is_some());
    assert_eq!(by(8)["result"]["prompts"].as_array().unwrap().len(), 3);
    let message = by(9)["result"]["messages"][0]["content"]["text"]
        .as_str()
        .unwrap();
    assert!(
        message.contains(&id) && !message.contains("{id}"),
        "{message}"
    );
    assert!(by(10).get("error").is_some());
    assert!(by(11).get("error").is_some());
}

#[test]
fn a_running_mcp_server_sees_edits_made_after_it_cached_the_graph() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::Stdio;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);
    let id = created_id(&run(p, &["new", "requirement", "auth", "Original title"]));

    let mut child = Command::new(env!("CARGO_BIN_EXE_oxispec"))
        .arg("mcp")
        .current_dir(p)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut n = 0;
    let mut ask = |tool: &str, args: serde_json::Value| -> String {
        n += 1;
        let request = rpc(
            n,
            "tools/call",
            serde_json::json!({"name": tool, "arguments": args}),
        );
        writeln!(stdin, "{request}").unwrap();
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        let reply: serde_json::Value = serde_json::from_str(&line).unwrap();
        reply["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string()
    };

    // Files older than the two-second window are what the server keeps in memory.
    std::thread::sleep(std::time::Duration::from_millis(2300));
    assert!(ask("spec_status", serde_json::json!({})).contains("\"nodes\": 4"));
    assert!(ask("spec_get", serde_json::json!({"id": id})).contains("Original title"));
    assert!(ask("spec_get", serde_json::json!({"id": id})).contains("Original title"));
    // The second search reuses the open index and still answers the same.
    for _ in 0..2 {
        assert!(ask("spec_search", serde_json::json!({"query": "Original"})).contains(&id));
    }

    // An edit, and a new file, are seen once the system reports them, which takes
    // a few milliseconds: a call made in that gap still sees the old files.
    let shown: serde_json::Value =
        serde_json::from_str(&run(p, &["show", &id, "--json"]).stdout).unwrap();
    let path = p.join(shown["node"]["path"].as_str().unwrap());
    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, text.replace("Original title", "Edited title")).unwrap();
    assert_eq!(run(p, &["new", "adr", "Another"]).code, 0);
    let mut eventually = |tool: &str, args: serde_json::Value, wanted: &str| {
        for _ in 0..40 {
            if ask(tool, args.clone()).contains(wanted) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("{tool} never showed {wanted}");
    };
    eventually("spec_get", serde_json::json!({"id": id}), "Edited title");
    eventually("spec_status", serde_json::json!({}), "\"nodes\": 5");
    eventually("spec_search", serde_json::json!({"query": "Edited"}), &id);

    drop(stdin);
    assert!(child.wait().unwrap().success());
}

#[test]
fn mcp_resources_come_in_pages_that_cover_every_node_once() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    let folder = p.join("specs/requirements/bulk");
    fs::create_dir_all(&folder).unwrap();
    for n in 1..=530 {
        let id = format!("REQ-BULK-{n:04}");
        fs::write(
            folder.join(format!("{id}.md")),
            format!("---\nid: {id}\ntype: requirement\nstatus: draft\n---\n\n# {id}: Item {n}\n"),
        )
        .unwrap();
    }
    let replies = mcp(
        p,
        &[],
        &[
            rpc(1, "resources/list", serde_json::json!({})),
            rpc(2, "resources/list", serde_json::json!({"cursor": "500"})),
            rpc(3, "resources/list", serde_json::json!({"cursor": "5000"})),
        ],
    );
    let by = |n: u64| replies.iter().find(|r| r["id"] == n).unwrap();
    let first = by(1)["result"]["resources"].as_array().unwrap();
    assert_eq!(first.len(), 500);
    assert_eq!(by(1)["result"]["nextCursor"], "500");
    let second = by(2)["result"]["resources"].as_array().unwrap();
    assert_eq!(second.len(), 30);
    assert!(by(2)["result"].get("nextCursor").is_none());
    let mut uris: Vec<_> = first
        .iter()
        .chain(second)
        .map(|r| r["uri"].clone())
        .collect();
    uris.sort_by_key(|u| u.to_string());
    uris.dedup();
    assert_eq!(uris.len(), 530);
    // A cursor past the end is an empty last page, not an error.
    assert_eq!(by(3)["result"]["resources"].as_array().unwrap().len(), 0);
}

// ---- oxispec serve and oxispec token ----

struct HttpServer {
    child: std::process::Child,
    addr: String,
}

impl Drop for HttpServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_server(p: &Path, extra: &[&str]) -> HttpServer {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    let mut child = Command::new(env!("CARGO_BIN_EXE_oxispec"))
        .args(["serve", "--listen", "127.0.0.1:0"])
        .args(extra)
        .current_dir(p)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stderr.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let addr = line
        .split("http://")
        .nth(1)
        .and_then(|rest| rest.split("/mcp").next())
        .unwrap_or_else(|| panic!("no address in {line:?}"))
        .to_string();
    HttpServer { child, addr }
}

/// One HTTP/1.1 request. Returns the status and the body.
fn http(
    server: &HttpServer,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &str,
) -> (u16, String) {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(&server.addr).unwrap();
    let mut request = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
    if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("host")) {
        request += &format!("Host: {}\r\n", server.addr);
    }
    for (k, v) in headers {
        request += &format!("{k}: {v}\r\n");
    }
    request += &format!("Content-Length: {}\r\n\r\n{body}", body.len());
    stream.write_all(request.as_bytes()).unwrap();
    let mut reply = String::new();
    stream.read_to_string(&mut reply).unwrap();
    let status = reply.split(' ').nth(1).unwrap().parse().unwrap();
    let body = reply.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    (status, body)
}

/// A JSON-RPC call to `/mcp` with a token. Returns the status and the parsed answer.
fn mcp_http(
    server: &HttpServer,
    token: &str,
    method: &str,
    params: serde_json::Value,
) -> (u16, serde_json::Value) {
    let auth = format!("Bearer {token}");
    let (status, body) = http(
        server,
        "POST",
        "/mcp",
        &[
            ("Authorization", &auth),
            ("Content-Type", "application/json"),
        ],
        &rpc(1, method, params).to_string(),
    );
    (status, serde_json::from_str(&body).unwrap_or_default())
}

fn make_token(p: &Path, name: &str, types: Option<&str>) -> String {
    let mut args = vec!["token", "add", name];
    if let Some(types) = types {
        args.extend(["--types", types]);
    }
    let out = run(p, &args);
    assert_eq!(out.code, 0, "{}", out.stderr);
    out.stdout.trim().to_string()
}

#[test]
fn tokens_are_created_listed_and_revoked_without_keeping_the_secret() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);
    let secret = make_token(p, "ci", None);
    assert!(secret.starts_with("oxs_"));
    make_token(p, "docs", Some("adr,requirement"));
    assert_eq!(run(p, &["token", "add", "ci"]).code, 2, "names are unique");

    let list = run(p, &["token", "list"]).stdout;
    assert!(list.contains("ci  everything"), "{list}");
    assert!(list.contains("docs  only adr, requirement"), "{list}");
    assert!(!list.contains(&secret));

    let file = fs::read_to_string(p.join(".oxispec/server/tokens.json")).unwrap();
    assert!(!file.contains(&secret), "only a hash is kept");
    let ignore = fs::read_to_string(p.join(".oxispec/.gitignore")).unwrap();
    assert!(ignore.lines().any(|l| l == "server/"), "{ignore}");

    assert_eq!(run(p, &["token", "revoke", "ci"]).code, 0);
    assert_eq!(run(p, &["token", "revoke", "ci"]).code, 2);
    assert!(!run(p, &["token", "list"]).stdout.contains("ci  "));
}

#[test]
fn serve_refuses_to_start_without_tokens_or_on_an_open_address() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);
    let out = run(p, &["serve", "--listen", "127.0.0.1:0"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("no tokens"), "{}", out.stderr);

    make_token(p, "ci", None);
    let out = run(p, &["serve", "--listen", "0.0.0.0:0"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("--behind-tls-proxy"), "{}", out.stderr);
}

#[test]
fn the_http_server_needs_a_token_and_answers_the_protocol() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);
    let token = make_token(p, "ci", None);
    let server = start_server(p, &[]);

    assert_eq!(http(&server, "GET", "/health", &[], "").0, 200);
    assert_eq!(http(&server, "GET", "/elsewhere", &[], "").0, 404);
    assert_eq!(http(&server, "GET", "/mcp", &[], "").0, 405);
    let body = rpc(1, "ping", serde_json::json!({})).to_string();
    assert_eq!(http(&server, "POST", "/mcp", &[], &body).0, 401, "no token");
    let wrong = [("Authorization", "Bearer oxs_wrong")];
    assert_eq!(http(&server, "POST", "/mcp", &wrong, &body).0, 401);
    let rebinding = [
        ("Host", "evil.example"),
        ("Authorization", &format!("Bearer {token}")),
    ];
    assert_eq!(http(&server, "POST", "/mcp", &rebinding, &body).0, 403);

    let (status, reply) = mcp_http(&server, &token, "initialize", serde_json::json!({}));
    assert_eq!(status, 200);
    assert_eq!(reply["result"]["serverInfo"]["name"], "oxispec");

    let (_, tools) = mcp_http(&server, &token, "tools/list", serde_json::json!({}));
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"spec_check") && names.contains(&"spec_get"));
    assert!(
        !names.contains(&"spec_propose_change"),
        "the server is read-only"
    );

    let (_, got) = mcp_http(
        &server,
        &token,
        "tools/call",
        serde_json::json!({"name": "spec_get", "arguments": {"id": "REQ-AUTH-0001"}}),
    );
    assert!(
        got["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Admin session timeout")
    );

    // A notification gets no body, and a batch is refused.
    let auth = format!("Bearer {token}");
    let headers = [("Authorization", auth.as_str())];
    let note = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    assert_eq!(http(&server, "POST", "/mcp", &headers, note).0, 202);
    let (_, batch) = http(&server, "POST", "/mcp", &headers, "[]");
    assert!(batch.contains("batches are not supported"), "{batch}");
    let big = "x".repeat(1024 * 1024 + 10);
    assert_eq!(http(&server, "POST", "/mcp", &headers, &big).0, 413);
}

#[test]
fn a_limited_token_sees_only_its_types_and_nothing_that_reveals_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);
    assert_eq!(
        run(p, &["link", "REQ-AUTH-0001", "decided-by", "ADR-0001"]).code,
        0
    );
    let adr_only = make_token(p, "adr-only", Some("adr"));
    let req_only = make_token(p, "req-only", Some("requirement"));
    let server = start_server(p, &[]);
    let call = |token: &str, name: &str, args: serde_json::Value| -> (bool, String) {
        let (_, reply) = mcp_http(
            &server,
            token,
            "tools/call",
            serde_json::json!({"name": name, "arguments": args}),
        );
        match reply["result"]["content"][0]["text"].as_str() {
            Some(text) => (reply["result"]["isError"] == true, text.to_string()),
            None => (
                true,
                reply["error"]["message"].as_str().unwrap_or("").to_string(),
            ),
        }
    };

    // Only two tools are offered, and the others look like they do not exist.
    let (_, tools) = mcp_http(&server, &adr_only, "tools/list", serde_json::json!({}));
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["spec_search", "spec_get"]);
    let (failed, text) = call(&adr_only, "spec_check", serde_json::json!({}));
    assert!(failed && text.contains("unknown tool"), "{text}");
    let (failed, _) = call(&adr_only, "spec_status", serde_json::json!({}));
    assert!(failed);

    // Search is narrowed in the index: no hidden hits, and no counts of the whole index.
    let (_, hits) = call(
        &adr_only,
        "spec_search",
        serde_json::json!({"query": "session"}),
    );
    let hits: serde_json::Value = serde_json::from_str(&hits).unwrap();
    assert_eq!(hits["hits"].as_array().unwrap().len(), 0, "{hits}");
    assert!(hits.get("index").is_none(), "{hits}");
    let (_, hits) = call(
        &adr_only,
        "spec_search",
        serde_json::json!({"query": "Postgres"}),
    );
    assert!(hits.contains("ADR-0001"));
    // Asking for a type it may not see finds nothing, not an error that confirms the type exists.
    let (failed, hits) = call(
        &adr_only,
        "spec_search",
        serde_json::json!({"query": "session", "type": "requirement"}),
    );
    assert!(
        !failed && hits.contains("\"hits\": []") || hits.contains("\"hits\":[]"),
        "{hits}"
    );
    let (_, hits) = call(
        &req_only,
        "spec_search",
        serde_json::json!({"query": "Postgres"}),
    );
    assert!(!hits.contains("ADR-0001"), "{hits}");

    // A hidden node answers like one that does not exist.
    let (failed, text) = call(
        &adr_only,
        "spec_get",
        serde_json::json!({"id": "REQ-AUTH-0001"}),
    );
    let (_, missing) = call(
        &adr_only,
        "spec_get",
        serde_json::json!({"id": "REQ-NOPE-0001"}),
    );
    assert!(failed);
    assert_eq!(
        text.replace("REQ-AUTH-0001", "X"),
        missing.replace("REQ-NOPE-0001", "X")
    );

    // Links to hidden nodes show as stubs, in both directions.
    let (_, adr) = call(&adr_only, "spec_get", serde_json::json!({"id": "ADR-0001"}));
    assert!(adr.contains("[restricted node]"), "{adr}");
    assert!(
        !adr.contains("REQ-AUTH-0001") && !adr.contains("Admin session"),
        "{adr}"
    );
    let (_, req) = call(
        &req_only,
        "spec_get",
        serde_json::json!({"id": "REQ-AUTH-0001"}),
    );
    assert!(req.contains("[restricted node]"), "{req}");
    assert!(!req.contains("\"ADR-0001\""), "{req}");

    // Resources follow the same rule.
    let (_, listed) = mcp_http(&server, &adr_only, "resources/list", serde_json::json!({}));
    let uris: Vec<&str> = listed["result"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["uri"].as_str().unwrap())
        .collect();
    assert_eq!(uris, ["oxispec://node/ADR-0001"]);
    let (_, read) = mcp_http(
        &server,
        &adr_only,
        "resources/read",
        serde_json::json!({"uri": "oxispec://node/REQ-AUTH-0001"}),
    );
    assert!(
        read["error"]["message"]
            .as_str()
            .unwrap()
            .contains("no node with id"),
        "{read}"
    );
}

#[test]
fn a_revoked_token_stops_working_and_every_request_is_audited() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);
    let token = make_token(p, "ci", None);
    let limited = make_token(p, "docs", Some("adr"));
    let server = start_server(p, &[]);

    let ask = |token: &str, name: &str, args: serde_json::Value| {
        mcp_http(
            &server,
            token,
            "tools/call",
            serde_json::json!({"name": name, "arguments": args}),
        )
    };
    ask(
        &token,
        "spec_get",
        serde_json::json!({"id": "REQ-AUTH-0001"}),
    );
    ask(
        &token,
        "spec_search",
        serde_json::json!({"query": "secretword"}),
    );
    ask(&limited, "spec_check", serde_json::json!({}));
    assert_eq!(
        mcp_http(&server, "oxs_nope", "ping", serde_json::json!({})).0,
        401
    );

    assert_eq!(run(p, &["token", "revoke", "ci"]).code, 0);
    assert_eq!(
        mcp_http(&server, &token, "ping", serde_json::json!({})).0,
        401
    );
    assert_eq!(
        mcp_http(&server, &limited, "ping", serde_json::json!({})).0,
        200
    );

    let log = fs::read_to_string(p.join(".oxispec/server/audit.jsonl")).unwrap();
    let entries: Vec<serde_json::Value> = log
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let find = |principal: &str, tool: &str| {
        entries
            .iter()
            .find(|e| e["principal"] == principal && e["tool"] == tool)
            .unwrap_or_else(|| panic!("no entry for {principal} {tool}: {log}"))
            .clone()
    };
    assert_eq!(find("ci", "spec_get")["node"], "REQ-AUTH-0001");
    assert_eq!(find("ci", "spec_get")["outcome"], "ok");
    assert_eq!(find("docs", "spec_check")["outcome"], "denied");
    assert!(
        entries
            .iter()
            .any(|e| e["outcome"] == "unauthenticated" && e["principal"].is_null())
    );
    assert!(!log.contains("secretword"), "search words are not recorded");
    assert!(
        !log.contains(&token) && !log.contains(&limited),
        "no secrets in the log"
    );
}

#[test]
fn connect_can_point_clients_at_a_running_server_without_writing_a_token() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init"]).code, 0);
    let url = "https://specs.example.com/mcp";
    let out = run(p, &["connect", "claude", "copilot", "--url", url]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stderr.contains("OXISPEC_TOKEN"), "{}", out.stderr);

    let claude = fs::read_to_string(p.join(".mcp.json")).unwrap();
    assert!(claude.contains(url) && claude.contains("Bearer ${OXISPEC_TOKEN}"));
    assert!(!claude.contains("\"command\""));
    let vscode = fs::read_to_string(p.join(".vscode/mcp.json")).unwrap();
    assert!(vscode.contains("${input:oxispec-token}") && vscode.contains("promptString"));

    // Again changes nothing; a local entry can take its place.
    assert!(
        run(p, &["connect", "claude", "--url", url])
            .stdout
            .contains("unchanged")
    );
    assert_eq!(run(p, &["connect", "claude"]).code, 0);
    assert!(
        fs::read_to_string(p.join(".mcp.json"))
            .unwrap()
            .contains("\"command\"")
    );

    // A URL that carries a secret, or plain http to another machine, is not accepted quietly.
    let bad = run(p, &["connect", "claude", "--url", "https://u:pw@host/mcp"]);
    assert_eq!(bad.code, 2);
    assert!(bad.stderr.contains("user or password"), "{}", bad.stderr);
    let open = run(
        p,
        &["connect", "cursor", "--url", "http://specs.example.com/mcp"],
    );
    assert_eq!(open.code, 0);
    assert!(open.stderr.contains("unencrypted"), "{}", open.stderr);
    let both = run(p, &["connect", "--url", url, "--read-only"]);
    assert_eq!(both.code, 2);
}

const LIGHT_ADR: &str = "formats:\n  adr: light\nprofiles:\n  light:\n    type: adr\n    description: A short ADR with an owner\n    sections: [Context, Decision]\n    optional: [Consequences]\n    fields: [owner]\n    statuses: [proposed, accepted]\n";

fn project_with_light_adrs(p: &Path) {
    let out = run(p, &["init", "--preset", "standard"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let config = p.join(".oxispec/config.yaml");
    let text = fs::read_to_string(&config).unwrap();
    fs::write(&config, format!("{text}{LIGHT_ADR}")).unwrap();
}

#[test]
fn a_profile_shapes_new_and_check_enforces_it() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_light_adrs(p);

    let out = run(p, &["new", "adr", "Use Postgres"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let file = p.join("specs/adr/0001-use-postgres.md");
    let text = fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("owner: \n") && text.contains("## Consequences"),
        "{text}"
    );

    // The owner is still empty, so the profile is not met.
    let out = run(p, &["check", "--json"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(out.stdout.contains("\"E029\""), "{}", out.stdout);

    fs::write(&file, text.replace("owner: \n", "owner: ana\n")).unwrap();
    let out = run(p, &["check"]);
    assert_eq!(out.code, 0, "{}", out.stdout);

    // Taking a required section away is reported, with the node.
    fs::write(
        &file,
        fs::read_to_string(&file)
            .unwrap()
            .replace("## Decision\n", ""),
    )
    .unwrap();
    let out = run(p, &["check", "--json"]);
    assert_eq!(out.code, 1);
    assert!(out.stdout.contains("\"E028\"") && out.stdout.contains("ADR-0001"));
}

#[test]
fn formats_list_and_show_describe_the_profiles() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_light_adrs(p);

    let out = run(p, &["formats", "list"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("default") && out.stdout.contains("shall"));
    let light = out.stdout.lines().find(|l| l.starts_with("light")).unwrap();
    assert!(
        light.contains("project") && light.contains("(in use)"),
        "{light}"
    );

    let out = run(p, &["formats", "list", "--json"]);
    let value: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(value["schema_version"], 1);
    let chosen: Vec<_> = value["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["chosen"] == true)
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(chosen, ["light"]);

    let out = run(p, &["formats", "show", "light"]);
    assert!(
        out.stdout.contains("section Context (required)"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("statuses: proposed, accepted"));
    let out = run(p, &["formats", "show", "nope"]);
    assert_eq!(out.code, 2);
    assert!(
        out.stderr.contains("no profile named `nope`"),
        "{}",
        out.stderr
    );
}

#[test]
fn new_adr_can_pick_a_profile_and_refuses_one_for_another_type() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    let out = run(p, &["init", "--preset", "standard"]);
    assert_eq!(out.code, 0, "{}", out.stderr);

    // Without a profile in the config, `new adr` writes what it always has.
    let out = run(p, &["new", "adr", "Plain"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let plain = fs::read_to_string(p.join("specs/adr/0001-plain.md")).unwrap();
    assert!(plain.contains("status: proposed\n---\n# ADR-0001: Plain\n\n## Context"));

    let out = run(p, &["new", "adr", "Named", "--format", "default"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let out = run(p, &["new", "adr", "Wrong", "--format", "shall"]);
    assert_eq!(out.code, 2);
    assert!(
        out.stderr.contains("is for requirement nodes"),
        "{}",
        out.stderr
    );
    let out = run(p, &["new", "adr", "Missing", "--format", "nope"]);
    assert_eq!(out.code, 2);
}

#[test]
fn a_config_that_names_an_unknown_profile_fails_check_with_e031() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    let out = run(p, &["init", "--preset", "standard"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let config = p.join(".oxispec/config.yaml");
    let text = fs::read_to_string(&config).unwrap();
    fs::write(&config, format!("{text}formats:\n  adr: nope\n")).unwrap();
    let out = run(p, &["check"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(out.stdout.contains("E031") && out.stdout.contains(".oxispec/config.yaml"));
}

#[test]
fn a_running_mcp_server_sees_a_change_to_the_format_profiles() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::Stdio;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_two_requirements_and_an_adr(p);

    let mut child = Command::new(env!("CARGO_BIN_EXE_oxispec"))
        .arg("mcp")
        .current_dir(p)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut n = 0;
    let mut ask = |tool: &str| -> String {
        n += 1;
        let request = rpc(
            n,
            "tools/call",
            serde_json::json!({"name": tool, "arguments": {}}),
        );
        writeln!(stdin, "{request}").unwrap();
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        let reply: serde_json::Value = serde_json::from_str(&line).unwrap();
        reply["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string()
    };

    std::thread::sleep(std::time::Duration::from_millis(2300));
    assert!(!ask("spec_check").contains("E029"));
    assert!(!ask("spec_check").contains("E029"));

    // The config is under `.oxispec`, which the watcher otherwise skips.
    let config = p.join(".oxispec/config.yaml");
    let text = fs::read_to_string(&config).unwrap();
    fs::write(&config, format!("{text}{LIGHT_ADR}")).unwrap();
    let mut applied = false;
    for _ in 0..60 {
        if ask("spec_check").contains("E029") {
            applied = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    drop(stdin);
    assert!(child.wait().unwrap().success());
    assert!(applied, "the server never applied the new profile");
}

#[test]
fn the_adr_formats_write_their_templates_and_hold_new_records_to_them() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);

    for (n, (format, heading)) in [
        ("nygard", "## Consequences"),
        ("madr", "## Considered Options"),
        ("y-statement", "## Statement"),
    ]
    .into_iter()
    .enumerate()
    {
        let out = run(p, &["new", "adr", format, "--format", format]);
        assert_eq!(out.code, 0, "{}", out.stderr);
        let file = p.join(format!("specs/adr/{:04}-{format}.md", n + 1));
        assert!(
            fs::read_to_string(&file).unwrap().contains(heading),
            "{format}"
        );
    }
    let madr = fs::read_to_string(p.join("specs/adr/0002-madr.md")).unwrap();
    assert!(madr.contains("### Confirmation") && madr.contains("## Considered Options"));
    // The date is today's, in the same shape `change status` writes.
    let date = madr.lines().find_map(|l| l.strip_prefix("date: ")).unwrap();
    assert!(
        date.len() == 10 && date.as_bytes()[4] == b'-' && date.as_bytes()[7] == b'-',
        "{date}"
    );
    assert!(
        fs::read_to_string(p.join("specs/adr/0003-y-statement.md"))
            .unwrap()
            .contains("we decided for {option}")
    );

    // Not chosen in the config, so nothing holds these records to a format yet.
    assert_eq!(run(p, &["check"]).code, 0);

    let config = p.join(".oxispec/config.yaml");
    let text = fs::read_to_string(&config).unwrap();
    fs::write(&config, format!("{text}formats:\n  adr: madr\n")).unwrap();
    let out = run(p, &["check", "--json"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    // The Nygard record has no options or outcome, and the Y-statement has no context.
    assert!(out.stdout.contains("Considered Options"), "{}", out.stdout);

    let out = run(p, &["formats", "show", "madr"]);
    assert!(
        out.stdout.contains("section Decision Outcome (required)"),
        "{}",
        out.stdout
    );
    assert!(
        out.stdout
            .contains("statuses: proposed, accepted, rejected, deprecated, superseded")
    );
}

#[test]
fn a_superseded_decision_is_checked_against_its_successor() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    let config = p.join(".oxispec/config.yaml");
    let text = fs::read_to_string(&config).unwrap();
    fs::write(&config, format!("{text}formats:\n  adr: nygard\n")).unwrap();

    assert_eq!(run(p, &["new", "adr", "Use MySQL"]).code, 0);
    assert_eq!(run(p, &["new", "adr", "Use Postgres"]).code, 0);
    let first = p.join("specs/adr/0001-use-mysql.md");
    let second = p.join("specs/adr/0002-use-postgres.md");
    for file in [&first, &second] {
        let t = fs::read_to_string(file).unwrap();
        fs::write(file, t.replace("status: proposed", "status: accepted")).unwrap();
    }
    assert_eq!(run(p, &["check"]).code, 0);

    // The successor is in force, so the first record has to say it was superseded.
    assert_eq!(
        run(p, &["link", "ADR-0002", "supersedes", "ADR-0001"]).code,
        0
    );
    let out = run(p, &["check"]);
    assert_eq!(out.code, 0, "a warning does not fail: {}", out.stdout);
    assert!(
        out.stdout.contains("W012") && out.stdout.contains("ADR-0002"),
        "{}",
        out.stdout
    );
    assert_eq!(run(p, &["check", "--strict"]).code, 1);

    let t = fs::read_to_string(&first).unwrap();
    fs::write(&first, t.replace("status: accepted", "status: superseded")).unwrap();
    let out = run(p, &["check", "--strict"]);
    assert_eq!(out.code, 0, "{}", out.stdout);

    // Without a successor in force, `superseded` is a claim nothing backs.
    let t = fs::read_to_string(&second).unwrap();
    fs::write(&second, t.replace("status: accepted", "status: proposed")).unwrap();
    let out = run(p, &["check"]);
    assert!(out.stdout.contains("W011"), "{}", out.stdout);
}

const OLD_NYGARD: &str = "---\nid: ADR-0001\ntype: adr\nstatus: accepted\ndate: 2026-01-02\n---\n# ADR-0001: Use Postgres\n\n## Context\n\nWe need transactions.\n\n## Decision\n\nWe will use Postgres.\n\n## Consequences\n\nOps must run it.\n";

#[test]
fn adr_convert_rewrites_a_decision_and_a_second_run_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    fs::write(p.join("specs/adr/0001-use-postgres.md"), OLD_NYGARD).unwrap();

    let out = run(p, &["adr", "convert", "--to", "madr", "--dry-run"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("would convert"), "{}", out.stdout);
    assert_eq!(
        fs::read_to_string(p.join("specs/adr/0001-use-postgres.md")).unwrap(),
        OLD_NYGARD,
        "a dry run writes nothing"
    );

    let out = run(p, &["adr", "convert", "ADR-0001", "--to", "madr"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("Considered Options"), "{}", out.stdout);
    assert!(out.stdout.contains("formats:"), "tells how to choose it");
    let text = fs::read_to_string(p.join("specs/adr/0001-use-postgres.md")).unwrap();
    assert!(text.contains("## Context and Problem Statement\n\nWe need transactions."));
    assert!(text.contains("### Consequences\n\nOps must run it."));
    assert!(text.starts_with("---\nid: ADR-0001"));

    let out = run(p, &["adr", "convert", "--to", "madr", "--json"]);
    assert!(out.stdout.contains("\"changed\": false"), "{}", out.stdout);

    // And back: the text of the shared sections returns as it was.
    assert_eq!(run(p, &["adr", "convert", "--to", "nygard"]).code, 0);
    assert_eq!(
        fs::read_to_string(p.join("specs/adr/0001-use-postgres.md")).unwrap(),
        OLD_NYGARD
    );
}

#[test]
fn adr_convert_refuses_what_is_not_an_adr_or_a_format() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    fs::write(p.join("specs/adr/0001-use-postgres.md"), OLD_NYGARD).unwrap();
    assert_eq!(run(p, &["new", "requirement", "auth", "Login"]).code, 0);

    let out = run(p, &["adr", "convert", "--to", "nope"]);
    assert_eq!(out.code, 2, "{}", out.stderr);
    assert!(out.stderr.contains("no profile named `nope`"));
    let out = run(p, &["adr", "convert", "REQ-AUTH-0001", "--to", "madr"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("not an ADR"), "{}", out.stderr);
    let out = run(p, &["adr", "convert", "--to", "shall"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("not ADRs"), "{}", out.stderr);
}

const TOOLS_ONE: &str = "# 1. Record architecture decisions\n\nDate: 2016-02-12\n\n## Status\n\nSuperceded by [2. Use Rust](0002-use-rust.md)\n\n## Context\n\nWe need to record decisions.\n\n## Decision\n\nWe will record them.\n\n## Consequences\n\nSee the article.\n";
const TOOLS_TWO: &str = "# 2. Use Rust\n\nDate: 2016-03-01\n\n## Status\n\nAccepted\n\nSupercedes [1. Record architecture decisions](0001-record-architecture-decisions.md)\n\n## Context\n\nSpeed.\n\n## Decision\n\nRust.\n\n## Consequences\n\nSlower builds.\n";

#[test]
fn import_adr_brings_in_adr_tools_records_that_then_pass_check() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    let from = p.join("doc/adr");
    fs::create_dir_all(&from).unwrap();
    fs::write(
        from.join("0001-record-architecture-decisions.md"),
        TOOLS_ONE,
    )
    .unwrap();
    fs::write(from.join("0002-use-rust.md"), TOOLS_TWO).unwrap();
    fs::write(from.join("README.md"), "# Records\n").unwrap();

    let out = run(p, &["import", "adr", "--from", "adr-tools", "--dry-run"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("would import"), "{}", out.stdout);
    assert!(
        !p.join("specs/adr/0001-record-architecture-decisions.md")
            .exists()
    );

    let out = run(
        p,
        &["import", "adr", "--from", "adr-tools", "--format", "nygard"],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    let one =
        fs::read_to_string(p.join("specs/adr/0001-record-architecture-decisions.md")).unwrap();
    assert!(one.contains("status: superseded") && one.contains("date: 2016-02-12"));
    let two = fs::read_to_string(p.join("specs/adr/0002-use-rust.md")).unwrap();
    assert!(two.contains("supersedes: [ADR-0001]"), "{two}");

    // Chosen in the config, the imported records are held to the format and pass.
    let config = p.join(".oxispec/config.yaml");
    let text = fs::read_to_string(&config).unwrap();
    fs::write(&config, format!("{text}formats:\n  adr: nygard\n")).unwrap();
    let out = run(p, &["check"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);

    // The graph has the link, and a second import refuses to overwrite.
    let out = run(p, &["show", "ADR-0002", "--json"]);
    assert!(out.stdout.contains("ADR-0001"), "{}", out.stdout);
    let out = run(p, &["import", "adr", "--from", "adr-tools"]);
    assert_eq!(out.code, 2, "{}", out.stdout);
    assert!(out.stderr.contains("--offset"), "{}", out.stderr);
    let out = run(
        p,
        &["import", "adr", "--from", "adr-tools", "--offset", "10"],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(
        p.join("specs/adr/0011-record-architecture-decisions.md")
            .exists()
    );
}

#[test]
fn import_adr_reads_madr_and_log4brains_and_new_link_kinds_are_known() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    let madr = p.join("docs/decisions");
    fs::create_dir_all(&madr).unwrap();
    fs::write(
        madr.join("0001-use-markdown.md"),
        "---\nstatus: accepted\ndate: 2024-09-17\ndecision-makers: Ann, Bo\n---\n# Use Markdown\n\n## Context and Problem Statement\n\nWhy?\n\n## Considered Options\n\n* A\n* B\n\n## Decision Outcome\n\nChosen option: \"A\", because simple.\n",
    )
    .unwrap();
    let out = run(p, &["import", "adr", "--from", "madr", "--format", "madr"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let text = fs::read_to_string(p.join("specs/adr/0001-use-markdown.md")).unwrap();
    assert!(text.contains("decision-makers: [Ann, Bo]"), "{text}");

    let brains = p.join("docs/adr");
    fs::create_dir_all(&brains).unwrap();
    fs::write(
        brains.join("20200101-use-redis.md"),
        "# Use Redis\n\n- Status: accepted\n- Date: 2020-01-01\n- Tags: cache\n\n## Context and Problem Statement\n\nSlow reads.\n",
    )
    .unwrap();
    let out = run(
        p,
        &[
            "import",
            "adr",
            "--from",
            "log4brains",
            "--offset",
            "1",
            "--format",
            "madr",
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    let redis = fs::read_to_string(p.join("specs/adr/0002-use-redis.md")).unwrap();
    assert!(redis.contains("- Tags: cache"), "{redis}");

    // `amends` and `clarifies` are links like the others: no warning, and `link` takes them.
    let out = run(p, &["link", "ADR-0002", "amends", "ADR-0001"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let out = run(p, &["link", "ADR-0002", "clarifies", "ADR-0001"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let out = run(p, &["check", "--json"]);
    assert!(!out.stdout.contains("W00"), "{}", out.stdout);

    let out = run(p, &["import", "adr", "--from", "nope"]);
    assert_eq!(out.code, 2);
    assert!(
        out.stderr.contains("adr-tools, madr, log4brains"),
        "{}",
        out.stderr
    );
}

#[test]
fn debt_is_created_listed_by_severity_and_moved_along() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    assert_eq!(run(p, &["new", "requirement", "api", "Retries"]).code, 0);

    let out = run(
        p,
        &[
            "new",
            "debt",
            "No backoff",
            "--severity",
            "high",
            "--from",
            "REQ-API-0001",
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("DEBT-0001"), "{}", out.stdout);
    assert_eq!(
        run(p, &["new", "debt", "Old script", "--severity", "low"]).code,
        0
    );
    let text = fs::read_to_string(p.join("specs/debt/0001-no-backoff.md")).unwrap();
    assert!(text.contains("severity: high") && text.contains("arises-from: REQ-API-0001"));

    // A new item has no owner, so check says so; the one without a source says that too.
    let out = run(p, &["check", "--json"]);
    assert!(out.stdout.contains("W013"), "{}", out.stdout);
    assert!(out.stdout.contains("W016"), "{}", out.stdout);

    let out = run(p, &["debt"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let high = out.stdout.find("DEBT-0001").unwrap();
    let low = out.stdout.find("DEBT-0002").unwrap();
    assert!(
        high < low,
        "the more serious item comes first:\n{}",
        out.stdout
    );
    assert!(
        out.stdout.contains("2 still to be paid: 1 high, 1 low"),
        "{}",
        out.stdout
    );

    let out = run(p, &["debt", "status", "DEBT-0001", "resolved"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    let text = fs::read_to_string(p.join("specs/debt/0001-no-backoff.md")).unwrap();
    assert!(text.contains("status: resolved"));
    assert!(!text.contains("date: 20") || text.contains(&format!("date: {}", today_utc())));
    let out = run(p, &["debt"]);
    assert!(
        !out.stdout.contains("DEBT-0001"),
        "resolved items are left out"
    );
    let out = run(p, &["debt", "--all", "--json"]);
    assert!(out.stdout.contains("DEBT-0001") && out.stdout.contains("\"live\": false"));
    let out = run(p, &["debt", "--status", "resolved"]);
    assert!(out.stdout.contains("DEBT-0001"), "{}", out.stdout);
}

fn today_utc() -> String {
    let out = Command::new("date")
        .args(["-u", "+%Y-%m-%d"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn debt_that_is_late_is_reported_and_bad_input_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    fs::write(
        p.join("specs/debt/0001-late.md"),
        "---\nid: DEBT-0001\ntype: debt\nstatus: open\nseverity: critical\nowner: ann\ndate: 2000-01-01\ndue: 2000-06-01\nlinks:\n  arises-from: ADR-0001\n---\n# DEBT-0001: Late\n",
    )
    .unwrap();
    let out = run(p, &["debt"]);
    assert!(
        out.stdout.contains("[overdue]") && out.stdout.contains("[stale]"),
        "{}",
        out.stdout
    );
    let out = run(p, &["check", "--json"]);
    assert!(
        out.stdout.contains("W014") && out.stdout.contains("W015"),
        "{}",
        out.stdout
    );

    for args in [
        vec!["new", "debt", "X", "--severity", "huge"],
        vec!["new", "debt", "X", "--from", "REQ-NOPE-0001"],
        vec!["debt", "--status", "later"],
        vec!["debt", "status", "DEBT-0001", "later"],
        vec!["debt", "status", "DEBT-0009", "open"],
    ] {
        let out = run(p, &args);
        assert_eq!(out.code, 2, "{args:?}: {}{}", out.stdout, out.stderr);
    }
}

#[test]
fn a_release_can_be_held_to_a_limit_on_debt() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q"]);
    assert_eq!(run(p, &["init", "--preset", "full"]).code, 0);
    assert_eq!(
        run(p, &["new", "debt", "Hack", "--severity", "critical"]).code,
        0
    );
    let config = p.join(".oxispec/config.yaml");
    let text = fs::read_to_string(&config).unwrap();
    fs::write(
        &config,
        format!("{text}debt:\n  limits:\n    critical: 0\n"),
    )
    .unwrap();
    commit_all(p, "debt");

    let out = run(p, &["debt"]);
    assert!(
        out.stdout.contains("over the limit: 1 critical"),
        "{}",
        out.stdout
    );
    let out = run(p, &["debt", "--json"]);
    assert!(out.stdout.contains("\"over_limits\""), "{}", out.stdout);
    assert!(out.stdout.contains("critical"), "{}", out.stdout);

    // The release gate sees it too.
    let out = run(p, &["release", "prepare", "0.1.0"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let out = run(p, &["release", "check", "0.1.0"]);
    assert!(out.stdout.contains("debt-within-limits"), "{}", out.stdout);
    assert_ne!(out.code, 0, "{}", out.stdout);

    assert_eq!(run(p, &["debt", "status", "DEBT-0001", "accepted"]).code, 0);
    let out = run(p, &["release", "check", "0.1.0"]);
    assert!(out.stdout.contains("debt-within-limits"), "{}", out.stdout);
    assert!(
        !out.stdout.contains("1 critical debt items"),
        "{}",
        out.stdout
    );
}

#[test]
fn debt_needs_its_module_and_an_mcp_client_can_read_the_backlog() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(
        run(p, &["init", "--preset", "minimal", "--with", "mcp"]).code,
        0
    );
    let out = run(p, &["debt"]);
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("module `debt`"), "{}", out.stderr);
    let names = debt_tool_names(p);
    assert!(!names.contains(&"spec_debt".to_string()));

    assert_eq!(run(p, &["modules", "add", "debt"]).code, 0);
    assert_eq!(
        run(p, &["new", "debt", "Hack", "--severity", "high"]).code,
        0
    );
    assert!(debt_tool_names(p).contains(&"spec_debt".to_string()));
    let (is_error, text) = call_tool(p, "spec_debt", serde_json::json!({"severity": "high"}));
    assert!(!is_error, "{text}");
    assert!(text.contains("DEBT-0001"), "{text}");
    let (is_error, _) = call_tool(p, "spec_debt", serde_json::json!({"severity": "Huge!"}));
    assert!(is_error);
}

fn debt_tool_names(p: &Path) -> Vec<String> {
    tool_names(&mcp(
        p,
        &[],
        &[
            rpc(
                1,
                "initialize",
                serde_json::json!({"protocolVersion": "2025-06-18"}),
            ),
            rpc(2, "tools/list", serde_json::json!({})),
        ],
    ))
}

fn choose_requirement_style(p: &Path, style: &str) {
    let config = p.join(".oxispec/config.yaml");
    let text = fs::read_to_string(&config).unwrap();
    fs::write(&config, format!("{text}formats:\n  requirement: {style}\n")).unwrap();
}

#[test]
fn each_requirement_style_writes_a_template_that_passes_its_own_check() {
    for style in ["shall", "ears", "gherkin", "user-story"] {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        assert_eq!(run(p, &["init", "--preset", "minimal"]).code, 0);
        choose_requirement_style(p, style);
        let out = run(p, &["new", "requirement", "auth", "Login"]);
        assert_eq!(out.code, 0, "{style}: {}", out.stderr);
        let out = run(p, &["check"]);
        assert_eq!(out.code, 0, "{style}: {}{}", out.stdout, out.stderr);
        let text = fs::read_to_string(p.join("specs/requirements/auth/login.md")).unwrap();
        assert!(text.contains("### Requirement: Login"), "{style}");
        match style {
            "ears" => assert!(
                text.contains("When <trigger>, the <system> shall"),
                "{text}"
            ),
            "user-story" => assert!(text.contains("As a <role>, I want <goal>"), "{text}"),
            "gherkin" => assert!(text.contains("**GIVEN**"), "{text}"),
            _ => assert!(text.contains("The system SHALL"), "{text}"),
        }
        let out = run(p, &["formats", "show", style]);
        assert_eq!(out.code, 0, "{}", out.stderr);
    }
}

#[test]
fn format_on_new_requirement_overrides_the_config_and_refuses_other_types() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    let out = run(
        p,
        &[
            "new",
            "requirement",
            "auth",
            "Login",
            "--format",
            "user-story",
        ],
    );
    assert_eq!(out.code, 0, "{}", out.stderr);
    let text = fs::read_to_string(p.join("specs/requirements/auth/login.md")).unwrap();
    assert!(text.contains("As a <role>"));
    for bad in ["nygard", "nope"] {
        let out = run(p, &["new", "requirement", "auth", "Other", "--format", bad]);
        assert_eq!(out.code, 2, "{bad}");
    }
}

fn requirement_file(p: &Path, name: &str, body: &str) {
    let dir = p.join("specs/requirements/auth");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(format!("{name}.md")),
        format!(
            "---\nid: REQ-AUTH-{}\ntype: requirement\nstatus: draft\n---\n{body}",
            name.to_uppercase()
        ),
    )
    .unwrap();
}

#[test]
fn ears_holds_a_requirement_to_one_of_its_patterns() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "minimal"]).code, 0);
    requirement_file(
        p,
        "0001",
        "### Requirement: Lock\nIf three passwords fail, then the system shall lock the account.\n\n#### Scenario: Locked {#S1}\n- **WHEN** x\n- **THEN** y\n",
    );
    requirement_file(
        p,
        "0002",
        "### Requirement: Loose\nWhen a user fails to sign in three times the system SHALL lock the account.\n\n#### Scenario: Locked {#S1}\n- **WHEN** x\n- **THEN** y\n",
    );
    // Nothing is chosen yet: only SHALL in capitals counts, as it always did.
    let out = run(p, &["check", "--json"]);
    assert_eq!(out.stdout.matches("E007").count(), 1, "{}", out.stdout);
    assert!(!out.stdout.contains("E033"), "{}", out.stdout);
    choose_requirement_style(p, "ears");
    let out = run(p, &["check", "--json"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert_eq!(out.stdout.matches("E033").count(), 1, "{}", out.stdout);
    assert!(!out.stdout.contains("E007"), "EARS checks its own `shall`");
    assert!(out.stdout.contains("needs a comma"), "{}", out.stdout);
    assert!(out.stdout.contains("REQ-AUTH-0002"), "{}", out.stdout);
}

#[test]
fn a_user_story_needs_no_shall_but_still_needs_acceptance_criteria() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "minimal"]).code, 0);
    requirement_file(
        p,
        "0001",
        "### Requirement: Export\nAs a user, I want to export my data, so that I can leave.\n\n#### Scenario: Exported {#S1}\n- **WHEN** x\n- **THEN** y\n",
    );
    requirement_file(
        p,
        "0002",
        "### Requirement: Import\nAs a user, I want to import my data.\n",
    );
    // As a SHALL requirement the first has no SHALL.
    let out = run(p, &["check", "--json"]);
    assert!(out.stdout.contains("E007"), "{}", out.stdout);
    choose_requirement_style(p, "user-story");
    let out = run(p, &["check", "--json"]);
    assert!(!out.stdout.contains("E007"), "{}", out.stdout);
    assert_eq!(out.stdout.matches("E033").count(), 1, "{}", out.stdout);
    assert!(out.stdout.contains("so that"), "{}", out.stdout);
    assert!(
        out.stdout.contains("E006"),
        "no acceptance criterion:\n{}",
        out.stdout
    );
}

#[test]
fn gherkin_scenarios_in_a_fence_are_covered_and_checked() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "specs-tests"]).code, 0);
    requirement_file(
        p,
        "0001",
        "### Requirement: Login\nThe system SHALL sign a user in.\n\n```gherkin\nScenario: Right password {#S1}\n  Given an account\n  When the password is right\n  Then the user is signed in\nScenario: Wrong password {#S2}\n  Given an account\n  Then nothing happens\n```\n",
    );
    choose_requirement_style(p, "gherkin");
    let out = run(p, &["check", "--json"]);
    // The fence gives two scenarios (no E006), and the second lacks a WHEN.
    assert!(!out.stdout.contains("E006"), "{}", out.stdout);
    assert_eq!(out.stdout.matches("E034").count(), 1, "{}", out.stdout);
    assert!(out.stdout.contains("Wrong password"), "{}", out.stdout);
    // A test can link to the anchor of a scenario in the fence.
    fs::create_dir_all(p.join("specs/tests")).unwrap();
    fs::write(
        p.join("specs/tests/login.md"),
        "---\nid: TC-AUTH-0001\ntype: test-case\nlinks:\n  verifies: REQ-AUTH-0001.S1\n---\n# TC-AUTH-0001: Login works\n",
    )
    .unwrap();
    let out = run(p, &["check", "--json"]);
    assert!(!out.stdout.contains("E009"), "{}", out.stdout);
}

#[test]
fn a_custom_profile_can_have_a_style_and_a_bad_style_is_e031() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    assert_eq!(run(p, &["init", "--preset", "minimal"]).code, 0);
    let config = p.join(".oxispec/config.yaml");
    let text = fs::read_to_string(&config).unwrap();
    fs::write(
        &config,
        format!("{text}formats:\n  requirement: strict\nprofiles:\n  strict:\n    type: requirement\n    style: ears\n    sections: [Rationale]\n  odd:\n    type: requirement\n    style: haiku\n"),
    )
    .unwrap();
    let out = run(p, &["check", "--json"]);
    assert!(
        out.stdout.contains("E031") && out.stdout.contains("haiku"),
        "{}",
        out.stdout
    );
    assert_eq!(run(p, &["new", "requirement", "auth", "Login"]).code, 0);
    let text = fs::read_to_string(p.join("specs/requirements/auth/login.md")).unwrap();
    assert!(
        text.contains("## Rationale") && text.contains("When <trigger>"),
        "{text}"
    );
}

const OPENSPEC_AUTH: &str = "# Auth Specification\n\n## Purpose\nLet people in.\n\n## Requirements\n\n### Requirement: Login\nThe system SHALL sign users in.\n\n#### Scenario: Good password\n- **WHEN** a user gives the right password\n- **THEN** a session starts\n";

fn import_project(p: &Path) {
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
}

#[test]
fn import_specs_brings_in_openspec_and_lists_what_it_leaves_out() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    import_project(p);
    let cap = p.join("openspec/specs/auth");
    fs::create_dir_all(&cap).unwrap();
    fs::write(cap.join("spec.md"), OPENSPEC_AUTH).unwrap();
    fs::write(cap.join("design.md"), "# Design\n").unwrap();
    fs::create_dir_all(p.join("openspec/changes/add-2fa")).unwrap();
    fs::create_dir_all(p.join("openspec/changes/archive")).unwrap();

    let out = run(p, &["import", "specs", "--from", "openspec", "--dry-run"]);
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert!(out.stdout.contains("would import"), "{}", out.stdout);
    assert!(!p.join("specs/requirements/auth/auth.md").exists());

    let out = run(p, &["import", "specs", "--from", "openspec"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("REQ-AUTH-0001"), "{}", out.stdout);
    assert!(out.stdout.contains("design.md"), "{}", out.stdout);
    assert!(out.stdout.contains("add-2fa"), "{}", out.stdout);
    let text = fs::read_to_string(p.join("specs/requirements/auth/auth.md")).unwrap();
    assert!(text.contains("### Requirement: Login"), "{text}");
    assert!(
        text.contains("#### Scenario: Good password {#S1}"),
        "{text}"
    );
    assert!(text.contains("## Purpose"), "{text}");
    let check = run(p, &["check"]);
    assert_eq!(check.code, 0, "{}{}", check.stdout, check.stderr);

    // The folder is still there, so a second import clashes and writes nothing more.
    let again = run(p, &["import", "specs", "--from", "openspec"]);
    assert_ne!(again.code, 0);
}

#[test]
fn import_specs_reads_kiro_and_spec_kit_and_marks_what_needs_filling_in() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    import_project(p);
    let kiro = p.join(".kiro/specs/login");
    fs::create_dir_all(&kiro).unwrap();
    fs::write(
        kiro.join("requirements.md"),
        "## Requirements\n\n### Requirement 1\n\n**User Story:** As a user, I want to log in, so that I can work.\n\n#### Acceptance Criteria\n\n1. WHEN a user gives a password THEN the system SHALL start a session\n",
    )
    .unwrap();
    fs::write(kiro.join("tasks.md"), "- [ ] build it\n").unwrap();
    let out = run(p, &["import", "specs", "--from", "kiro"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("REQ-LOGIN-0001"), "{}", out.stdout);
    assert!(out.stdout.contains("tasks.md"), "{}", out.stdout);

    let kit = p.join("kit/001-search");
    fs::create_dir_all(&kit).unwrap();
    fs::write(
        kit.join("spec.md"),
        "## Requirements\n\n- **FR-001**: System MUST rank by title.\n",
    )
    .unwrap();
    let out = run(
        p,
        &[
            "import", "specs", "--from", "speckit", "--dir", "kit", "--json",
        ],
    );
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let v: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(v["nodes"][0]["id"], "REQ-SEARCH-0001");
    assert!(
        v["nodes"][0]["notes"].to_string().contains("placeholder"),
        "{v}"
    );
    let text = fs::read_to_string(p.join("specs/requirements/search/001-search.md")).unwrap();
    assert!(text.contains("<outcome>"), "{text}");
}

#[test]
fn import_specs_takes_plain_markdown_and_refuses_unknown_sources() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    import_project(p);
    fs::write(
        p.join("notes.md"),
        "# Notes\n\n## Retry\nFailed jobs are retried.\n",
    )
    .unwrap();
    let out = run(p, &["import", "specs", "--from", "markdown"]);
    assert_ne!(out.code, 0);
    assert!(out.stderr.contains("--dir"), "{}", out.stderr);
    let out = run(
        p,
        &["import", "specs", "--from", "markdown", "--dir", "notes.md"],
    );
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let text = fs::read_to_string(p.join("specs/requirements/notes/notes.md")).unwrap();
    assert!(
        text.contains("The system SHALL satisfy this requirement: Failed jobs are retried."),
        "{text}"
    );
    let out = run(p, &["import", "specs", "--from", "word"]);
    assert_ne!(out.code, 0);
    assert!(out.stderr.contains("unknown source"), "{}", out.stderr);
}

#[test]
fn import_specs_reads_a_prd_and_an_openapi_description() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    import_project(p);
    fs::write(
        p.join("prd.md"),
        "# Checkout\n\n## Overview\nLet shoppers pay.\n\n## Functional requirements\n- FR-1: Shoppers can pay by card\n  - WHEN the card is valid THEN the order is placed\n",
    )
    .unwrap();
    let out = run(p, &["import", "specs", "--from", "prd", "--dir", "prd.md"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let text = fs::read_to_string(p.join("specs/requirements/prd/prd.md")).unwrap();
    assert!(
        text.contains("### Requirement: FR-1 Shoppers can pay by card"),
        "{text}"
    );
    assert!(text.contains("**THEN** the order is placed"), "{text}");

    fs::write(
        p.join("api.json"),
        "{\"openapi\":\"3.0.0\",\"info\":{\"title\":\"Shop\"},\"paths\":{\"/orders\":{\"post\":{\"tags\":[\"orders\"],\"summary\":\"Place an order\",\"responses\":{\"201\":{\"description\":\"Created\"}}}}}}",
    )
    .unwrap();
    let out = run(
        p,
        &["import", "specs", "--from", "openapi", "--dir", "api.json"],
    );
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let text = fs::read_to_string(p.join("specs/requirements/orders/orders.md")).unwrap();
    assert!(text.contains("### Requirement: POST /orders"), "{text}");
    assert!(text.contains("responds with 201: Created"), "{text}");
    let out = run(
        p,
        &[
            "import", "plan", "start", "--from", "openapi", "--dir", "api.json",
        ],
    );
    assert_ne!(out.code, 0);
    assert!(out.stderr.contains("no plan"), "{}", out.stderr);
    assert_eq!(run(p, &["check"]).code, 0);
}

#[test]
fn a_prd_is_written_as_nodes_and_read_as_a_document() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q"]);
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    let out = run(p, &["new", "prd", "Checkout"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("PRD-0001"), "{}", out.stdout);
    assert_eq!(
        run(p, &["new", "requirement", "pay", "Pay by card"]).code,
        0
    );
    let out = run(p, &["link", "REQ-PAY-0001", "refines", "PRD-0001"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert_eq!(run(p, &["check"]).code, 0);
    let out = run(p, &["docs", "prd", "PRD-0001", "--out", "prd.md"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let doc = fs::read_to_string(p.join("prd.md")).unwrap();
    assert!(doc.contains("## Non-goals"), "{doc}");
    assert!(doc.contains("### REQ-PAY-0001: "), "{doc}");
    let out = run(p, &["docs", "prd", "REQ-PAY-0001"]);
    assert_ne!(out.code, 0);
    assert!(out.stderr.contains("not a prd"), "{}", out.stderr);
}

#[test]
fn openapi_is_made_from_requirements_and_a_file_is_compared_with_them() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    import_project(p);
    fs::write(
        p.join("api.json"),
        "{\"openapi\":\"3.0.0\",\"info\":{\"title\":\"Shop\"},\"paths\":{\"/orders/{id}\":{\"get\":{\"tags\":[\"orders\"],\"summary\":\"Fetch an order\",\"responses\":{\"200\":{\"description\":\"The order\"},\"404\":{\"description\":\"No such order\"}}}}}}",
    )
    .unwrap();
    let out = run(
        p,
        &["import", "specs", "--from", "openapi", "--dir", "api.json"],
    );
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let out = run(
        p,
        &["docs", "openapi", "--title", "Shop", "--out", "made.yaml"],
    );
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let made = fs::read_to_string(p.join("made.yaml")).unwrap();
    assert!(made.contains("\"/orders/{id}\":"), "{made}");
    assert!(made.contains("\"404\":"), "{made}");
    let out = run(p, &["docs", "openapi", "--against", "api.json"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout.contains("agree on 1 operation"),
        "{}",
        out.stdout
    );
    fs::write(
        p.join("other.json"),
        "{\"openapi\":\"3.0.0\",\"paths\":{\"/health\":{\"get\":{\"responses\":{\"200\":{}}}}}}",
    )
    .unwrap();
    let out = run(p, &["docs", "openapi", "--against", "other.json"]);
    assert_eq!(out.code, 1, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stdout
            .contains("only in the requirements: GET /orders/{id}"),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("GET /health"), "{}", out.stdout);
}

#[test]
fn risks_are_registered_listed_and_accepted_by_a_person() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q"]);
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    let out = run(
        p,
        &[
            "new",
            "risk",
            "Tokens never expire",
            "--likelihood",
            "high",
            "--impact",
            "high",
        ],
    );
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("RISK-0001"), "{}", out.stdout);
    let out = run(p, &["risks"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("critical"), "{}", out.stdout);
    assert!(
        out.stdout.contains("holds a release back"),
        "{}",
        out.stdout
    );
    let out = run(p, &["risks", "--json"]);
    assert!(
        out.stdout.contains("\"level\": \"critical\""),
        "{}",
        out.stdout
    );
    let out = run(
        p,
        &["risks", "status", "RISK-0001", "accepted", "--by", "Dana"],
    );
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let text = fs::read_to_string(
        p.join("specs/risks").join(
            fs::read_dir(p.join("specs/risks"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .file_name(),
        ),
    )
    .unwrap();
    assert!(text.contains("approved_by: \"Dana\""), "{text}");
    let out = run(p, &["risks"]);
    assert!(out.stdout.contains("no risks"), "{}", out.stdout);
    let out = run(p, &["risks", "status", "RISK-0001", "later"]);
    assert_ne!(out.code, 0);
    let out = run(p, &["new", "risk", "x", "--impact", "huge"]);
    assert_ne!(out.code, 0);
}

#[test]
fn a_test_plan_is_read_against_the_tests() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q"]);
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    assert_eq!(
        run(p, &["new", "requirement", "pay", "Pay by card"]).code,
        0
    );
    let out = run(p, &["new", "test-plan", "Checkout"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("TP-0001"), "{}", out.stdout);
    let out = run(p, &["link", "TP-0001", "covers", "REQ-PAY-0001"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let out = run(p, &["test-plan"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("REQ-PAY-0001"), "{}", out.stdout);
    assert!(
        out.stdout.contains("not verified: REQ-PAY-0001.S1"),
        "{}",
        out.stdout
    );
    // A draft plan has no gaps that fail a strict run; an approved one does.
    assert_eq!(run(p, &["test-plan", "--strict"]).code, 0);
    let file = fs::read_dir(p.join("specs/test-plans"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = fs::read_to_string(&file).unwrap();
    fs::write(&file, text.replace("status: draft", "status: approved")).unwrap();
    assert_eq!(run(p, &["test-plan", "--strict"]).code, 1);
    let out = run(p, &["test-plan", "--json"]);
    assert!(out.stdout.contains("\"gaps\": true"), "{}", out.stdout);
    let out = run(p, &["test-plan", "REQ-PAY-0001"]);
    assert_ne!(out.code, 0);
}

// verifies: REQ-API-0005.S1
// verifies: REQ-API-0005.S2
#[test]
fn an_operation_card_and_a_running_service_are_read() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    git(p, &["init", "-q"]);
    assert_eq!(run(p, &["init", "--preset", "standard"]).code, 0);
    let api = "{\"openapi\":\"3.0.0\",\"info\":{\"title\":\"Shop\"},\"paths\":{\"/orders/{id}\":{\"get\":{\"tags\":[\"orders\"],\"summary\":\"Fetch an order\",\"responses\":{\"200\":{\"description\":\"The order\"},\"404\":{\"description\":\"No such order\"}}}}}}";
    fs::write(p.join("api.json"), api).unwrap();
    let out = run(
        p,
        &["import", "specs", "--from", "openapi", "--dir", "api.json"],
    );
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert_eq!(run(p, &["new", "prd", "Shop"]).code, 0);
    assert_eq!(
        run(p, &["link", "REQ-ORDERS-0001", "refines", "PRD-0001"]).code,
        0
    );
    assert_eq!(
        run(
            p,
            &[
                "new",
                "risk",
                "Order ids can be guessed",
                "--from",
                "REQ-ORDERS-0001"
            ]
        )
        .code,
        0
    );
    let out = run(p, &["api", "show", "GET /orders/{id}"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    for want in [
        "PRD: PRD-0001",
        "risks: RISK-0001",
        "responds 404: No such order",
        "REQ-ORDERS-0001.S1: no test",
    ] {
        assert!(out.stdout.contains(want), "{want} in {}", out.stdout);
    }
    let out = run(p, &["api", "show", "REQ-ORDERS-0001", "--json"]);
    assert!(
        out.stdout.contains("\"call\": \"GET /orders/{id}\""),
        "{}",
        out.stdout
    );
    let out = run(p, &["api", "show", "DELETE /nothing"]);
    assert_ne!(out.code, 0);
    assert!(
        out.stderr.contains("no operation matches"),
        "{}",
        out.stderr
    );

    // A running service, with the body sent in chunks as some frameworks do.
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let port = server.server_addr().to_ip().unwrap().port();
    let handle = std::thread::spawn(move || {
        for request in server.incoming_requests().take(3) {
            let response = match request.url() {
                "/docs" => tiny_http::Response::from_string(api).boxed(),
                "/chunked" => tiny_http::Response::new(
                    tiny_http::StatusCode(200),
                    vec![],
                    std::io::Cursor::new(api.as_bytes().to_vec()),
                    None,
                    None,
                )
                .boxed(),
                _ => tiny_http::Response::from_string("no")
                    .with_status_code(404)
                    .boxed(),
            };
            let _ = request.respond(response);
        }
    });
    for path in ["docs", "chunked"] {
        let out = run(
            p,
            &[
                "docs",
                "openapi",
                "--against",
                &format!("http://127.0.0.1:{port}/{path}"),
            ],
        );
        assert_eq!(out.code, 0, "{path}: {}{}", out.stdout, out.stderr);
        assert!(
            out.stdout.contains("agree on 1 operation"),
            "{}",
            out.stdout
        );
    }
    let out = run(
        p,
        &[
            "docs",
            "openapi",
            "--against",
            &format!("http://127.0.0.1:{port}/missing"),
        ],
    );
    assert_ne!(out.code, 0);
    assert!(out.stderr.contains("404"), "{}", out.stderr);
    handle.join().unwrap();
    let out = run(
        p,
        &["docs", "openapi", "--against", "https://example.com/docs"],
    );
    assert_ne!(out.code, 0);
    assert!(
        out.stderr.contains("https is not supported"),
        "{}",
        out.stderr
    );
}

fn project_with_a_standard(p: &Path, standard: &str) {
    git(p, &["init", "-q"]);
    assert_eq!(run(p, &["init", "--preset", "full"]).code, 0);
    assert_eq!(
        run(p, &["new", "requirement", "dose", "Limit the dose"]).code,
        0
    );
    let file = p.join("specs/requirements/dose/limit-the-dose.md");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(&file, text.replace("status: draft", "status: accepted")).unwrap();
    let config = p.join(".oxispec/config.yaml");
    let text = fs::read_to_string(&config).unwrap();
    fs::write(&config, format!("{text}compliance:\n{standard}")).unwrap();
}

#[test]
fn a_standard_asks_for_tests_and_sign_offs_until_they_exist() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_a_standard(p, "  standard: iec-62304\n  level: A\n");

    let out = run(p, &["check"]);
    assert_ne!(out.code, 0, "{}", out.stdout);
    assert!(out.stdout.contains("E035"), "{}", out.stdout);
    assert!(out.stdout.contains("E036"), "{}", out.stdout);
    let out = run(p, &["compliance"]);
    assert_eq!(out.code, 1, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("GAP"), "{}", out.stdout);
    assert!(out.stdout.contains("REQ-DOSE-0001"), "{}", out.stdout);

    // A test case that verifies a scenario, and a sign-off, meet the level.
    fs::create_dir_all(p.join("specs/tests")).unwrap();
    fs::write(
        p.join("specs/tests/dose.md"),
        "---\nid: TC-DOSE-0001\ntype: test-case\nlinks:\n  verifies:\n    - REQ-DOSE-0001.S1\n---\n# Dose test\n",
    )
    .unwrap();
    let file = p.join("specs/requirements/dose/limit-the-dose.md");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        text.replace(
            "status: accepted\n",
            "status: accepted\napproved_by: Ann Lee\napproved_on: \"2026-10-28\"\n",
        ),
    )
    .unwrap();
    let out = run(p, &["compliance", "--json"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("\"met\": true"), "{}", out.stdout);
    let out = run(p, &["check"]);
    assert!(
        !out.stdout.contains("E035") && !out.stdout.contains("E036"),
        "{}",
        out.stdout
    );
}

#[test]
fn compliance_lists_standards_and_a_higher_level_asks_for_more() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    let out = run(p, &["compliance", "standards"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    for name in ["iec-62304", "iso-26262", "do-178c"] {
        assert!(out.stdout.contains(name), "{}", out.stdout);
    }

    project_with_a_standard(p, "  standard: do-178c\n  level: A\n");
    let out = run(p, &["compliance", "--json"]);
    assert_eq!(out.code, 1, "{}", out.stdout);
    assert!(
        out.stdout.contains("requirement-allocated"),
        "{}",
        out.stdout
    );

    let config = p.join(".oxispec/config.yaml");
    let text = fs::read_to_string(&config).unwrap();
    fs::write(&config, text.replace("level: A", "level: Z")).unwrap();
    let out = run(p, &["check"]);
    assert!(out.stdout.contains("E037"), "{}", out.stdout);
}

#[test]
fn a_release_is_held_until_the_compliance_rules_are_met() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    project_with_a_standard(p, "  standard: iec-62304\n  level: A\n");
    commit_all(p, "dose");
    let out = run(p, &["release", "prepare", "0.1.0"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let out = run(p, &["release", "check", "0.1.0"]);
    assert!(out.stdout.contains("compliance-met"), "{}", out.stdout);
    assert_ne!(out.code, 0, "{}", out.stdout);
}

fn openspec_with_a_change(p: &Path) {
    let write = |rel: &str, text: &str| {
        let f = p.join(rel);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(f, text).unwrap();
    };
    write("openspec/specs/auth/spec.md", OPENSPEC_AUTH);
    write(
        "openspec/specs/auth/design.md",
        "# Design\n\nUse sessions.\n",
    );
    write(
        "openspec/changes/add-2fa/proposal.md",
        "## Why\nPasswords leak.\n\n## What Changes\n- a second step\n\n## Impact\n- auth\n",
    );
    write("openspec/changes/add-2fa/tasks.md", "- [ ] build\n");
    write(
        "openspec/changes/add-2fa/specs/auth/spec.md",
        "## MODIFIED Requirements\n### Requirement: Login\nThe system SHALL sign in with a code.\n",
    );
}

#[test]
fn an_import_plan_is_looked_at_changed_and_only_then_applied() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    import_project(p);
    openspec_with_a_change(p);

    let out = run(p, &["import", "apply"]);
    assert_ne!(out.code, 0);
    assert!(out.stderr.contains("import plan"), "{}", out.stderr);

    let out = run(
        p,
        &["import", "plan", "start", "--from", "openspec", "--json"],
    );
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let v: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    let kinds: Vec<_> = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(kinds, ["requirement", "change", "change"]);
    assert!(p.join(".oxispec/import-plan.json").is_file());
    assert_ne!(
        run(p, &["import", "plan", "start", "--from", "openspec"]).code,
        0,
        "a plan that exists is not replaced"
    );

    // The design notes of the spec are better as a decision.
    let out = run(p, &["import", "plan", "set", "2", "--kind", "adr"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let out = run(p, &["import", "plan", "show", "2", "--draft"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("type: adr"), "{}", out.stdout);
    assert!(out.stdout.contains("Use sessions."), "{}", out.stdout);
    let out = run(p, &["import", "plan", "show", "3", "--draft"]);
    assert!(out.stdout.contains("- REQ-AUTH-0001"), "{}", out.stdout);
    assert!(out.stdout.contains("## Tasks"), "{}", out.stdout);

    let out = run(p, &["import", "plan", "set", "3", "--kind", "bogus"]);
    assert_ne!(out.code, 0);
    let out = run(p, &["import", "apply", "--dry-run"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("would write"), "{}", out.stdout);
    assert!(!p.join("specs/requirements/auth/auth.md").exists());
    assert!(p.join(".oxispec/import-plan.json").is_file());

    let out = run(p, &["import", "apply"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(!p.join(".oxispec/import-plan.json").exists());
    assert!(p.join("specs/requirements/auth/auth.md").is_file());
    let change = fs::read_dir(p.join("specs/changes"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = fs::read_to_string(change).unwrap();
    assert!(text.contains("status: draft"), "{text}");
    assert!(text.contains("## Why\n\nPasswords leak."), "{text}");
    assert!(p.join("specs/adr").read_dir().unwrap().count() == 1);
    let check = run(p, &["check"]);
    assert!(!check.stdout.contains("E002"), "{}", check.stdout);
}

#[test]
fn a_skipped_item_is_not_written_and_an_empty_plan_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    import_project(p);
    openspec_with_a_change(p);
    assert_eq!(
        run(p, &["import", "plan", "start", "--from", "openspec"]).code,
        0
    );
    for n in ["1", "2", "3"] {
        assert_eq!(
            run(p, &["import", "plan", "set", n, "--kind", "skip"]).code,
            0
        );
    }
    let out = run(p, &["import", "apply", "--dry-run"]);
    assert_ne!(out.code, 0);
    assert!(out.stderr.contains("nothing to write"), "{}", out.stderr);
    assert_eq!(
        run(p, &["import", "plan", "set", "1", "--kind", "requirement"]).code,
        0
    );
    let out = run(p, &["import", "apply"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("skipped item 2"), "{}", out.stdout);
    assert!(!p.join("specs/changes").read_dir().unwrap().any(|_| true));
}

#[test]
fn the_committed_reference_matches_the_program() {
    let dir = tempfile::tempdir().unwrap();
    let committed = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/reference");
    let out = run(
        dir.path(),
        &[
            "docs",
            "reference",
            "--check",
            "--out",
            committed.to_str().unwrap(),
        ],
    );
    assert_eq!(
        out.code, 0,
        "docs/reference is out of date. Run: cargo run -- docs reference\n{}",
        out.stderr
    );
}

#[test]
fn docs_reference_writes_the_commands_and_the_codes_and_check_notices_a_change() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path();
    let out = run(p, &["docs", "reference", "--out", "ref"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    let commands = fs::read_to_string(p.join("ref/commands.md")).unwrap();
    for name in [
        "`oxispec check`",
        "`oxispec import plan start`",
        "`oxispec docs reference`",
    ] {
        assert!(commands.contains(name), "{name} is missing");
    }
    assert!(
        !commands.contains("--no-cache`:"),
        "global options are listed once"
    );
    let tools = fs::read_to_string(p.join("ref/mcp-tools.md")).unwrap();
    assert!(tools.contains("## `spec_propose_change`"), "{tools}");
    assert!(tools.contains("Needs the `tests` module."), "{tools}");
    assert!(tools.contains("Writes files."), "{tools}");
    let codes = fs::read_to_string(p.join("ref/check-codes.md")).unwrap();
    assert!(codes.contains("| E002 | error |"), "{codes}");

    assert_eq!(
        run(p, &["docs", "reference", "--check", "--out", "ref"]).code,
        0
    );
    fs::write(p.join("ref/check-codes.md"), "old\n").unwrap();
    let out = run(p, &["docs", "reference", "--check", "--out", "ref"]);
    assert_ne!(out.code, 0);
    assert!(out.stderr.contains("out of date"), "{}", out.stderr);
}

#[test]
fn every_relative_link_in_the_guide_and_the_reference_points_at_a_file() {
    let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    let mut checked = 0;
    for folder in ["guide", "reference"] {
        for entry in fs::read_dir(docs.join(folder)).unwrap() {
            let path = entry.unwrap().path();
            let text = fs::read_to_string(&path).unwrap();
            for part in text.split("](").skip(1) {
                let target = part.split(')').next().unwrap();
                let file = target.split('#').next().unwrap();
                if file.is_empty() || file.starts_with("http") {
                    continue;
                }
                assert!(
                    path.parent().unwrap().join(file).exists(),
                    "{} links to {file}, which does not exist",
                    path.display()
                );
                checked += 1;
            }
        }
    }
    assert!(checked >= 20, "only {checked} links were checked");
}
