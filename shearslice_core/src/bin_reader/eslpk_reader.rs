use bytes::Bytes;

use super::BinReader;

/// Read the binary file from eslpk
pub(crate) struct EslpkBinReader {
    // filepath? file handle?
    data: Bytes,
}

impl BinReader for EslpkBinReader {
    fn get_data(&self) -> Bytes {
        self.data.clone()
    }
    fn len(&self) -> usize {
        self.data.len()
    }
}

impl EslpkBinReader {
    pub(crate) fn new(data: Bytes) -> Self {
        Self { data }
    }
}
