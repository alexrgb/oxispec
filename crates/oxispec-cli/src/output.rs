//! Where command output goes.
//!
//! Commands write with `out!` and `outln!` instead of `println!`. Normally
//! that is stdout. The MCP server runs commands in its own process and
//! collects what they write with [`capture`], so a tool call costs no new
//! process. The buffer belongs to the thread, so two captures never mix.

use std::cell::RefCell;
use std::io::Write;

thread_local! {
    static SINK: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
}

/// Runs `f` and returns what it wrote with `out!`, along with its result.
pub fn capture<T>(f: impl FnOnce() -> T) -> (String, T) {
    let before = SINK.with(|s| s.borrow_mut().replace(Vec::new()));
    let value = f();
    let bytes = SINK.with(|s| std::mem::replace(&mut *s.borrow_mut(), before));
    (
        String::from_utf8_lossy(&bytes.unwrap_or_default()).into_owned(),
        value,
    )
}

pub fn write(args: std::fmt::Arguments) {
    let buffered = SINK.with(|s| match s.borrow_mut().as_mut() {
        Some(buffer) => {
            let _ = buffer.write_fmt(args);
            true
        }
        None => false,
    });
    if !buffered {
        // Like `print!`: a closed pipe is a panic that `main` turns into a quiet exit.
        let mut out = std::io::stdout().lock();
        if let Err(e) = out.write_fmt(args) {
            // The text has "(os error N)", which the panic hook in `main` looks for.
            panic!("failed printing to stdout: {e}");
        }
    }
}

macro_rules! out {
    ($($arg:tt)*) => { $crate::output::write(format_args!($($arg)*)) };
}

macro_rules! outln {
    () => { $crate::output::write(format_args!("\n")) };
    ($($arg:tt)*) => { $crate::output::write(format_args!("{}\n", format_args!($($arg)*))) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_what_is_written_and_nests() {
        let (outer, inner) = capture(|| {
            outln!("a {}", 1);
            let (inner, _) = capture(|| out!("b"));
            outln!("c");
            inner
        });
        assert_eq!(inner, "b");
        assert_eq!(outer, "a 1\nc\n");
    }
}
