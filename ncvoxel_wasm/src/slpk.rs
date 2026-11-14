//! `.slpk` packaging (port of theloderma `eslpk_to_slpk.py`).
//!
//! An `.slpk` is a ZIP using STORE (no ZIP compression) whose entries are individually gzipped
//! (`layer.json` -> `layer.json.gz`, names lowercased). The last entry is
//! `@specialIndexFileHASH128@`: 24-byte records `(md5(name incl. ".gz"), u64 local-header offset)`
//! sorted by the little-endian u64 halves of the digest. It is stored uncompressed and has no
//! record of its own.

use std::io::Write;

use flate2::{Compression, Crc, GzBuilder, write::GzEncoder};
use md5::{Digest, Md5};

use crate::{Error, Result};

pub const HASH_FILE: &str = "@specialIndexFileHASH128@";

const LOCAL_SIG: u32 = 0x0403_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const EOCD_SIG: u32 = 0x0605_4b50;
const ZIP64_EOCD_SIG: u32 = 0x0606_4b50;
const ZIP64_LOC_SIG: u32 = 0x0707_4b50;
/// 1980-01-01 00:00:00, what Python's `zipfile.writestr` stamps by default.
const DOS_TIME: u16 = 0;
const DOS_DATE: u16 = 0x21;

struct Entry {
    name: String,
    crc: u32,
    size: u32,
    offset: u32,
}

/// Streaming writer: each file is gzipped as it is added, so the uncompressed tree never needs to
/// exist in memory at once.
#[derive(Default)]
pub struct SlpkWriter {
    out: Vec<u8>,
    entries: Vec<Entry>,
}

fn canonical(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches('/')
        .to_lowercase()
}

pub fn gzip(data: &[u8]) -> Result<Vec<u8>> {
    // mtime 0 => deterministic output.
    let mut enc: GzEncoder<Vec<u8>> = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::default());
    enc.write_all(data)?;
    Ok(enc.finish()?)
}

