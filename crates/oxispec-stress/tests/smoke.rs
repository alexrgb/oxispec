//! The stress tool works: it makes a project, `check` agrees with the plan, every command
//! runs, and both load modes finish without a failed request.

use std::process::Command;

fn stress(args: &[&str]) -> std::process::Output {
    // The program under test is built by cargo for the CLI package, not for this one.
    let status = Command::new(env!("CARGO"))
        .args(["build", "-q", "-p", "oxispec"])
        .status()
        .expect("cargo runs");
    assert!(status.success());
    Command::new(env!("CARGO_BIN_EXE_oxispec-stress"))
        .args(args)
        .output()
        .expect("the stress tool runs")
}

fn ok(args: &[&str]) -> String {
    let out = stress(args);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "{args:?}:\n{text}");
    text
}

#[test]
fn a_run_times_every_command_and_the_oracle_agrees_with_planted_conflicts() {
    let text = ok(&[
        "run",
        "--preset",
        "tiny",
        "--conflicts",
        "3",
        "--repeat",
        "1",
        "--no-budget",
    ]);
    assert!(
        text.contains("oracle: check reports exactly the 3 planted conflicts"),
        "{text}"
    );
    for name in [
        "check (cold)",
        "search",
        "ci --base v1",
        "site",
        "machine tests",
    ] {
        assert!(text.contains(name), "{name} is not in:\n{text}");
    }
    assert!(!text.contains("FAILED"), "{text}");
}

#[test]
fn the_mcp_load_finishes_without_failed_requests() {
    let text = ok(&[
        "load",
        "--preset",
        "tiny",
        "--mode",
        "mcp",
        "--clients",
        "3",
        "--requests",
        "15",
    ]);
    assert!(text.contains("requests per second"), "{text}");
}

#[test]
fn processes_sharing_a_project_do_not_clash_and_writers_get_distinct_ids() {
    let text = ok(&[
        "load",
        "--preset",
        "tiny",
        "--mode",
        "cli",
        "--clients",
        "4",
        "--requests",
        "6",
        "--writers",
        "3",
    ]);
    assert!(text.contains("0 requirements share an ID"), "{text}");
}
