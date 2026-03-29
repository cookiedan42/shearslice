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
    // number of bricks in this node in each dimension
    bx: usize,
    by: usize,
    bz: usize,
    // number of voxels in this brick in each dimension
    vx: usize,
    vy: usize,
    vz: usize,
    // brick data
    bricks: Vec<Brick<T>>,
}

impl<T> Node<T>
where
    T: FromBytes + Copy + bytemuck::Pod,
{
    pub fn from_bin(
        data: &[u8],
        bx: usize,
        by: usize,
        bz: usize,
        vx: usize,
        vy: usize,
        vz: usize,
        aw: usize,
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
            bricks: Brick::from_bin(&data[24..], vx, vy, vz, aw),
        }
    }
    pub fn query(&self, x: usize, y: usize, z: usize) -> T {
        let x1 = x % (self.vx * self.bx);
        let y1 = y % (self.vy * self.by);
        let z1 = z % (self.vz * self.bz);

        // pick the brick that contains this voxel
        let brick_x = x1 / self.vx; // 0,1,2,3
        let brick_y = y1 / self.vy; // 0,4,8,12
        let brick_z = z1 / self.vz; // 0,16,32,48
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

    pub fn query(&self, x: usize, y: usize, z: usize) -> T {
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
