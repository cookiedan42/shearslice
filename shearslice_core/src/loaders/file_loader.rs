use super::BinLoader;
use crate::bin_reader::EslpkBinReader;
use std::rc::Rc;

/// File system loader for native builds, based on eslpk
pub struct EslpkLoader {
    base_path: String,
}

impl EslpkLoader {
    pub fn new(base_path: &str) -> Self {
        Self {
            base_path: base_path.to_string(),
        }
    }
    pub fn load2(&self, path: &str) -> Result<EslpkBinReader, String> {
        Ok(EslpkBinReader::new(&format!("{}/{}", self.base_path, path)))
    }
}

impl BinLoader for EslpkLoader {
    type Reader = EslpkBinReader;
    fn load(&self, path: &str) -> Result<Rc<EslpkBinReader>, String> {
        Ok(Rc::new(EslpkBinReader::new(&format!(
            "{}/{}",
            self.base_path, path
        ))))
    }
}
