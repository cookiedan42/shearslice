//! A logical grouping of bricks in a voxel layer, encoded as a single file

use super::brick::Brick;

/// size in bytes of the header at the start of every node bin
const HEADER_SIZE: usize = 24;

/// Number of bricks stored per dimension, decoded from a node's brick mask.
///
/// Bit `z * nx * ny + y * nx + x` of `mask` is set if the brick at that slot of the
/// `node_size` grid is stored. Bricks are stored densely, so the set bits must form a
/// full `bx * by * bz` rectangle anchored at slot (0, 0, 0).
fn stored_extents(mask: u64, node_size: [usize; 3]) -> [usize; 3] {
    let [nx, ny, nz] = node_size;
    assert!(
        nx * ny * nz <= 64,
        "node_size {node_size:?} has more bricks than the 64 bit brick mask can describe"
    );
    let is_set = |x: usize, y: usize, z: usize| mask >> (z * nx * ny + y * nx + x) & 1 == 1;

    // extent along each axis = run of set bits starting at the origin
    let bx = (0..nx).take_while(|&x| is_set(x, 0, 0)).count();
    let by = (0..ny).take_while(|&y| is_set(0, y, 0)).count();
    let bz = (0..nz).take_while(|&z| is_set(0, 0, z)).count();

    let rectangle = (0..bz).fold(0u64, |m, z| {
        (0..by).fold(m, |m, y| {
            (0..bx).fold(m, |m, x| m | 1 << (z * nx * ny + y * nx + x))
        })
    });
    assert!(
        mask == rectangle,
        "brick mask {mask:#066b} is not a full rectangle for node_size {node_size:?}"
    );
    [bx, by, bz]
}

/// A logical grouping of bricks
/// BX, BY, BZ are the number of bricks in each dimension of the node
/// VX, VY, VZ are the number of voxels in each dimension of the brick
/// AW is the apron width
/// T is the datatype of the voxel
pub struct Node<T>
where
    T: bytemuck::Pod,
{
    // number of bricks actually stored in this node in each dimension (may be less than
    // the configured node_size for a node on the far edge of the volume, whose brick grid
    // doesn't divide evenly by node_size)
    bx: usize,
    by: usize,
    bz: usize,
    // number of voxels in this brick in each dimension
    vx: usize,
    vy: usize,
    vz: usize,
    // origin of this node, in global voxel coordinates
    ox: usize,
    oy: usize,
    oz: usize,
    // brick data
    bricks: Vec<Brick<T>>,
}

impl<T> Node<T>
where
    T: bytemuck::Pod,
{
    /// `node_size` is the configured number of bricks per node in each dimension; it is
    /// needed to decode the brick mask in the header. The number of bricks actually
    /// stored (a node on the far edge of the volume may store fewer than `node_size` if
    /// the brick grid doesn't divide evenly) is derived from that mask.
    /// vxyz are the number of voxels in each dimension of the brick
    /// aw is the apron width, which is constant for all dimensions
    ///
    /// Everything else is derived from the 24 byte header (all little-endian u32):
    /// - `0..12`: the node's brick index, matching the `X-Y-Z` in the bin's file name, so
    ///   origin = brick index * vxyz
    /// - `12..16`: byte size of one brick
    /// - `16..24`: u64 bitmask of the bricks stored, one bit per slot of the `node_size`
    ///   grid at `z * nx * ny + y * nx + x`. Must be a full rectangle from the node's
    ///   origin. (Read as u64 because a 4x4x4 node has 64 slots; the data seen so far
    ///   only ever sets the low 32 bits.)
    ///
    /// # Panics
    /// Panics if `data` is shorter than the header, if `node_size` has more than 64
    /// bricks, or if the mask isn't a rectangle anchored at the origin.
    #[allow(clippy::too_many_arguments)]
    pub fn from_bin(
        data: &[u8],
        node_size: [usize; 3],
        vx: usize,
        vy: usize,
        vz: usize,
        aw: usize,
    ) -> Self {
        assert!(
            data.len() >= HEADER_SIZE,
            "node bin is {} bytes, shorter than the {HEADER_SIZE} byte header",
            data.len()
        );

        // Brick offset derived from Node
        let bx = u32::from_le_bytes(data[0..4].try_into().unwrap());

        let by = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let bz = u32::from_le_bytes(data[8..12].try_into().unwrap());

        // Voxel offset by multiplying
        let ox = bx as usize * vx;
        let oy = by as usize * vy;
        let oz = bz as usize * vz;

        let brick_size = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;

        let mask = u64::from_le_bytes(data[16..24].try_into().unwrap());

        let [bx, by, bz] = stored_extents(mask, node_size);

        let bricks = (HEADER_SIZE..data.len())
            .step_by(brick_size)
            .map(|i| Brick::new(vx, vy, vz, aw, i, i + brick_size));

        Self {
            bx,
            by,
            bz,
            vx,
            vy,
            vz,
            ox,
            oy,
            oz,
            bricks: bricks.collect(),
        }
    }

    /// query a single voxel at (x,y,z) in the global coordinate space
    pub fn query(&self, x: usize, y: usize, z: usize, data: &[u8]) -> T {
        // convert to node-local coordinates by subtracting this node's origin.
        let x1 = x - self.ox;
        let y1 = y - self.oy;
        let z1 = z - self.oz;

        // pick the brick that contains this voxel
        let brick_x = x1 / self.vx;
        let brick_y = y1 / self.vy;
        let brick_z = z1 / self.vz;
        debug_assert!(brick_x < self.bx && brick_y < self.by && brick_z < self.bz);

        let brick_index = brick_z * self.by * self.bx + brick_y * self.bx + brick_x;

        // then query the brick
        self.bricks[brick_index].query(x, y, z, data)
    }
}
