#![cfg(windows)]
use hc_atomic_file_lab::{create_fixture, recover, Status, NEW, OLD};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
static COUNT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "hc-a16-fixture-child-{}-{}",
                std::process::id(),
                COUNT.fetch_add(1, Ordering::SeqCst)
            ));
        create_fixture(&root).unwrap();
        Self(root)
    }
    fn child(&self, mode: &str) {
        let status = Command::new(env!("CARGO_BIN_EXE_crash_worker"))
            .arg(&self.0)
            .arg(mode)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(47));
    }
    fn bytes(&self) -> Vec<u8> {
        fs::read(self.0.join("sample.txt")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn separate_process_crash_before_replace() {
    let f = Fixture::new();
    f.child("before");
    assert_eq!(f.bytes(), OLD);
    assert_eq!(recover(&f.0).unwrap(), Status::PreparedAborted);
    assert_eq!(f.bytes(), OLD);
}
#[test]
fn separate_process_crash_after_replace() {
    let f = Fixture::new();
    f.child("after");
    assert_eq!(f.bytes(), NEW);
    assert_eq!(recover(&f.0).unwrap(), Status::RollbackCompleted);
    assert_eq!(f.bytes(), OLD);
}
#[test]
fn separate_process_crash_after_undo_replace() {
    let f = Fixture::new();
    f.child("undo");
    assert_eq!(f.bytes(), OLD);
    assert_eq!(recover(&f.0).unwrap(), Status::Undone);
    assert_eq!(f.bytes(), OLD);
}
#[test]
fn separate_process_committed_journal_survives() {
    let f = Fixture::new();
    f.child("committed");
    assert_eq!(f.bytes(), NEW);
    assert_eq!(recover(&f.0).unwrap(), Status::Committed);
}