impl SlpkWriter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an uncompressed eslpk file under its canonical, gzipped name.
    pub fn add(&mut self, path: &str, data: &[u8]) -> Result<()> {
        let name = canonical(path);
        if name == HASH_FILE.to_lowercase() {
            return Ok(()); // always recomputed
        }
        self.store(format!("{name}.gz"), &gzip(data)?)
    }

    fn store(&mut self, name: String, data: &[u8]) -> Result<()> {
        let size = u32::try_from(data.len()).map_err(|_| Error::new("entry larger than 4 GiB"))?;
        let offset =
            u32::try_from(self.out.len()).map_err(|_| Error::new("archive larger than 4 GiB"))?;
        let name_len = u16::try_from(name.len()).map_err(|_| Error::new("entry name too long"))?;
        let mut crc = Crc::new();
        crc.update(data);

        let o = &mut self.out;
        o.extend_from_slice(&LOCAL_SIG.to_le_bytes());
        o.extend_from_slice(&20u16.to_le_bytes()); // version needed
        o.extend_from_slice(&0u16.to_le_bytes()); // flags
        o.extend_from_slice(&0u16.to_le_bytes()); // method: STORE
        o.extend_from_slice(&DOS_TIME.to_le_bytes());
        o.extend_from_slice(&DOS_DATE.to_le_bytes());
        o.extend_from_slice(&crc.sum().to_le_bytes());
        o.extend_from_slice(&size.to_le_bytes()); // compressed
        o.extend_from_slice(&size.to_le_bytes()); // uncompressed
        o.extend_from_slice(&name_len.to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes()); // extra len
        o.extend_from_slice(name.as_bytes());
        o.extend_from_slice(data);

        self.entries.push(Entry {
            name,
            crc: crc.sum(),
            size,
            offset,
        });
        Ok(())
    }

    /// Append the hash index and the central directory, and return the archive.
    pub fn finish(mut self) -> Result<Vec<u8>> {
        let mut records: Vec<(u64, u64, [u8; 24])> = self
            .entries
            .iter()
            .map(|e| {
                let digest = Md5::digest(e.name.as_bytes());
                let lo = u64::from_le_bytes(digest[0..8].try_into().unwrap_or_default());
                let hi = u64::from_le_bytes(digest[8..16].try_into().unwrap_or_default());
                let mut rec = [0u8; 24];
                rec[..16].copy_from_slice(&digest);
                rec[16..].copy_from_slice(&u64::from(e.offset).to_le_bytes());
                (lo, hi, rec)
            })
            .collect();
        records.sort_by_key(|r| (r.0, r.1));
        let table: Vec<u8> = records.iter().flat_map(|r| r.2).collect();
        self.store(HASH_FILE.to_string(), &table)?;

        let cd_offset = self.out.len();
        for e in &self.entries {
            let o = &mut self.out;
            o.extend_from_slice(&CENTRAL_SIG.to_le_bytes());
            o.extend_from_slice(&20u16.to_le_bytes()); // version made by
            o.extend_from_slice(&20u16.to_le_bytes()); // version needed
            o.extend_from_slice(&0u16.to_le_bytes()); // flags
            o.extend_from_slice(&0u16.to_le_bytes()); // method
            o.extend_from_slice(&DOS_TIME.to_le_bytes());
            o.extend_from_slice(&DOS_DATE.to_le_bytes());
            o.extend_from_slice(&e.crc.to_le_bytes());
            o.extend_from_slice(&e.size.to_le_bytes());
            o.extend_from_slice(&e.size.to_le_bytes());
            o.extend_from_slice(&(e.name.len() as u16).to_le_bytes());
            o.extend_from_slice(&[0u8; 2 + 2 + 2 + 2 + 4]); // extra, comment, disk, int attr, ext attr
            o.extend_from_slice(&e.offset.to_le_bytes());
            o.extend_from_slice(e.name.as_bytes());
        }
        let cd_size = self.out.len() - cd_offset;
        let n = self.entries.len();
        let cd_offset =
            u32::try_from(cd_offset).map_err(|_| Error::new("archive larger than 4 GiB"))?;

        let o = &mut self.out;
        if n >= 0xFFFF {
            // ZIP64 end-of-central-directory record + locator, so readers get the real count.
            let z64_offset = o.len() as u64;
            o.extend_from_slice(&ZIP64_EOCD_SIG.to_le_bytes());
            o.extend_from_slice(&44u64.to_le_bytes());
            o.extend_from_slice(&45u16.to_le_bytes());
            o.extend_from_slice(&45u16.to_le_bytes());
            o.extend_from_slice(&0u32.to_le_bytes());
            o.extend_from_slice(&0u32.to_le_bytes());
            o.extend_from_slice(&(n as u64).to_le_bytes());
            o.extend_from_slice(&(n as u64).to_le_bytes());
            o.extend_from_slice(&(cd_size as u64).to_le_bytes());
            o.extend_from_slice(&u64::from(cd_offset).to_le_bytes());
            o.extend_from_slice(&ZIP64_LOC_SIG.to_le_bytes());
            o.extend_from_slice(&0u32.to_le_bytes());
            o.extend_from_slice(&z64_offset.to_le_bytes());
            o.extend_from_slice(&1u32.to_le_bytes());
        }
        let n16 = u16::try_from(n).unwrap_or(0xFFFF);
        o.extend_from_slice(&EOCD_SIG.to_le_bytes());
        o.extend_from_slice(&[0u8; 4]); // disk numbers
        o.extend_from_slice(&n16.to_le_bytes());
        o.extend_from_slice(&n16.to_le_bytes());
        o.extend_from_slice(&(cd_size as u32).to_le_bytes());
        o.extend_from_slice(&cd_offset.to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes()); // comment len
        Ok(self.out)
    }
}

