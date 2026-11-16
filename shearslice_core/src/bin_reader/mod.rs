use bytemuck;
use bytes::Bytes;

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

impl BinReader for Bytes {
    fn get_data(&self) -> Bytes {
        self.clone()
    }
    fn get_at(&self, start: usize, end: usize) -> Bytes {
        self.slice(start..end)
    }
    fn len(&self) -> usize {
        self.as_ref().len()
    }
}

// Blanket impl so every BinReader gets BinReaderExt for free
impl<B: BinReader> BinReaderExt for B {}
