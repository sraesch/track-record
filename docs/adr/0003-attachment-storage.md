# ADR 0003: Attachment storage on a shared volume

**Status:** accepted · **Date:** 2026-11-21

## Context

Attachments (screenshots, logs, traces, profiles) are kept forever and removed
only with a hard-deleted run (FR-5, FR-8, FR-10). Object storage would be one
more service to run and back up; as in [ADR 0002](0002-series-storage.md), we
start simple.

## Decision

- **Files on one shared `ReadWriteMany` volume**, mounted by every backend pod.
  The Helm chart takes the storage class (e.g. NFS, CephFS, EFS, Azure Files).
- **Metadata in PostgreSQL:** client ID, owner (run, case execution or sample),
  file name, content type, size, SHA-256.
- **Path:** `{root}/runs/{run_id}/{attachment_id}`. User-supplied names never
  become paths.
- **Only the attachment store module touches files**, so a later move to
  object storage changes one module.

## Rules

- **Write:** stream to a temp file on the volume, verify size and hash, rename
  into place. Never overwrite. Same ID and hash on retry is a no-op; a
  different hash is a conflict (FR-3, FR-6).
- **Order:** file first, then the metadata row. A crash leaves a stray file,
  never a row without a file. Stray files are not results and may be cleaned up.
- **Hard delete:** rows first, then `{root}/runs/{run_id}/`.
- **Backup:** database first, then the volume (snapshots or Velero).
- **Download:** set `Content-Type` and `Content-Disposition` from metadata;
  never render uploaded HTML inline.

## When to revisit

The cluster has no reliable RWX storage class, the volume hits size or
throughput limits, backups become impractical, or downloads load the backend
enough that object storage with pre-signed URLs pays off.
