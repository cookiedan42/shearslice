#[cfg(feature = "std")]
mod file_loader;
#[cfg(feature = "std")]
pub(crate) use file_loader::EslpkLoader;

mod memory_loader;
pub(crate) use memory_loader::MemoryLoader;

use crate::bin_reader::BinReader;
use std::rc::Rc;

/// Trait for loading binary data from different sources
pub trait BinLoader {
    type Reader: BinReader + 'static;
    fn load(&self, path: &str) -> Result<Rc<Self::Reader>, String>;
}
