use hashbrown::HashMap;

/// Trait for loading binary data from different sources
pub trait BinLoader {
    fn load(&self, path: &str) -> Result<Vec<u8>, String>;
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
    fn load(&self, path: &str) -> Result<Vec<u8>, String> {
        std::fs::read(format!("{}/{}", self.base_path, path))
            .map_err(|e| format!("Failed to read {}: {}", path, e))
    }
}

/// In-memory loader for pre-loaded data (works in WASM)
pub struct MemoryLoader {
    data: HashMap<String, Vec<u8>>,
}

impl MemoryLoader {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn insert(&mut self, path: &str, data: Vec<u8>) {
        self.data.insert(path.to_string(), data);
    }
}

impl Default for MemoryLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl BinLoader for MemoryLoader {
    fn load(&self, path: &str) -> Result<Vec<u8>, String> {
        self.data
            .get(path)
            .cloned()
            .ok_or_else(|| format!("Data not found: {}", path))
    }
}
