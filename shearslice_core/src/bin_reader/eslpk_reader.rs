use super::BinReader;

/// Read the binary file from eslpk
pub(crate) struct EslpkBinReader {
    // filepath? file handle?
    data: Vec<u8>,
}

impl BinReader for EslpkBinReader {
    fn get_data(&self) -> &[u8] {
        &self.data
    }
    fn len(&self) -> usize {
        self.data.len()
    }
}

impl EslpkBinReader {
    pub(crate) fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
}
