use num_traits::FromBytes;

/// A logical grouping of bricks
/// BX, BY, BZ are the number of bricks in each dimension of the node
/// VX, VY, VZ are the number of voxels in each dimension of the brick
/// AW is the apron width
/// T is the datatype of the voxel
pub struct Node<T>
where
    T: FromBytes + Copy + bytemuck::Pod,
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
    T: FromBytes + Copy + bytemuck::Pod,
{
    /// bxyz are the number of bricks actually stored in this node in each dimension (a
    /// node on the far edge of the volume may store fewer than the configured node_size
    /// if the brick grid doesn't divide evenly)
    /// vxyz are the number of voxels in each dimension of the brick
    /// aw is the apron width, which is constant for all dimensions
    /// oxyz is this node's origin, in global voxel coordinates (node_key * vxyz)
    #[allow(clippy::too_many_arguments)]
    pub fn from_bin(
        data: &[u8],
        bx: usize,
        by: usize,
        bz: usize,
        vx: usize,
        vy: usize,
        vz: usize,
        aw: usize,
        ox: usize,
        oy: usize,
        oz: usize,
    ) -> Self {
        // 24 byte header which is the brick index which this node starts with
        // followed by the individual bricks concatenated
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
            bricks: Brick::from_bin(&data[24..], vx, vy, vz, aw),
        }
    }
    /// query a single voxel at (x,y,z) in the global coordinate space
    pub fn query(&self, x: usize, y: usize, z: usize) -> T {
        // convert to node-local coordinates by subtracting this node's origin. This must
        // be a subtraction, not `x % (vx*bx)`: on the far edge of the volume this node's
        // stored brick count (bx/by/bz) may not evenly divide the configured node_size
        // (e.g. 3 bricks stored where a full node would hold 4), and in that case the
        // modulus would wrap at the wrong period.
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
        self.bricks[brick_index].query(x, y, z)
    }
}

pub struct Brick<T>
where
    T: FromBytes + Copy + bytemuck::Pod,
{
    vx: usize,
    vy: usize,
    vz: usize,
    aw: usize,
    data: Vec<T>,
}

impl<T> Brick<T>
where
    T: FromBytes + Copy + bytemuck::Pod,
{
    pub fn from_bin(data: &[u8], vx: usize, vy: usize, vz: usize, aw: usize) -> Vec<Self> {
        let item_size = core::mem::size_of::<T>();
        let brick_size = (vx + aw + aw) * (vy + aw + aw) * (vz + aw + aw) * item_size;

        (0..(data.len() / brick_size))
            .map(|i| Self {
                data: bytemuck::cast_slice::<u8, T>(&data[i * brick_size..(i + 1) * brick_size])
                    .to_vec(),
                vx,
                vy,
                vz,
                aw,
            })
            .collect()
    }

    /// query a single voxel at (x,y,z) in the global coordinate space
    pub fn query(&self, x: usize, y: usize, z: usize) -> T {
        // convert to local coordinates
        let x = x % self.vx;
        let y = y % self.vy;
        let z = z % self.vz;

        // fully skipped stride has pre-apron and post-apron
        let stride_x = self.vx + self.aw + self.aw;
        let stride_y = self.vy + self.aw + self.aw;

        // unskipped stride only has pre-apron
        let index = (x + self.aw) + (y + self.aw) * stride_x + (z + self.aw) * stride_x * stride_y;
        self.data[index]
    }
}
