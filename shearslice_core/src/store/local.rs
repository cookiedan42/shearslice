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

fn map_err(path: &Path, e: &std::io::Error) -> Error {
    if e.kind() == ErrorKind::NotFound {
        Error::NotFound { path: path.clone() }
    } else {
        Error::Generic {
            path: path.clone(),
            message: e.to_string(),
        }
    }
}

#[async_trait(?Send)]
impl BinStore for LocalFileSystem {
    async fn get(&self, path: &Path) -> Result<Bytes> {
        // Blocking open + map; nothing yields. A truly async fs backend can replace this later.
        let file =
            std::fs::File::open(self.root.join(path.as_str())).map_err(|e| map_err(path, &e))?;
        // SAFETY: bins are treated as immutable while mapped. Truncating or modifying
        // a file while it is mapped is undefined behaviour (e.g. SIGBUS on truncation).
        #[allow(unsafe_code)]
        let mmap = unsafe { memmap2::Mmap::map(&file) }.map_err(|e| map_err(path, &e))?;
        Ok(Bytes::from_owner(mmap))
    }
}
