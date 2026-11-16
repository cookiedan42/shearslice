use bytes::Bytes;

use super::BinReader;

/// Read the binary file from eslpk
pub struct EslpkBinReader {
    path: String,
}

impl BinReader for EslpkBinReader {
    fn get_data(&self) -> Bytes {
        // Opens and reads the file each access but that's ok
        std::fs::read(&self.path)
            .expect("path not found, unable to load bin")
            .into()
    }
    fn len(&self) -> usize {
        self.get_data().len()
    }
}

impl EslpkBinReader {
    pub(crate) fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }
}
