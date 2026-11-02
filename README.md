# Hüseyin Commander — Windows File Engine Lab

**Status: experimental / test-only.** A free, open-source Rust prototype for Windows NTFS atomic file replacement, integrity-checked backups, recovery after abrupt process exits and native consent denial testing.

**Not the complete Hüseyin Commander desktop app.** No remote control, arbitrary path access, elevated permissions, background service, MCP write permissions or Tauri bridge.

## Features

- Windows ReplaceFileW changes the file in one replacement operation, retaining a backup and recording original/replacement volume + file-index identities.
- On-disk SHA-256 journal: original.bin, new.bin, staging.tmp, record.json, with commit/undo markers.
- Recovery on restart examines the journal and file identity/content; known interrupted states roll back, unknown states fail closed.
- Explicit consent callbacks for prepare and undo; separate MessageBoxW test binary checks the real Windows No / Hayır button.
- Negative tests: rejected consent, changed file, read-only Windows attribute, hardlinks, journal tampering, aborted replacement and interrupted undo.
- Separate Windows process exits with code 47, and another process performs recovery.

**Safety scope:** file operations are hardcoded to sample.txt within auto-created Windows temp directories named hc-a16-fixture-*. Arbitrary personal folders and file names are not accepted.

## Requirements and tests

- Windows 10/11 with NTFS
- Rust stable and Cargo (validated with Rust 1.99.1)
- No paid API or tokens

Run in PowerShell:
    cargo test --locked -- --test-threads=1
    cargo clippy --locked --all-targets -- -D warnings

For offline dependencies cached locally:
    .\run-tests.ps1

For the native Windows **NO** dialog test in an interactive desktop:
    .\run-native-no-test.ps1

The UI test opens a MessageBoxW belonging to its own test process and programmatically selects the No control (IDNO=7). It verifies that the test file was not changed and no journal created. This is **not** a human-performed acceptance test or an end-to-end Tauri test. Do not run the UI test in headless CI.

## Verified behavior

10 Rust unit tests + 4 separate-process recovery integration tests pass on the reference Windows device. Native Windows No denial passes separately. ReplaceFileW's result takes the staged file ID; the backup holds the original ID. An additional replacement can restore the original. An in-place write can preserve identity but does not have the same atomic-visibility properties.

## Critical security limitations

**Do not use on important files or in production.** SHA-256 integrity is not authentication or encryption. This prototype does not guarantee power-loss durability for Windows metadata or storage cache. TOCTOU, ACL/SID, ancestor junction swaps, hardlink races, alternate data streams, antivirus/cloud-sync and user-selected folder permissions require further validation. On ambiguous ReplaceFileW failure do not blindly retry before inspecting target, staging and backup.

This project is deliberately not connected to the installed Hüseyin Commander or private Desktop Commander. No user documents were modified.

## Roadmap

1. NTFS handle-bound authorization, ACL/SID and directory-reparse race tests.
2. Stronger journal/power-loss durability guarantees.
3. Human-operated Windows confirmation and Tauri GUI acceptance tests.
4. Separately authorized revocable per-file write grants.
5. Only then consider scoped MCP and remote management.

## References

- Microsoft ReplaceFileW: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew
- Microsoft GetFileInformationByHandle: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle
- Rust OpenOptionsExt: https://doc.rust-lang.org/std/os/windows/fs/trait.OpenOptionsExt.html

MIT licensed © 2026 Hüseyin Özcan. See LICENSE.
