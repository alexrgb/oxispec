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
    let out = Command::new("git")
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
