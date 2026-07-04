# Distributed Backup Engine

Implemented in [`crates/open-runo-backup`](../crates/open-runo-backup).

## Scope (Phase 5)

- `BackupTarget`: `LocalStorage`, `RemoteVps`, `S3Compatible`,
  `PeerOpenRunoNode`, `GitArchive`.
- `BackupKind`: `Full` / `Incremental`.
- `BackupJob`: models the lifecycle `Scheduled -> Running -> Succeeded`
  (or `Failed { reason }`), enforcing valid transitions (e.g. you cannot
  mark a `Scheduled` job `Succeeded` without it having run).
- Jobs default to `encrypted: true`.

## Not yet implemented

This crate models backup *plans and state transitions*; it does not yet
implement the actual transport for any `BackupTarget` (no S3 client, no
rsync-over-SSH, no peer-node protocol). Scheduling (cron-like recurring
jobs), integrity checks, and restore-test workflows (README §7) are also
future work — they would consume `BackupJob` as their unit of work.
