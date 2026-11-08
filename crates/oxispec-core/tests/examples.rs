//! The examples in `examples/` run and say what the guide says they say.

use std::process::Command;

/// Runs an example through cargo itself, so that the test does not depend on where cargo
/// puts the build (that differs between versions of cargo).
fn run(name: &str) -> String {
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let out = Command::new(env!("CARGO"))
        .args([
            "run",
            "--quiet",
            "--locked",
            "--manifest-path",
            manifest,
            "--example",
            name,
        ])
        .output()
        .unwrap_or_else(|e| panic!("cannot run cargo: {e}"));
    assert!(
        out.status.success(),
        "{name} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

// verifies: REQ-LIBRARY-0001.S1
#[test]
fn every_example_runs_and_prints_what_it_teaches() {
    let read = run("read_specs");
    assert!(
        read.contains("REQ-AUTH-0001 points at ADR-0001 with `decided-by`"),
        "{read}"
    );

    let check = run("check_specs");
    assert!(check.contains("error E002 in REQ-PAY-0001"), "{check}");
    assert!(check.contains("2 findings, 2 errors"), "{check}");

    let trace = run("trace_coverage");
    assert!(trace.contains("verified 1/2 scenarios (50%)"), "{trace}");
    assert!(
        trace.contains("no test verifies REQ-AUTH-0001.S2"),
        "{trace}"
    );

    let context = run("context_for_an_agent");
    assert!(
        context.contains("included ADR-0001 (REQ-AUTH-0001 decided-by ADR-0001)"),
        "{context}"
    );
    assert!(
        context.contains("only a pointer: REQ-PAY-0001 (2 hops away)"),
        "{context}"
    );
    assert!(!context.contains("included REQ-PAY-0001"), "{context}");

    let diff = run("compare_revisions");
    assert!(diff.contains("added   REQ-A-0003"), "{diff}");
    assert!(diff.contains("removed REQ-A-0002"), "{diff}");
    assert!(diff.contains("changed REQ-A-0001"), "{diff}");

    let api = run("search_the_api");
    assert!(api.contains("second sync: 0 added, 2 unchanged"), "{api}");
    let nearest = api.lines().find(|l| l.trim_start().starts_with("0."));
    assert!(
        nearest.is_some_and(|l| l.contains("POST /payments")),
        "the nearest operation to `charge a card`: {api}"
    );
}
