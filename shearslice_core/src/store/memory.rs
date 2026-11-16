use super::{BinStore, Error, Path, Result};
use async_trait::async_trait;
use bytes::Bytes;
use core::cell::RefCell;
use hashbrown::HashMap;

/// In-memory store (works in WASM).
#[derive(Debug, Default)]
pub struct InMemory {
    data: RefCell<HashMap<Path, Bytes>>,
}

impl InMemory {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, path: Path, bytes: Bytes) {
        self.data.borrow_mut().insert(path, bytes);
    }
}

#[async_trait(?Send)]
impl BinStore for InMemory {
    async fn get(&self, path: &Path) -> Result<Bytes> {
        self.data
            .borrow()
            .get(path)
            .cloned()
            .ok_or_else(|| Error::NotFound { path: path.clone() })
    }
}
