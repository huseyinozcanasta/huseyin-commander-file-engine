//! Real Windows MessageBoxW negative-consent fixture test. No installed GUI integration.
use hc_atomic_file_lab::{create_fixture, prepare, OLD};
use std::{ffi::OsStr, fs, os::windows::ffi::OsStrExt};
#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBoxW(
        hwnd: *mut std::ffi::c_void,
        text: *const u16,
        caption: *const u16,
        kind: u32,
    ) -> i32;
}
fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
fn main() {
    let root = std::env::temp_dir()
        .canonicalize()
        .unwrap()
        .join(format!("hc-a16-fixture-consent-{}", std::process::id()));
    if create_fixture(&root).is_err() {
        std::process::exit(92)
    }
    let result = prepare(&root, || {
        let text=wide("HC-A16 Windows Consent Test\n\nOnly a disposable sample file would be prepared.\nPress NO / HAYIR to reject. No real files are touched.");
        let title = wide("HC-A16 Native Consent Test");
        // MB_YESNO|MB_ICONWARNING|MB_DEFBUTTON2|MB_TOPMOST.
        let clicked = unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                0x4 | 0x30 | 0x100 | 0x40000,
            )
        };
        println!("NATIVE_DIALOG_RESULT={clicked}");
        clicked == 6
    });
    let old_ok = fs::read(root.join("sample.txt"))
        .map(|x| x == OLD)
        .unwrap_or(false);
    let journal_exists = root.join("a16-journal").exists();
    let _ = fs::remove_dir_all(&root);
    if result.is_err() && old_ok && !journal_exists {
        println!("A16_NATIVE_NO_DENIED_AND_NO_MUTATION");
        std::process::exit(0)
    }
    eprintln!(
        "NATIVE_NO_TEST_FAILED: result={:?} old_ok={} journal={}",
        result, old_ok, journal_exists
    );
    std::process::exit(93)
}
