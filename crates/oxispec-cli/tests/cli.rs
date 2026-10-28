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
