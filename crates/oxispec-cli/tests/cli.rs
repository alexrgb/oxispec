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
