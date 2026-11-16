use super::BinReader;
use std::cell::OnceCell;

/// Read the binary file from slpk  
/// uncompressed zip container following eslpk structure where individual zip files are gzipped  
pub struct SlpkBinReader {
    path: String,
    data: OnceCell<Vec<u8>>,
}

impl BinReader for SlpkBinReader {
    fn get_data(&self) -> &[u8] {
        self.data
            .get_or_init(|| std::fs::read(&self.path).expect("path not found, unable to load bin"))
    }
    fn len(&self) -> usize {
        self.get_data().len()
    }
}

impl SlpkBinReader {
    pub(crate) fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            data: OnceCell::new(),
        }
    }
}
