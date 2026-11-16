use super::BinLoader;
use crate::bin_reader::MemoryBinReader;
use hashbrown::HashMap;
use std::rc::Rc;

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
    pub fn load2(&self, path: &str) -> Result<MemoryBinReader, String> {
        self.data
            .get(path)
            .map(|data| data.clone())
            .ok_or_else(|| format!("Data not found: {}", path))
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
