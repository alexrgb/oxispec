//! The examples in `examples/` run and say what the guide says they say.

use std::process::Command;

/// `cargo test` builds the examples before it runs the tests of a package, and
/// puts them next to the folder of the test binaries.
fn run(name: &str) -> String {
    let exe = std::env::current_exe().unwrap();
    let dir = exe.parent().unwrap().parent().unwrap().join("examples");
    let mut path = dir.join(name);
    if cfg!(windows) {
        path.set_extension("exe");
    }
    let out = Command::new(&path)
        .output()
        .unwrap_or_else(|e| panic!("cannot run {}: {e}", path.display()));
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
        read.contains("ADR-0001 is the decided-by of REQ-AUTH-0001"),
        "{read}"
    );

    let check = run("check_specs");
    assert!(check.contains("E002"), "{check}");
    assert!(check.contains("2 errors"), "{check}");

    let trace = run("trace_coverage");
    assert!(trace.contains("verified 1/2 scenarios (50%)"), "{trace}");
    assert!(
        trace.contains("no test verifies REQ-AUTH-0001.S2"),
        "{trace}"
    );

    let context = run("context_for_an_agent");
    assert!(
        context.contains("ADR-0001 (REQ-AUTH-0001 decided-by ADR-0001)"),
        "{context}"
    );
    assert!(!context.contains("REQ-PAY-0001 ("), "{context}");

    let diff = run("compare_revisions");
    assert!(diff.contains("added   REQ-A-0003"), "{diff}");
    assert!(diff.contains("removed REQ-A-0002"), "{diff}");
    assert!(diff.contains("changed REQ-A-0001"), "{diff}");

    let api = run("search_the_api");
    let nearest = api.lines().find(|l| l.starts_with("0."));
    assert!(
        nearest.is_some_and(|l| l.contains("POST /payments")),
        "the nearest operation to `charge a card`: {api}"
    );
}
