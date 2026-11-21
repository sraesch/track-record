//! Attachment store (ADR 0003, FR-5).
//!
//! The ONLY module that touches attachment files. Files live on the shared
//! volume under `{root}/runs/{run_id}/{attachment_id}`; user-supplied names
//! never become paths. A test in tests/invariants.rs enforces that no other
//! module uses the file system.
//!
//! Write: stream into a temp file next to the target, hash and count while
//! writing, fsync, then hard-link into place (which never overwrites) and
//! remove the temp file. The caller writes the metadata row afterwards, so a
//! crash leaves a stray file, never a row without a file.

use std::io;
use std::path::{Path, PathBuf};

use bytes::Bytes;
use futures::{Stream, StreamExt};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AttachmentStore {
    root: PathBuf,
}

/// A fully written temp file, not yet in place. Removed on drop unless
/// committed.
#[derive(Debug)]
pub struct TempFile {
    path: PathBuf,
    pub size: u64,
    pub sha256: [u8; 32],
    committed: bool,
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("reading the upload failed: {0}")]
    Body(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl AttachmentStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Creates the root directory if needed and checks that it is writable.
    pub async fn init(&self) -> io::Result<()> {
        tokio::fs::create_dir_all(self.root.join("runs")).await?;
        let probe = self.root.join(format!(".probe-{}", Uuid::new_v4()));
        tokio::fs::write(&probe, b"").await?;
        tokio::fs::remove_file(&probe).await
    }

    fn run_dir(&self, run_id: Uuid) -> PathBuf {
        self.root.join("runs").join(run_id.to_string())
    }

    fn path(&self, run_id: Uuid, attachment_id: Uuid) -> PathBuf {
        self.run_dir(run_id).join(attachment_id.to_string())
    }

    /// Streams an upload into a temp file in the run's directory.
    pub async fn write_temp<S, E>(&self, run_id: Uuid, mut body: S) -> Result<TempFile, WriteError>
    where
        S: Stream<Item = Result<Bytes, E>> + Unpin,
        E: std::fmt::Display,
    {
        let dir = self.run_dir(run_id);
        tokio::fs::create_dir_all(&dir).await?;
        let mut temp = TempFile {
            path: dir.join(format!(".tmp-{}", Uuid::new_v4())),
            size: 0,
            sha256: [0; 32],
            committed: false,
        };
        let mut file = tokio::fs::File::create_new(&temp.path).await?;
        let mut hasher = Sha256::new();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(|e| WriteError::Body(e.to_string()))?;
            hasher.update(&chunk);
            temp.size += chunk.len() as u64;
            file.write_all(&chunk).await?;
        }
        file.sync_all().await?;
        temp.sha256 = hasher.finalize().into();
        Ok(temp)
    }

    /// Moves a temp file into place as `attachment_id`. The caller must hold
    /// a lock on the attachment id and must have checked that no metadata row
    /// exists, so a file already in place is a stray from a crashed upload:
    /// it is kept if identical and replaced otherwise. Never overwrites a
    /// file that belongs to a stored attachment.
    pub async fn commit(
        &self,
        mut temp: TempFile,
        run_id: Uuid,
        attachment_id: Uuid,
    ) -> io::Result<()> {
        let target = self.path(run_id, attachment_id);
        match tokio::fs::hard_link(&temp.path, &target).await {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                if hash_file(&target).await? != temp.sha256 {
                    tokio::fs::remove_file(&target).await?;
                    tokio::fs::hard_link(&temp.path, &target).await?;
                }
            }
            Err(e) => return Err(e),
        }
        sync_dir(&self.run_dir(run_id)).await?;
        tokio::fs::remove_file(&temp.path).await?;
        temp.committed = true;
        Ok(())
    }

    /// Opens a stored attachment for reading.
    pub async fn open(&self, run_id: Uuid, attachment_id: Uuid) -> io::Result<tokio::fs::File> {
        tokio::fs::File::open(self.path(run_id, attachment_id)).await
    }

    /// Removes all files of a hard-deleted run (FR-10). Call after the rows
    /// are deleted. A missing directory is fine.
    pub async fn delete_run(&self, run_id: Uuid) -> io::Result<()> {
        match tokio::fs::remove_dir_all(self.run_dir(run_id)).await {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }
}

