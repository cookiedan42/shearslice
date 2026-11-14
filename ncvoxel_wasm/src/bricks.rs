//! Brick packing and chunk file layout (port of theloderma `bricks.py`).
//!
//! A *brick* is a 34³ cube: a 32³ data interior plus a 1-voxel apron on every face. A *chunk* is a
//! 4×4×4 group of bricks stored as one `bundles/0/<cx>-<cy>-<cz>.bin` file:
//!
//! ```text
//! u32 originX, u32 originY, u32 originZ   (chunk's lower brick coordinate, multiple of 4)
//! u32 payloadSize                         (34³ * sizeof(T))
//! u64 presenceMask                        (bit `slot` set => that brick is stored)
//! bricks in ascending slot order          (slot = lx + 4*ly + 16*lz)
//! ```
//!
//! The apron edge-replicates the volume boundary by one voxel; anything further out is zero.
//! Bricks are read straight from the source slab through an index map, so no padded copy of the
//! whole volume is ever allocated.

use crate::Result;
use crate::reader::Sample;

pub const BRICK: usize = 32;
pub const APRON: usize = 1;
pub const PADDED: usize = BRICK + 2 * APRON;
pub const CHUNK_BRICKS: usize = 4;
const HEADER_LEN: usize = 24;

/// Number of bricks along `[x, y, z]` for a slab of shape `[nz, ny, nx]`.
pub const fn grid_shape(shape: [usize; 3]) -> [usize; 3] {
    let [nz, ny, nx] = shape;
    [nx.div_ceil(BRICK), ny.div_ceil(BRICK), nz.div_ceil(BRICK)]
}

/// One chunk file, ready to be written.
pub struct Chunk {
    pub cx: usize,
    pub cy: usize,
    pub cz: usize,
    pub bytes: Vec<u8>,
}

impl Chunk {
    pub fn file_name(&self) -> String {
        format!("{}-{}-{}.bin", self.cx, self.cy, self.cz)
    }
}

/// Map the 34 padded positions of a brick along one axis to source indices.
///
/// `origin` is `brick_index * 32`, which is also the brick's first padded coordinate (the +1 for
/// the global apron and the -1 for the brick's own lower apron cancel out).
fn axis_map(origin: usize, n: usize) -> [Option<usize>; PADDED] {
    let mut out = [None; PADDED];
    for (k, slot) in out.iter_mut().enumerate() {
        let p = origin + k;
        *slot = match p {
            0 => Some(0),
            p if p <= n => Some(p - 1),
            p if p == n + 1 => Some(n - 1),
            _ => None,
        };
    }
    out
}

