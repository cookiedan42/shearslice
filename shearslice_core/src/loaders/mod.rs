#[cfg(feature = "std")]
use std::rc::Rc;

use hashbrown::HashMap;

#[cfg(feature = "std")]
use crate::bin_reader::SlpkBinReader;
use crate::bin_reader::{BinReader, MemoryBinReader};

/// Trait for loading binary data from different sources
pub trait BinLoader {
    type Reader: BinReader + 'static;
    fn load(&self, path: &str) -> Result<Rc<Self::Reader>, String>;
}

/// File system loader for native builds
#[cfg(feature = "std")]
pub struct FileLoader {
    base_path: String,
}

#[cfg(feature = "std")]
impl FileLoader {
    pub fn new(base_path: &str) -> Self {
        Self {
            base_path: base_path.to_string(),
        }
    }
}

#[cfg(feature = "std")]
impl BinLoader for FileLoader {
    type Reader = SlpkBinReader;
    fn load(&self, path: &str) -> Result<Rc<SlpkBinReader>, String> {
        Ok(Rc::new(SlpkBinReader::new(&format!(
            "{}/{}",
            self.base_path, path
        ))))
    }
}

/// In-memory loader for pre-loaded data (works in WASM)
pub struct MemoryLoader {
    // key is eslpk path
    // value is data
    data: HashMap<String, MemoryBinReader>,
}

impl MemoryLoader {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
}

impl Default for MemoryLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl BinLoader for MemoryLoader {
    type Reader = MemoryBinReader;

    fn load(&self, path: &str) -> Result<Rc<MemoryBinReader>, String> {
        self.data
            .get(path)
            .map(|data| Rc::new(data.clone()))
            .ok_or_else(|| format!("Data not found: {}", path))
    }
}