/// Package an in-memory eslpk file list.
pub fn pack(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut w = SlpkWriter::new();
    for (path, data) in files {
        w.add(path, data)?;
    }
    w.finish()
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    fn u16_at(b: &[u8], i: usize) -> usize {
        u16::from_le_bytes(b[i..i + 2].try_into().unwrap()) as usize
    }
    fn u32_at(b: &[u8], i: usize) -> usize {
        u32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as usize
    }

    /// Minimal STORE-only reader: `(name, data, local_header_offset)`.
    fn read_zip(z: &[u8]) -> Vec<(String, Vec<u8>, usize)> {
        let eocd = z.len() - 22;
        assert_eq!(u32_at(z, eocd), EOCD_SIG as usize);
        let n = u16_at(z, eocd + 10);
        let mut p = u32_at(z, eocd + 16);
        let mut out = Vec::new();
        for _ in 0..n {
            assert_eq!(u32_at(z, p), CENTRAL_SIG as usize);
            assert_eq!(u16_at(z, p + 10), 0, "STORE only");
            let (size, nlen, elen, clen) = (
                u32_at(z, p + 24),
                u16_at(z, p + 28),
                u16_at(z, p + 30),
                u16_at(z, p + 32),
            );
            let off = u32_at(z, p + 42);
            let name = String::from_utf8(z[p + 46..p + 46 + nlen].to_vec()).unwrap();
            assert_eq!(u32_at(z, off), LOCAL_SIG as usize);
            let data_start = off + 30 + u16_at(z, off + 26) + u16_at(z, off + 28);
            let data = z[data_start..data_start + size].to_vec();
            let mut crc = Crc::new();
            crc.update(&data);
            assert_eq!(crc.sum() as usize, u32_at(z, p + 16), "crc of {name}");
            out.push((name, data, off));
            p += 46 + nlen + elen + clen;
        }
        out
    }

    #[test]
    fn md5_matches_reference_example() {
        // reference/special-hash.md worked example
        let d = Md5::digest(b"variables/2/stats.json.gz");
        assert_eq!(&d[..4], &[0x09, 0x4c, 0x6a, 0xc1]);
    }

    #[test]
    fn pack_round_trip_and_hash_index() {
        let files = vec![
            ("layer.json".to_string(), b"{\"a\":1}".to_vec()),
            (
                "variables/0/0/bundles/0/0-0-0.bin".to_string(),
                vec![7u8; 1000],
            ),
            ("Variables/0/Stats.json".to_string(), b"{}".to_vec()),
        ];
        let z = pack(&files).unwrap();
        let entries = read_zip(&z);
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[3].0, HASH_FILE);

        for ((path, data), (name, stored, _)) in files.iter().zip(&entries) {
            assert_eq!(name, &format!("{}.gz", path.to_lowercase()));
            let mut plain = Vec::new();
            flate2::read::GzDecoder::new(&stored[..])
                .read_to_end(&mut plain)
                .unwrap();
            assert_eq!(&plain, data);
        }

        let table = &entries[3].1;
        assert_eq!(table.len(), 24 * 3);
        let mut keys = Vec::new();
        for rec in table.chunks(24) {
            let off = u64::from_le_bytes(rec[16..].try_into().unwrap()) as usize;
            let e = entries
                .iter()
                .find(|e| e.2 == off)
                .expect("offset points at a local header");
            assert_eq!(rec[..16], Md5::digest(e.0.as_bytes())[..]);
            keys.push((
                u64::from_le_bytes(rec[0..8].try_into().unwrap()),
                u64::from_le_bytes(rec[8..16].try_into().unwrap()),
            ));
        }
        assert!(keys.windows(2).all(|w| w[0] <= w[1]), "records sorted");
    }

    #[test]
    fn gzip_is_deterministic() {
        assert_eq!(gzip(b"hello").unwrap(), gzip(b"hello").unwrap());
    }
}
