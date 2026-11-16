use super::BinReader;

#[derive(Clone)]
pub struct MemoryBinReader {
    data: Vec<u8>,
}

impl BinReader for MemoryBinReader {
    fn get_data(&self) -> &[u8] {
        &self.data
    }
    fn len(&self) -> usize {
        self.data.len()
    }
}

impl MemoryBinReader {
    pub(crate) fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
}
