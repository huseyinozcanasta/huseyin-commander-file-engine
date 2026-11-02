# Security

This is a **fixture-scoped research prototype**, not a security product or Windows sandbox. Do not connect the file mutation API to a network endpoint or arbitrary paths.

The code supports only automatically created Windows temporary test fixtures. It does not ask for or store tokens, API keys, personal information or administrator credentials.

If you discover a vulnerability, prefer GitHub's private security advisory reporting feature (if enabled), rather than publishing sensitive exploit details or data.

Not-yet-resolved areas: power failure and metadata durability, process-level path races and NTFS junction-ancestor swaps, ACL/SID, authenticated backups, arbitrary file inputs and end-user graphical consent.
