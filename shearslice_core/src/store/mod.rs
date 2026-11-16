//! Storage abstraction for node bins, modelled on the `object_store` crate:
//! an async, object-safe trait plus interchangeable backends.

use crate::voxel_layer::NodeKey;
use async_trait::async_trait;
use bytes::Bytes;
use core::fmt;
use core::ops::Range;

mod memory;
pub use memory::InMemory;

#[cfg(feature = "std")]
mod local;
#[cfg(feature = "std")]
pub use local::LocalFileSystem;

/// Location of a bin relative to the layer root,
/// e.g. `variables/0/0/bundles/0/4-0-0.bin`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Path(String);

impl Path {
    /// Path of the bin for `layer_id` at node origin `key`.
    #[must_use]
    pub fn bin(layer_id: usize, key: NodeKey) -> Self {
        let (x, y, z) = key;
        // TODO: add parameter for time dim
        Self(format!("variables/{layer_id}/0/bundles/0/{x}-{y}-{z}.bin"))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Path {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for Path {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Errors returned by a [`BinStore`] or by queries that depend on one.
#[derive(Debug)]
pub enum Error {
    /// The object does not exist (or, for cached queries, is not loaded).
    NotFound { path: Path },
    /// Any other backend failure.
    Generic { path: Path, message: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { path } => write!(f, "bin not found: {path}"),
            Self::Generic { path, message } => write!(f, "error reading {path}: {message}"),
        }
    }
}

impl core::error::Error for Error {}

pub type Result<T, E = Error> = core::result::Result<T, E>;

/// A source of node bins. Futures are not required to be `Send`, so backends may hold
/// `Rc`/`RefCell` state or wrap non-thread-safe handles such as wasm `fetch`.
#[async_trait(?Send)]
pub trait BinStore: fmt::Debug {
    /// Fetch the whole object.
    async fn get(&self, path: &Path) -> Result<Bytes>;

    /// Fetch a byte range. The default fetches everything and slices; backends that
    /// support range reads should override it.
    async fn get_range(&self, path: &Path, range: Range<usize>) -> Result<Bytes> {
        let bytes = self.get(path).await?;
        if range.start > range.end || range.end > bytes.len() {
            return Err(Error::Generic {
                path: path.clone(),
                message: format!("range {range:?} out of bounds for {} bytes", bytes.len()),
            });
        }
        Ok(bytes.slice(range))
    }
}