async fn hash_file(path: &Path) -> io::Result<[u8; 32]> {
    use tokio::io::AsyncReadExt;
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            return Ok(hasher.finalize().into());
        }
        hasher.update(&buf[..n]);
    }
}

async fn sync_dir(dir: &Path) -> io::Result<()> {
    // Directory fsync makes the rename durable; not supported everywhere.
    match tokio::fs::File::open(dir).await {
        Ok(d) => match d.sync_all().await {
            Ok(()) => Ok(()),
            Err(_) => Ok(()),
        },
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(parts: &[&'static [u8]]) -> impl Stream<Item = Result<Bytes, io::Error>> + Unpin {
        futures::stream::iter(
            parts
                .iter()
                .map(|p| Ok(Bytes::from_static(p)))
                .collect::<Vec<_>>(),
        )
    }

    async fn read(store: &AttachmentStore, run: Uuid, id: Uuid) -> Vec<u8> {
        use tokio::io::AsyncReadExt;
        let mut buf = Vec::new();
        store
            .open(run, id)
            .await
            .unwrap()
            .read_to_end(&mut buf)
            .await
            .unwrap();
        buf
    }

    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[tokio::test]
    async fn writes_hashes_and_commits_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let store = AttachmentStore::new(dir.path());
        store.init().await.unwrap();
        let (run, id) = (Uuid::new_v4(), Uuid::new_v4());

        let temp = store
            .write_temp(run, body(&[b"hello ", b"world"]))
            .await
            .unwrap();
        assert_eq!(temp.size, 11);
        assert_eq!(
            temp.sha256.to_vec(),
            Sha256::digest(b"hello world").to_vec()
        );
        store.commit(temp, run, id).await.unwrap();

        assert_eq!(read(&store, run, id).await, b"hello world");
        // No temp file left behind.
        assert_eq!(entries(&store.run_dir(run)), vec![id.to_string()]);
    }

    #[tokio::test]
    async fn discarded_temp_files_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        let store = AttachmentStore::new(dir.path());
        let run = Uuid::new_v4();
        let temp = store.write_temp(run, body(&[b"x"])).await.unwrap();
        drop(temp);
        assert!(entries(&store.run_dir(run)).is_empty());
    }

    #[tokio::test]
    async fn failed_upload_leaves_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = AttachmentStore::new(dir.path());
        let run = Uuid::new_v4();
        let failing = futures::stream::iter(vec![
            Ok(Bytes::from_static(b"partial")),
            Err(io::Error::other("connection reset")),
        ]);
        let err = store.write_temp(run, failing).await.unwrap_err();
        assert!(matches!(err, WriteError::Body(_)));
        assert!(entries(&store.run_dir(run)).is_empty());
    }

    #[tokio::test]
    async fn stray_file_from_crash_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let store = AttachmentStore::new(dir.path());
        let (run, id) = (Uuid::new_v4(), Uuid::new_v4());
        let stray = store.write_temp(run, body(&[b"old"])).await.unwrap();
        store.commit(stray, run, id).await.unwrap();

        let temp = store.write_temp(run, body(&[b"new"])).await.unwrap();
        store.commit(temp, run, id).await.unwrap();
        assert_eq!(read(&store, run, id).await, b"new");
    }

    #[tokio::test]
    async fn delete_run_removes_directory() {
        let dir = tempfile::tempdir().unwrap();
        let store = AttachmentStore::new(dir.path());
        let (run, id) = (Uuid::new_v4(), Uuid::new_v4());
        let temp = store.write_temp(run, body(&[b"x"])).await.unwrap();
        store.commit(temp, run, id).await.unwrap();
        store.delete_run(run).await.unwrap();
        assert!(store.open(run, id).await.is_err());
        // Deleting again is fine.
        store.delete_run(run).await.unwrap();
    }
}