/// Emit one [`Chunk`] per occupied 4×4×4 brick block of `data` (shape `[nz, ny, nx]`, x fastest).
pub fn pack_chunks<T: Sample>(
    data: &[T],
    shape: [usize; 3],
    mut emit: impl FnMut(Chunk) -> Result<()>,
) -> Result<()> {
    let [nz, ny, nx] = shape;
    let [nbx, nby, nbz] = grid_shape(shape);
    let payload = PADDED * PADDED * PADDED * size_of::<T>();

    for cz in (0..nbz).step_by(CHUNK_BRICKS) {
        for cy in (0..nby).step_by(CHUNK_BRICKS) {
            for cx in (0..nbx).step_by(CHUNK_BRICKS) {
                let mut bytes = vec![0u8; HEADER_LEN];
                let mut mask = 0u64;
                for lz in 0..CHUNK_BRICKS.min(nbz - cz) {
                    let zm = axis_map((cz + lz) * BRICK, nz);
                    for ly in 0..CHUNK_BRICKS.min(nby - cy) {
                        let ym = axis_map((cy + ly) * BRICK, ny);
                        for lx in 0..CHUNK_BRICKS.min(nbx - cx) {
                            let xm = axis_map((cx + lx) * BRICK, nx);
                            mask |= 1 << (lx + 4 * ly + 16 * lz);
                            bytes.reserve(payload);
                            for z in zm {
                                for y in ym {
                                    for x in xm {
                                        match (z, y, x) {
                                            (Some(z), Some(y), Some(x)) => {
                                                data[(z * ny + y) * nx + x].push_le(&mut bytes);
                                            }
                                            _ => T::ZERO.push_le(&mut bytes),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                let payload_size = u32::try_from(payload)
                    .map_err(|_| crate::Error::new("brick payload exceeds u32"))?;
                bytes[0..4].copy_from_slice(&(cx as u32).to_le_bytes());
                bytes[4..8].copy_from_slice(&(cy as u32).to_le_bytes());
                bytes[8..12].copy_from_slice(&(cz as u32).to_le_bytes());
                bytes[12..16].copy_from_slice(&payload_size.to_le_bytes());
                bytes[16..24].copy_from_slice(&mask.to_le_bytes());
                emit(Chunk { cx, cy, cz, bytes })?;
            }
        }
    }
    Ok(())
}

/// `(min, max)` of every leaf brick's data interior (apron excluded), in `x + nbx*(y + nby*z)`
/// order. `valid` decodes a raw voxel and returns `None` for no-data. A brick with no valid voxel
/// gets `(+inf, -inf)`.
pub fn leaf_minmax<T: Sample>(
    data: &[T],
    shape: [usize; 3],
    valid: impl Fn(T) -> Option<f64>,
) -> Vec<(f64, f64)> {
    let [nz, ny, nx] = shape;
    let [nbx, nby, nbz] = grid_shape(shape);
    let mut out = Vec::with_capacity(nbx * nby * nbz);
    for bz in 0..nbz {
        for by in 0..nby {
            for bx in 0..nbx {
                let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
                for z in bz * BRICK..(bz * BRICK + BRICK).min(nz) {
                    for y in by * BRICK..(by * BRICK + BRICK).min(ny) {
                        let row = (z * ny + y) * nx;
                        for &v in &data[row + bx * BRICK..row + (bx * BRICK + BRICK).min(nx)] {
                            if let Some(d) = valid(v) {
                                lo = lo.min(d);
                                hi = hi.max(d);
                            }
                        }
                    }
                }
                out.push((lo, hi));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(data: &[f32], shape: [usize; 3]) -> Vec<Chunk> {
        let mut out = Vec::new();
        pack_chunks(data, shape, |c| {
            out.push(c);
            Ok(())
        })
        .unwrap();
        out
    }

    fn f32_at(bytes: &[u8], i: usize) -> f32 {
        f32::from_le_bytes(
            bytes[HEADER_LEN + 4 * i..HEADER_LEN + 4 * i + 4]
                .try_into()
                .unwrap(),
        )
    }

    #[test]
    fn single_brick_interior_and_apron() {
        let (nz, ny, nx) = (3, 4, 5);
        let data: Vec<f32> = (0..nz * ny * nx).map(|i| i as f32 + 1.0).collect();
        let chunks = run(&data, [nz, ny, nx]);
        assert_eq!(chunks.len(), 1);
        let c = &chunks[0];
        assert_eq!(c.file_name(), "0-0-0.bin");
        assert_eq!(c.bytes.len(), HEADER_LEN + PADDED.pow(3) * 4);
        assert_eq!(u64::from_le_bytes(c.bytes[16..24].try_into().unwrap()), 1);
        assert_eq!(
            u32::from_le_bytes(c.bytes[12..16].try_into().unwrap()),
            (PADDED.pow(3) * 4) as u32
        );
        let at = |z: usize, y: usize, x: usize| f32_at(&c.bytes, (z * PADDED + y) * PADDED + x);
        // interior
        assert_eq!(at(1, 1, 1), data[0]);
        assert_eq!(at(nz, ny, nx), data[data.len() - 1]);
        // edge replication, including a corner
        assert_eq!(at(0, 0, 0), data[0]);
        assert_eq!(at(nz + 1, ny + 1, nx + 1), data[data.len() - 1]);
        assert_eq!(at(0, 2, 3), data[(ny + 1) * 0 + nx + 2]);
        // beyond the apron is zero
        assert_eq!(at(nz + 2, 1, 1), 0.0);
        assert_eq!(at(1, 1, nx + 2), 0.0);
    }

    #[test]
    fn multi_brick_grid_and_slots() {
        // 40 wide => 2 bricks in x; they share a chunk with slots 0 and 1.
        let (nz, ny, nx) = (2, 2, 40);
        let data: Vec<f32> = (0..nz * ny * nx).map(|i| i as f32).collect();
        let chunks = run(&data, [nz, ny, nx]);
        assert_eq!(chunks.len(), 1);
        let c = &chunks[0];
        assert_eq!(
            u64::from_le_bytes(c.bytes[16..24].try_into().unwrap()),
            0b11
        );
        let brick = PADDED.pow(3);
        assert_eq!(c.bytes.len(), HEADER_LEN + 2 * brick * 4);
        // brick 1 (x 32..40): its lower apron is the neighbour's last data column (x = 31).
        let at1 =
            |z: usize, y: usize, x: usize| f32_at(&c.bytes, brick + (z * PADDED + y) * PADDED + x);
        assert_eq!(at1(1, 1, 0), data[31]);
        assert_eq!(at1(1, 1, 1), data[32]);
    }

    #[test]
    fn leaf_minmax_skips_invalid() {
        let data = [1.0f32, -9999.0, 3.0, 2.0];
        let mm = leaf_minmax(&data, [1, 2, 2], |v| (v != -9999.0).then(|| f64::from(v)));
        assert_eq!(mm, vec![(1.0, 3.0)]);
        let all_bad = leaf_minmax(&[-9999.0f32], [1, 1, 1], |v| {
            (v != -9999.0).then(|| f64::from(v))
        });
        assert_eq!(all_bad, vec![(f64::INFINITY, f64::NEG_INFINITY)]);
    }
}
