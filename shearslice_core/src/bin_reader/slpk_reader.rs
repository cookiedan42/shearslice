use super::BinReader;
use bytes::Bytes;

/// Read the binary file from slpk  
/// uncompressed zip container following eslpk structure where individual zip files are gzipped  
pub struct SlpkBinReader {
    path: String,
}

impl BinReader for SlpkBinReader {
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

impl SlpkBinReader {
    pub(crate) fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }
}
