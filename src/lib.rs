//! Windows NTFS atomic-replacement + journal research lab.
//! Test fixtures only. NOT a production, sandbox, or privilege-escalation API.
#![cfg(windows)]
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    ffi::c_void,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::windows::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    },
    path::{Path, PathBuf},
};

pub const OLD: &[u8] = b"HC-A16 original sample\n";
pub const NEW: &[u8] = b"HC-A16 updated sample\n";
const SIZE: usize = 1024;
const REPARSE: u32 = 0x400;
const OPEN_REPARSE: u32 = 0x0020_0000;
const NAME: &str = "sample.txt";
#[repr(C)]
struct Time {
    lo: u32,
    hi: u32,
}
#[repr(C)]
struct Info {
    attributes: u32,
    c: Time,
    a: Time,
    w: Time,
    volume: u32,
    hi: u32,
    lo: u32,
    links: u32,
    idxhi: u32,
    idxlo: u32,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetFileInformationByHandle(h: *mut c_void, i: *mut Info) -> i32;
    fn ReplaceFileW(
        target: *const u16,
        source: *const u16,
        backup: *const u16,
        flags: u32,
        x: *mut c_void,
        y: *mut c_void,
    ) -> i32;
}
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct FileId {
    volume: u32,
    index: u64,
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    version: u8,
    file: String,
    before: String,
    after: String,
    original: FileId,
    staged: FileId,
}
fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
fn e(err: std::io::Error) -> String {
    err.to_string()
}
fn fileid(p: &Path) -> Result<FileId, String> {
    let f = OpenOptions::new()
        .read(true)
        .share_mode(7)
        .custom_flags(OPEN_REPARSE)
        .open(p)
        .map_err(e)?;
    let mut info = std::mem::MaybeUninit::<Info>::uninit();
    if unsafe { GetFileInformationByHandle(f.as_raw_handle().cast(), info.as_mut_ptr()) } == 0 {
        return Err(e(std::io::Error::last_os_error()));
    }
    let x = unsafe { info.assume_init() };
    if x.attributes & REPARSE != 0 || x.links != 1 {
        return Err("Reparse or multi-hardlink file denied".into());
    }
    Ok(FileId {
        volume: x.volume,
        index: ((x.idxhi as u64) << 32) | x.idxlo as u64,
    })
}
fn regular(p: &Path, folder: bool) -> Result<(), String> {
    let x = fs::symlink_metadata(p).map_err(e)?;
    if (x.file_attributes() & REPARSE) != 0 || if folder { !x.is_dir() } else { !x.is_file() } {
        return Err("Non-regular file or reparse point denied".into());
    }
    if !folder && x.len() > SIZE as u64 {
        return Err("File too large".into());
    }
    Ok(())
}
fn scope(root: &Path) -> Result<PathBuf, String> {
    regular(root, true)?;
    let temp = fs::canonicalize(std::env::temp_dir()).map_err(e)?;
    let can = fs::canonicalize(root).map_err(e)?;
    if can.parent() != Some(temp.as_path())
        || !can
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("hc-a16-fixture-"))
    {
        return Err("Outside isolated A16 temporary fixture".into());
    }
    let target = can.join(NAME);
    regular(&target, false)?;
    if fs::canonicalize(&target).map_err(e)?.parent() != Some(can.as_path()) {
        return Err("Target escaped fixture".into());
    }
    Ok(can)
}
fn read(p: &Path) -> Result<Vec<u8>, String> {
    regular(p, false)?;
    let mut f = File::open(p).map_err(e)?;
    let mut v = Vec::new();
    (&mut f)
        .take((SIZE + 1) as u64)
        .read_to_end(&mut v)
        .map_err(e)?;
    if v.len() > SIZE {
        return Err("Too large".into());
    }
    Ok(v)
}
fn store_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > SIZE * 4 {
        return Err("Journal payload too large".into());
    }
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(e)?;
    f.write_all(bytes).and_then(|_| f.sync_all()).map_err(e)
}
fn wide(p: &Path) -> Vec<u16> {
    p.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
fn replace(dst: &Path, src: &Path, backup: &Path) -> Result<(), String> {
    let (d, s, b) = (wide(dst), wide(src), wide(backup));
    if unsafe {
        ReplaceFileW(
            d.as_ptr(),
            s.as_ptr(),
            b.as_ptr(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(format!("ReplaceFileW: {}", std::io::Error::last_os_error()));
    }
    Ok(())
}
fn journal(r: &Path) -> PathBuf {
    r.join("a16-journal")
}
fn snapshot(j: &Path) -> Result<(Manifest, Vec<u8>, Vec<u8>), String> {
    regular(j, true)?;
    let record = fs::read(j.join("record.json")).map_err(e)?;
    if record.len() > 4096 {
        return Err("Oversize manifest".into());
    }
    let m: Manifest = serde_json::from_slice(&record).map_err(|e| e.to_string())?;
    if m.version != 1 || m.file != NAME {
        return Err("Manifest scope invalid".into());
    }
    let old = read(&j.join("original.bin"))?;
    let new = read(&j.join("new.bin"))?;
    if digest(&old) != m.before || digest(&new) != m.after {
        return Err("Journal SHA256 mismatch".into());
    }
    if old != OLD || new != NEW {
        return Err("Outside known fixture content".into());
    }
    Ok((m, old, new))
}
fn remove_journal(j: &Path) -> Result<(), String> {
    regular(j, true)?;
    for entry in fs::read_dir(j).map_err(e)? {
        let ent = entry.map_err(e)?;
        regular(&ent.path(), false)?;
        fs::remove_file(ent.path()).map_err(e)?;
    }
    fs::remove_dir(j).map_err(e)
}
pub fn create_fixture(root: &Path) -> Result<(), String> {
    let parent = fs::canonicalize(std::env::temp_dir()).map_err(e)?;
    if root.parent() != Some(parent.as_path())
        || !root
            .file_name()
            .is_some_and(|s| s.to_string_lossy().starts_with("hc-a16-fixture-"))
        || root.exists()
    {
        return Err("Only new, isolated temp fixtures permitted".into());
    }
    fs::create_dir(root).map_err(e)?;
    store_new(&root.join(NAME), OLD)
}
pub fn prepare(root: &Path, approve: impl FnOnce() -> bool) -> Result<(), String> {
    let root = scope(root)?;
    if journal(&root).exists() {
        return Err("Unresolved journal exists".into());
    }
    let target = root.join(NAME);
    if fs::metadata(&target).map_err(e)?.permissions().readonly() {
        return Err("Read-only Windows target denied".into());
    }
    if read(&target)? != OLD {
        return Err("Unexpected original content".into());
    }
    if !approve() {
        return Err("Explicit permission denied".into());
    }
    // Validate again after user consent; no privileges are inherited or widened.
    if read(&target)? != OLD {
        return Err("Changed during approval".into());
    }
    let old_id = fileid(&target)?;
    let j = journal(&root);
    fs::create_dir(&j).map_err(e)?;
    store_new(&j.join("original.bin"), OLD)?;
    store_new(&j.join("new.bin"), NEW)?;
    store_new(&j.join("staging.tmp"), NEW)?;
    let staged = fileid(&j.join("staging.tmp"))?;
    let m = Manifest {
        version: 1,
        file: NAME.into(),
        before: digest(OLD),
        after: digest(NEW),
        original: old_id,
        staged,
    };
    store_new(
        &j.join("record.json"),
        &serde_json::to_vec(&m).map_err(|e| e.to_string())?,
    )?;
    Ok(())
}
#[derive(Clone, Copy)]
pub enum Failpoint {
    None,
    BeforeReplace,
    AfterReplace,
}
pub fn commit(root: &Path, fail: Failpoint) -> Result<(), String> {
    let root = scope(root)?;
    let j = journal(&root);
    let (m, old, new) = snapshot(&j)?;
    let target = root.join(NAME);
    let stage = j.join("staging.tmp");
    let backup = j.join("backup.bak");
    if backup.exists() || j.join("committed.flag").exists() {
        return Err("Ambiguous transaction already started".into());
    }
    if read(&target)? != old || fileid(&target)? != m.original {
        return Err("Original changed before commit".into());
    }
    if read(&stage)? != new || fileid(&stage)? != m.staged {
        return Err("Staging changed before commit".into());
    }
    if matches!(fail, Failpoint::BeforeReplace) {
        return Err("Injected exit before ReplaceFileW".into());
    }
    // TOCTOU: ReplaceFileW takes file paths; this remains an isolated fixture test, not race-proof.
    replace(&target, &stage, &backup)?;
    if matches!(fail, Failpoint::AfterReplace) {
        return Err("Injected exit after ReplaceFileW".into());
    }
    if fileid(&target)? != m.staged
        || fileid(&backup)? != m.original
        || read(&target)? != new
        || read(&backup)? != old
    {
        return Err("Post-replace integrity failed".into());
    }
    store_new(&j.join("committed.flag"), b"v1")?;
    Ok(())
}
#[derive(Debug, PartialEq, Eq)]
pub enum Status {
    NoJournal,
    PreparedAborted,
    RollbackCompleted,
    Committed,
    Undone,
}
pub fn recover(root: &Path) -> Result<Status, String> {
    let root = scope(root)?;
    let j = journal(&root);
    if !j.exists() {
        return Ok(Status::NoJournal);
    }
    let (m, old, new) = snapshot(&j)?;
    let target = root.join(NAME);
    let committed = j.join("committed.flag").exists();
    let undoing = j.join("undo.flag").exists();
    if committed && !undoing {
        if read(&target)? != new || fileid(&target)? != m.staged {
            return Err("Committed file altered; manual review".into());
        }
        let backup = j.join("backup.bak");
        if read(&backup)? != old || fileid(&backup)? != m.original {
            return Err("Committed backup altered".into());
        }
        return Ok(Status::Committed);
    }
    if undoing {
        if !committed {
            return Err("Invalid undo state".into());
        }
        if read(&target)? == old && fileid(&target)? == m.original {
            let undone = j.join("undone-new.bak");
            if read(&undone)? != new || fileid(&undone)? != m.staged {
                return Err("Interrupted undo copy mismatch".into());
            }
            remove_journal(&j)?;
            return Ok(Status::Undone);
        }
        if read(&target)? == new && fileid(&target)? == m.staged {
            if read(&j.join("backup.bak"))? != old {
                return Err("Undo backup changed".into());
            }
            return Ok(Status::Committed);
        }
        return Err("Unknown partial undo state; manual review".into());
    }
    let stage = j.join("staging.tmp");
    let backup = j.join("backup.bak");
    if read(&target)? == old && fileid(&target)? == m.original && stage.exists() && !backup.exists()
    {
        if read(&stage)? != new || fileid(&stage)? != m.staged {
            return Err("Prepared staging corrupted".into());
        }
        remove_journal(&j)?;
        return Ok(Status::PreparedAborted);
    }
    if read(&target)? == new && fileid(&target)? == m.staged && backup.exists() && !stage.exists() {
        if read(&backup)? != old || fileid(&backup)? != m.original {
            return Err("Rollback backup corrupted".into());
        }
        let undone = j.join("reverted-new.bak");
        if undone.exists() {
            return Err("Rollback already ambiguous".into());
        }
        replace(&target, &backup, &undone)?;
        if read(&target)? != old || fileid(&target)? != m.original {
            return Err("Failed rollback integrity".into());
        }
        remove_journal(&j)?;
        return Ok(Status::RollbackCompleted);
    }
    Err("Unknown transaction state; manual review required".into())
}
pub fn undo(
    root: &Path,
    approve: impl FnOnce() -> bool,
    fail_after_replace: bool,
) -> Result<(), String> {
    let root = scope(root)?;
    let j = journal(&root);
    let (m, old, new) = snapshot(&j)?;
    if !j.join("committed.flag").exists() {
        return Err("Not committed".into());
    }
    if j.join("undo.flag").exists() {
        return Err("Undo already in progress".into());
    }
    let target = root.join(NAME);
    let backup = j.join("backup.bak");
    let undone = j.join("undone-new.bak");
    if read(&target)? != new
        || fileid(&target)? != m.staged
        || read(&backup)? != old
        || fileid(&backup)? != m.original
    {
        return Err("Changed target/backup".into());
    }
    if !approve() {
        return Err("Undo user denied".into());
    }
    if read(&target)? != new || fileid(&target)? != m.staged {
        return Err("Changed during approval".into());
    }
    store_new(&j.join("undo.flag"), b"v1")?;
    replace(&target, &backup, &undone)?;
    if fail_after_replace {
        return Err("Injected exit after undo replace".into());
    }
    if read(&target)? != old || fileid(&target)? != m.original {
        return Err("Undo integrity failure".into());
    }
    remove_journal(&j)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    pub struct Fixture(pub PathBuf);
    impl Fixture {
        pub fn new() -> Self {
            let root = fs::canonicalize(std::env::temp_dir())
                .unwrap()
                .join(format!(
                    "hc-a16-fixture-{}-{}",
                    std::process::id(),
                    N.fetch_add(1, Ordering::Relaxed)
                ));
            create_fixture(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn deny_approval_no_journal() {
        let f = Fixture::new();
        assert!(prepare(&f.0, || false).is_err());
        assert!(!journal(&f.0).exists());
        assert_eq!(read(&f.0.join(NAME)).unwrap(), OLD);
    }
    #[test]
    fn prepare_abort_on_restart() {
        let f = Fixture::new();
        prepare(&f.0, || true).unwrap();
        assert_eq!(recover(&f.0).unwrap(), Status::PreparedAborted);
        assert_eq!(read(&f.0.join(NAME)).unwrap(), OLD);
    }
    #[test]
    fn commit_undo_same_file_content_and_ids() {
        let f = Fixture::new();
        let old_id = fileid(&f.0.join(NAME)).unwrap();
        prepare(&f.0, || true).unwrap();
        commit(&f.0, Failpoint::None).unwrap();
        assert_ne!(fileid(&f.0.join(NAME)).unwrap(), old_id);
        assert_eq!(recover(&f.0).unwrap(), Status::Committed);
        assert!(undo(&f.0, || false, false).is_err());
        assert_eq!(read(&f.0.join(NAME)).unwrap(), NEW);
        undo(&f.0, || true, false).unwrap();
        assert_eq!(read(&f.0.join(NAME)).unwrap(), OLD);
        assert_eq!(fileid(&f.0.join(NAME)).unwrap(), old_id);
    }
    #[test]
    fn before_commit_crash_aborts() {
        let f = Fixture::new();
        prepare(&f.0, || true).unwrap();
        assert!(commit(&f.0, Failpoint::BeforeReplace).is_err());
        assert_eq!(recover(&f.0).unwrap(), Status::PreparedAborted);
    }
    #[test]
    fn after_replace_crash_rolls_back() {
        let f = Fixture::new();
        prepare(&f.0, || true).unwrap();
        assert!(commit(&f.0, Failpoint::AfterReplace).is_err());
        assert_eq!(read(&f.0.join(NAME)).unwrap(), NEW);
        assert_eq!(recover(&f.0).unwrap(), Status::RollbackCompleted);
        assert_eq!(read(&f.0.join(NAME)).unwrap(), OLD);
    }
    #[test]
    fn interrupted_undo_completes_recovery() {
        let f = Fixture::new();
        prepare(&f.0, || true).unwrap();
        commit(&f.0, Failpoint::None).unwrap();
        assert!(undo(&f.0, || true, true).is_err());
        assert_eq!(recover(&f.0).unwrap(), Status::Undone);
        assert_eq!(read(&f.0.join(NAME)).unwrap(), OLD);
    }
    #[test]
    fn corrupted_snapshot_denied() {
        let f = Fixture::new();
        prepare(&f.0, || true).unwrap();
        fs::write(journal(&f.0).join("original.bin"), b"tamper").unwrap();
        assert!(recover(&f.0).is_err());
        assert!(journal(&f.0).exists());
    }
    #[test]
    fn changed_file_denied() {
        let f = Fixture::new();
        prepare(&f.0, || true).unwrap();
        fs::write(f.0.join(NAME), b"third party").unwrap();
        assert!(commit(&f.0, Failpoint::None).is_err());
        assert!(recover(&f.0).is_err());
    }
    #[test]
    fn readonly_windows_attribute_denied_without_journal() {
        let f = Fixture::new();
        let target = f.0.join(NAME);
        let original_permissions = fs::metadata(&target).unwrap().permissions();
        let mut read_only_permissions = original_permissions.clone();
        read_only_permissions.set_readonly(true);
        fs::set_permissions(&target, read_only_permissions).unwrap();
        assert!(prepare(&f.0, || true).is_err());
        assert_eq!(read(&target).unwrap(), OLD);
        assert!(!journal(&f.0).exists());
        fs::set_permissions(&target, original_permissions).unwrap();
    }
    #[test]
    fn hardlink_denied() {
        let f = Fixture::new();
        fs::hard_link(f.0.join(NAME), f.0.join("alias.txt")).unwrap();
        assert!(prepare(&f.0, || true).is_err());
    }
}
