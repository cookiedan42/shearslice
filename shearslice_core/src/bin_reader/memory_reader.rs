use bytes::Bytes;

use super::BinReader;

#[derive(Clone)]
pub struct MemoryBinReader {
    data: Bytes,
}

impl BinReader for MemoryBinReader {
    fn get_data(&self) -> Bytes {
        self.data.clone()
    }
    fn len(&self) -> usize {
        self.data.len()
    }
}

impl MemoryBinReader {
    pub(crate) fn new(data: Bytes) -> Self {
        Self { data }
    }
}
