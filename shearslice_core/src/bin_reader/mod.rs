mod eslpk_reader;
mod memory_reader;
mod slpk_reader;

use bytemuck;
use bytes::Bytes;

pub(crate) use eslpk_reader::EslpkBinReader;
pub(crate) use memory_reader::MemoryBinReader;
pub(crate) use slpk_reader::SlpkBinReader;

/// Implementing this trait enables different backing stores to provide the binary data for a Voxel
pub trait BinReader {
    fn get_data(&self) -> Bytes;
    fn get_at(&self, start: usize, end: usize) -> Bytes {
        self.get_data().slice(start..end)
    }

    fn len(&self) -> usize;
}

pub trait BinReaderExt: BinReader {
    /// Read the next bytes starting at this offset as one element of type T
    fn read_as<T: bytemuck::Pod>(&self, offset: usize) -> T {
        let size = std::mem::size_of::<T>();
        *bytemuck::from_bytes(&self.get_at(offset, offset + size))
    }
}

// Blanket impl so every BinReader gets BinReaderExt for free
impl<B: BinReader> BinReaderExt for B {}
