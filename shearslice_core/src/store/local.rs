use super::{BinStore, Error, Path, Result};
use async_trait::async_trait;
use bytes::Bytes;
use std::io::ErrorKind;
use std::path::PathBuf;

/// Reads bins from an unpacked eslpk directory on the local filesystem.
#[derive(Debug)]
pub struct LocalFileSystem {
    root: PathBuf,
}

impl LocalFileSystem {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

#[async_trait(?Send)]
impl BinStore for LocalFileSystem {
    async fn get(&self, path: &Path) -> Result<Bytes> {
        // Blocking read; nothing yields. A truly async fs backend can replace this later.
        match std::fs::read(self.root.join(path.as_str())) {
            Ok(v) => Ok(v.into()),
            Err(e) if e.kind() == ErrorKind::NotFound => {
                Err(Error::NotFound { path: path.clone() })
            }
            Err(e) => Err(Error::Generic {
                path: path.clone(),
                message: e.to_string(),
            }),
        }
    }
}
