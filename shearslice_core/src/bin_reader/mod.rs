mod eslpk_reader;
mod memory_reader;
mod slpk_reader;

pub(crate) use eslpk_reader::EslpkBinReader;
pub(crate) use memory_reader::MemoryBinReader;
pub(crate) use slpk_reader::SlpkBinReader;

/// Implementing this trait enables different backing stores to provide the binary data for a Voxel
pub trait BinReader {
    fn get_data(&self) -> &[u8];
    fn get_at(&self, start: usize, end: usize) -> &[u8] {
        &self.get_data()[start..end]
    }
    fn len(&self) -> usize;
}
