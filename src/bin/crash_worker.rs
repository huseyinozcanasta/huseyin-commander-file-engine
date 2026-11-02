//! Separate process intentionally exits without destructors, fixture-scoped.
use hc_atomic_file_lab::{commit, prepare, undo, Failpoint};
use std::path::PathBuf;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        std::process::exit(91)
    }
    let p = PathBuf::from(&args[1]);
    let ok = match args[2].as_str() {
        "before" => prepare(&p, || true).is_ok() && commit(&p, Failpoint::BeforeReplace).is_err(),
        "after" => prepare(&p, || true).is_ok() && commit(&p, Failpoint::AfterReplace).is_err(),
        "undo" => {
            prepare(&p, || true).is_ok()
                && commit(&p, Failpoint::None).is_ok()
                && undo(&p, || true, true).is_err()
        }
        "committed" => prepare(&p, || true).is_ok() && commit(&p, Failpoint::None).is_ok(),
        _ => false,
    };
    if ok {
        std::process::exit(47)
    } else {
        std::process::exit(92)
    }
}
